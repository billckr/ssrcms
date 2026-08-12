# E2E tests

Browser-driven Playwright tests for behavior that can't be verified by
Rust integration tests (`core/tests/`) — client-side/WASM state bugs,
in-page reactivity, anything that only manifests through real DOM/network
interaction in a running browser.

## Requirements

- Dev server running (`./app.sh start`)
- Python 3 with `playwright` installed (`pip install playwright && playwright install chromium`)

## Running a test

Credentials are always passed via env vars — never hardcode them in a script:

```bash
E2E_ADMIN_EMAIL=you@example.com E2E_ADMIN_PASSWORD='...' python3 tests/e2e/<script>.py
```

Optional: `E2E_BASE_URL` (defaults to `http://localhost:3000`) to point at a
different running instance.

## Tests

- `media_bulk_move.py` — regression test for the media-library WASM island's
  bulk-move flow. Requires 2+ media items and a folder named
  `e2e-test-folder` on the current admin site.
