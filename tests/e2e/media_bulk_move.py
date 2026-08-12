"""
E2E regression test for the media-library bulk-move stale-selection bug.

Reproduces: select item -> bulk move -> (in-place refresh, no reload) ->
select another item -> bulk move again, all in one page session (no reload
in between). Verifies correctness by querying the JSON grid API scoped to
the target folder directly, rather than trusting DOM item counts (folders
are an optional filter, not exclusive of "All Media" — moving a file INTO
a folder does not remove it from the unfiltered "All Media" count, that's
expected behavior, not a bug).

Prerequisites:
  - The dev server running (./app.sh start)
  - At least 2 media items uploaded on the current admin site
  - A folder named "e2e-test-folder" on that site (create once via the
    media library UI, or `synap` / direct SQL insert into media_folders)

Credentials come from env vars, not hardcoded:
  E2E_ADMIN_EMAIL=you@example.com E2E_ADMIN_PASSWORD='...' python3 tests/e2e/media_bulk_move.py
"""
import os
import json
from playwright.sync_api import sync_playwright

EMAIL = os.environ["E2E_ADMIN_EMAIL"]
PASSWORD = os.environ["E2E_ADMIN_PASSWORD"]
BASE = os.environ.get("E2E_BASE_URL", "http://localhost:3000")
TEST_FOLDER_NAME = "e2e-test-folder"


def login(page):
    page.goto(f"{BASE}/admin/login")
    page.fill("#email", EMAIL)
    page.fill("#password", PASSWORD)
    page.click("button[type=submit]")
    page.wait_for_load_state("networkidle")
    if "/admin/login" in page.url:
        raise RuntimeError("login failed — check E2E_ADMIN_EMAIL/E2E_ADMIN_PASSWORD")


def grid(page, folder_id=None):
    """Hit the JSON grid API directly (ground truth), bypassing DOM state entirely."""
    url = f"{BASE}/admin/api/media/grid"
    if folder_id:
        url += f"?folder_id={folder_id}"
    resp = page.request.get(url)
    assert resp.ok, f"grid API failed: {resp.status}"
    return resp.json()


def bulk_move_nth_item(page, folder_label, tag, n=0):
    """Enter bulk mode, select the nth visible grid item, move it via the modal."""
    page.click("#mmBulkToggle")
    page.wait_for_timeout(200)
    nth_item = page.locator("#mmGrid .mm-item").nth(n)
    nth_item.click()
    page.wait_for_timeout(150)

    page.click("#mmBulkBar >> text=Move")
    page.wait_for_selector("#mmMoveModal", state="visible")
    page.select_option("#mmMoveSelect", label=folder_label)
    page.click("#mmMoveModal >> button:has-text('Move')")
    page.wait_for_timeout(1200)  # fetch chain + mediaAppRefresh
    page.screenshot(path=f"/tmp/e2e_{tag}_after.png")


def run():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        console_errors = []
        page.on("console", lambda msg: console_errors.append(msg.text) if msg.type == "error" else None)
        page.on("pageerror", lambda exc: console_errors.append(f"pageerror: {exc}"))

        def log_folder_calls(resp):
            if "/folder" in resp.url and "media" in resp.url:
                try:
                    print(f"[net] {resp.request.method} {resp.url} -> {resp.status} body={resp.request.post_data!r}")
                except Exception as e:
                    print(f"[net] {resp.url} -> {resp.status} (log failed: {e})")
        page.on("response", log_folder_calls)

        login(page)

        page.goto(f"{BASE}/admin/media")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(500)

        data = grid(page)
        folder = next((f for f in data["folders"] if f["name"] == TEST_FOLDER_NAME), None)
        if not folder:
            raise RuntimeError(f"test folder '{TEST_FOLDER_NAME}' not found — create it first")
        folder_id = folder["id"]

        if len(data["items"]) < 2:
            print(f"[skip] need at least 2 media items to test with, have {len(data['items'])}")
            browser.close()
            return

        item_a = data["items"][0]["id"]
        print(f"[info] item A = {item_a}")

        # ── Move #1: item A -> test folder ──────────────────────────────
        bulk_move_nth_item(page, TEST_FOLDER_NAME, "move1", n=0)
        in_folder = grid(page, folder_id)["items"]
        move1_ok = any(i["id"] == item_a for i in in_folder)
        print(f"[{'PASS' if move1_ok else 'FAIL'}] move #1: item A present in test folder = {move1_ok}")

        # ── Reset selection UI state, pick a DIFFERENT item, move #2 ────
        # No page reload in between — this is the exact scenario that
        # would trigger stale `selected`/frozen-ITEMS bugs.
        page.click("text=All files")
        page.wait_for_timeout(500)

        all_now = grid(page)["items"]
        candidates = [i["id"] for i in all_now if i["id"] != item_a]
        if not candidates:
            print("[skip] no second distinct item available for move #2")
        else:
            item_b = candidates[0]
            print(f"[info] item B = {item_b}")
            # item B is whatever DOM item is NOT item A — with only 2 items and
            # stable created_at ordering (folder moves don't reorder), that's
            # index 1. Verify the id at that DOM position actually matches
            # item_b before moving, so the test fails loudly if that
            # assumption ever stops holding instead of silently mismoving.
            live_items = grid(page)["items"]
            assert live_items[1]["id"] == item_b, (
                f"test assumption broke: DOM index 1 is not item B "
                f"(items order: {[i['id'] for i in live_items]})"
            )
            bulk_move_nth_item(page, TEST_FOLDER_NAME, "move2", n=1)
            in_folder_2 = grid(page, folder_id)["items"]
            ids_in_folder = {i["id"] for i in in_folder_2}
            move2_correct_item = item_b in ids_in_folder
            print(f"[info] items now in test folder: {sorted(ids_in_folder)}")
            print(f"[{'PASS' if move2_correct_item else 'FAIL'}] move #2: item B present in test folder = {move2_correct_item}")
            print(f"[{'PASS' if item_a in ids_in_folder else 'FAIL'}] move #1's item A still present after move #2 (no clobber) = {item_a in ids_in_folder}")

        # ── Cleanup: move both back out so re-runs start clean ──────────
        for item_id in [item_a] + (candidates[:1] if candidates else []):
            page.request.post(
                f"{BASE}/admin/api/media/{item_id}/folder",
                data=json.dumps({"folder_id": None}),
                headers={"Content-Type": "application/json"},
            )

        if console_errors:
            print("\n[console errors captured]")
            for e in console_errors:
                print(" -", e)

        browser.close()


if __name__ == "__main__":
    run()
