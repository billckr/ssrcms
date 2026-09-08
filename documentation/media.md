---
title: Media Library
group: feature
updated_by: claude
last_updated: 2026-08-12
---
# Media Library

> Last updated: 2026-08-12 | Updated by: claude

## Overview

The media library stores uploaded files on the filesystem, organized into per-site subdirectories, and records metadata in the `media` table. Files are served statically via `/uploads/*`. The library supports images (with dimension detection), and any other MIME type; media can optionally be organized into named folders (`media_folders`, migrations 0035–0036).

The admin media library page itself (`/admin/media`) is a **WASM island** — the first page converted from full-page-reload server rendering to a client-side Leptos app (`media-app` crate). Folder switching, type filtering, pagination, upload, and folder create/delete now happen in place, backed by a JSON API, instead of a page navigation per click.

## How It Works

### Model (`core/src/models/media.rs`)

Key structs:
- `Media` — full DB row: `id`, `site_id`, `filename` (original name), `mime_type`, `path` (stored path, `{site_uuid}/{stored_filename}`), `alt_text`, `title`, `caption`, `width`, `height`, `file_size`, `uploaded_by`, `folder_id`, `created_at`.
- `MediaContext` — template-safe view (`id`, `url`, `filename`, `mime_type`, `alt_text`, `title`, `caption`, `width`, `height`).
- `CreateMedia` — insert struct.

`Media::url(base_url)` builds the public URL. For real hostnames (containing a `.`, not `localhost`) it emits a **bare-filename path**, `/uploads/{filename}` — no hostname or UUID segment. Production Caddy scopes each site's own `/uploads/*` root to that site's symlinked upload folder (`uploads/{hostname}/` → `uploads/{site_uuid}/`, maintained by `ensure_hostname_symlink`), so the site is already implied by which domain served the request; repeating it in the path would be redundant. For local/dev hosts it falls back to the raw UUID-based `path`.

**Every other place that reconstructs an `/uploads/` URL from a stored `Media` row must follow this same bare-filename convention** — it is not automatic just because `Media::url()` does the right thing. As of 2026-08-12 this includes: the admin media grid (`list()` and `api_grid()` in `core/src/handlers/admin/media.rs`), the media-app WASM detail-panel sync (`media-app/src/window_items.rs`), and the post editor's featured-image preview reconstruction (`edit_post_type` in `core/src/handlers/admin/posts.rs`). Each of these strips the UUID prefix (`path.splitn(2, '/').nth(1)`) rather than using the raw stored `path`. If a future call site builds an `/uploads/` URL some other way, it will silently produce broken images once accessed through Caddy in production — there is no single shared helper enforcing this yet; it's currently a convention each call site has to follow by hand.

Key functions: `create`, `get_by_id`, `update_media_meta`, `unassign_folder`, `delete`, `list` (filterable by `site_id`, `uploaded_by`, and `folder_id`), `count`.

`MediaFolder` (`core/src/models/media_folder.rs`): `id`, `site_id`, `name`, `created_at`. `list`, `create`, `delete` are all scoped to a `site_id`.

### Upload Handler (`core/src/handlers/admin/upload.rs`)

`POST /admin/media/upload` — multipart form accepting `file`, optional `alt_text`, optional `folder_id`, and an internal `redirect` field (validated to start with `/admin/`). Generates a URL-safe stored filename: `{slugified-stem-capped-at-80-chars}-{8-char-uuid}.{ext}`. Writes bytes to a **per-site subdirectory** (`{uploads_dir}/{site_uuid}/`), creating it if needed; falls back to the flat uploads dir (with a warning) if no `site_id` is present. Reads image dimensions directly from the in-memory bytes via `imagesize::blob_size` for `image/*` MIME types. Body size is capped by `DefaultBodyLimit::max(config.max_upload_mb * 1MB)` (default 25 MB, configurable via app settings).

The media-app WASM island uploads to this same endpoint via `XMLHttpRequest` (not `fetch`) specifically to get `xhr.upload.onprogress` events for a real progress percentage on the dropzone — `fetch` has no upload-progress API.

### Admin Media Handler (`core/src/handlers/admin/media.rs`)

- `list` — the original server-rendered page. Still renders the initial HTML shell (SSR fallback) that the WASM island mounts into and takes over — folder list, type tabs, upload form, grid/list containers, and pagination/footer all keep their existing element IDs so the island can find and replace their contents on load. Authors (`site_role == "author"`) see only their own uploads; supports a `picker`/`browser` query mode for embedding in the rich-text/image picker (loaded in an iframe — see below).
- `api_grid` (`GET /admin/api/media/grid`) — **the WASM island's actual data source.** JSON response: `items` (id/filename/type/isImage/path/alt/title/caption/size/dims/uploader/uploaded_at/folder_id — the same shape as the legacy embedded `ITEMS` array), `folders`, `type_counts`, `total`/`page`/`page_size`/`total_pages`. Takes `folder_id`/`type`/`page` query params. Mirrors `list()`'s filter/pagination logic but returns data instead of HTML, so folder/type/page changes can happen client-side without a full page reload.
- `delete` — enforces site ownership (403 if `media.site_id != admin.site_id` for non-global-admins) and author-only restriction (403 if an author tries to delete another user's upload); removes the file from disk then the DB row.
- `api_list` (`GET /admin/api/media`) — JSON array of the caller's accessible images (`image/` MIME prefix only), up to 500 items, used by the rich text editor's image picker. Distinct from `api_grid` — this one is unfiltered by type/folder and images-only, built for the Quill inline-image picker rather than the media library grid.
- `api_update_meta` (`POST /admin/api/media/{id}/meta`) — JSON body `alt_text`/`title`/`caption`, sanitized via `sanitize_media_text`; enforces site ownership.
- `api_update_folder` (`POST /admin/api/media/{id}/folder`) — assigns/clears a media item's folder; verifies both the media item and the target folder belong to the caller's site before allowing the change.
- `create_folder` (`POST /admin/media/folders/new`) — folder name sanitized to alphanumerics/hyphens, 4–25 chars. Called from the WASM island via `fetch`, not a real form submit (see below).
- `delete_folder` (`POST /admin/media/folders/{id}/delete`) — optionally cascades to delete all media files/rows in the folder (`delete_media=true`), otherwise just unassigns the folder from its media so items fall back to "All Media". Also called via `fetch` from the island.

### The WASM Island (`media-app/` crate)

A separate workspace crate, compiled to `wasm32-unknown-unknown` and loaded via `wasm-bindgen` (`target: web`) from a `<script type="module">` at the bottom of the media library page. Built with Leptos in **CSR mode** (`features = ["csr"]`), not SSR/hydrate — there's no server-rendered tree to match, so it just mounts fresh into a handful of specific element IDs in the existing page and replaces their contents:

| Element ID (in `admin/src/pages/media.rs`) | Component | Owns |
|---|---|---|
| `mm-type-tabs-app` | `TypeTabs` | Image/Video/Audio/Document filter tabs + counts |
| `mm-folder-select-app` | `FolderSelect` | Folder dropdown |
| `mm-delete-folder-app` | `DeleteFolderButton` | "Delete folder" button (only rendered when a folder is selected) |
| `mm-new-folder-btn-app` | `NewFolderButton` | "Folder +" button |
| `mm-new-folder-modal-app` | `NewFolderModal` | New-folder name input + Create/Cancel — renders nothing until opened |
| `mm-delete-folder-modal-app` | `DeleteFolderModal` | Delete-folder confirmation (message + Move/Delete/Cancel) — renders nothing until opened |
| `mm-toolbar-app` | `Toolbar` | Upload dropzone + hidden file input, with progress |
| `mmGridWrap` | `ContentGrid` | Grid tiles + list-view table rows |
| `mmPagination` | `Pagination` | Page number links |
| `mmFooterInfo` | `FooterInfo` | "Showing X–Y of Z files" text |

All components share one set of signals (`media-app/src/state.rs`: `folder_id`, `type_filter`, `page`, `grid`, `loading`, plus per-modal `show_*`/error signals) via a `thread_local!` — changing one (e.g. clicking a type tab) triggers `state::refresh()`, which re-fetches `/admin/api/media/grid` and updates every mounted component reactively, even though they're mounted as separate trees rather than one shared component tree.

**New folder / Delete folder are owned entirely by the island**, not legacy JS. `NewFolderModal`/`DeleteFolderModal` POST to `create_folder`/`delete_folder` (above) via `gloo_net`, then call `state::refresh()` in place — no `window.location.reload()`, unlike the original hand-written versions. Deleting a folder always resets to "All Media" afterward (`state::set_folder(None)`), since the delete button is only reachable while that folder is the one currently selected. This also fixed two bugs that existed under the old server-rendered version: the delete-confirmation message's file count and the search-clear footer text were both frozen at page-load time (`FOLDER_TOTAL`/`{footer_info}` baked into the initial HTML) and would go stale the moment folder switching stopped triggering a full reload; both now read live state instead.

**Bridging to the legacy (pre-WASM) JS**: the item detail panel and bulk select/move/delete are still the original hand-written JS in `media.rs`'s inline `<script>` — that part was not rewritten. That script reads two globals, `window.ITEMS` and `window.FOLDERS`, indexed by array position (`data-idx` DOM attributes) to know what the user clicked. Two things make this work correctly with the island:
1. The legacy script's own `<script>` block assigns to these via `window.ITEMS = ...` / `window.FOLDERS = ...`, **not** `var ITEMS = ...`. This is deliberate — `var` inside the script's IIFE would make them closure-local, invisible to the WASM module's own `window.ITEMS = ...` reassignments on every refresh (this was a real, hard-to-spot bug: the legacy JS silently kept reading a frozen page-load snapshot for a while before this was caught).
2. `media-app/src/window_items.rs` (`sync_items`/`sync_folders`) rewrites both globals every time the island fetches new data, in the exact array order the DOM was just rendered in, and rewrites `path` with the `/uploads/` prefix baked in (the grid's own Rust-rendered `<img>` tags add that prefix themselves; the legacy detail panel does not, so it has to already be present in the value it reads).

Rendered items keep the same `data-idx`/`data-type`/`data-name`/`onclick="selectItem(this)"` attributes the legacy JS expects, so the bridge is invisible from that side — it just looks like a normal DOM node to click on.

Bulk move/delete (still legacy JS) originally ended with `window.location.reload()`. Now that folder/filter switching doesn't reload the page, that stood out as a jarring full-page flash by comparison — both now call `window.mediaAppRefresh()` instead (a `#[wasm_bindgen]`-exported `refresh_grid()`, assigned to that name once the module loads), which re-fetches in place. They also now explicitly `selected.clear()` afterward — the bulk-selection Set stores array *positions*, and since the grid can reorder/shrink after a refresh, leftover stale positions from before a move could otherwise cause a later action to silently operate on the wrong item.

**Initial paint — avoiding a flash of empty content**: `mount()` is `async` and `await`s one grid fetch (`state::initial_load()`) *before* mounting any component, rather than mounting empty components and letting a background fetch catch up. Without this, every mount point would briefly clear its SSR fallback, paint blank (since `grid` starts as `None`), then repaint again a moment later once the fetch resolved — a visible flash between the SSR content disappearing and the real content appearing. Awaiting first means the very first paint already has real data.

**Build**: `./app.sh build`/`build`/`rebuild` all run a `build_wasm` step that compiles `media-app` (`cargo build -p media-app --target wasm32-unknown-unknown --release`) and regenerates `admin/static/media-app/{media_app.js,media_app_bg.wasm}` via `wasm-bindgen`. **Editing `media.rs` (the SSR page) without also running a real rebuild — i.e. using `./app.sh restart` instead of `rebuild` — leaves the running server serving stale HTML while the WASM bundle expects the new element IDs/markup.** This produced a real "mount point not found" bug during development; always use `rebuild` when both sides changed together. `install-vps.sh`'s `do_build()` step and its requirements check (for `wasm32-unknown-unknown` + `wasm-bindgen-cli`) were updated to match — a VPS deploy builds the WASM bundle locally, same as the main binary, and ships only the resulting static files.

### The Media Picker (`admin/src/lib.rs`)

Used from the post/page editor ("Set Featured Image") and Quill's inline image/audio insert, plus the left-nav "Media" entry (a second, near-identical iframe: `media-browser-frame`/`openMediaBrowser`). Opens `/admin/media?picker=1` (or `?browser=1` for the nav browser) in an **iframe**; the selected item is returned to the parent page via `postMessage`. This is a clean boundary — the picker's internals (now including the WASM island) don't need the parent page, or vice versa, to know anything about each other. Converting the media library to WASM did not require any change to picker call sites.

**Keeping the iframe warm across opens (2026-08-12)**: originally both `openMediaPicker`/`openMediaBrowser` reset the iframe's `src` to `about:blank` on every close, so reopening always meant a full page load — including re-running the *entire* WASM bootstrap (download, instantiate, initial fetch) from scratch every single time. Now, closing just hides the modal; the iframe (and its already-running WASM island) is left alone. On reopen, if the iframe was already loaded once (`data-loaded === '1'`), instead of resetting `frame.src`, the parent posts a `{ type: 'resetPickerFilter', typeFilter }` message into it. `media.rs`'s inline script forwards that to `window.mediaAppResetForPicker` (a `#[wasm_bindgen]`-exported `reset_for_picker()`), which resets to a clean "All Media" view scoped to the requested type filter (only the audio-insert picker mode actually filters by type) and refetches — in place, no reload. First open per admin page still pays the full bootstrap cost; every open after that in the same page reuses the warm iframe.

This does **not** persist across a real page navigation — navigating to a different admin page is a genuine browser page load, which tears down the iframe (and everything inside it) along with the rest of the page. The gain is "once per admin page visited," not "once ever." See `TODO.md` for a noted, not-yet-done partial mitigation (caching the last-fetched grid data in `sessionStorage` so even a fresh page's first open skips the network round trip) and the longer-term option (converting the whole admin into a persistent client-side-routed WASM SPA, which is the only thing that fully solves this — see the project's own planning notes on that, currently slated for after feature-complete, not incremental work).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/media | `admin::media::list` | Media library page — SSR shell that the WASM island mounts into |
| GET | /admin/api/media/grid | `admin::media::api_grid` | JSON grid data (items/folders/counts/pagination) — the island's data source |
| POST | /admin/media/upload | `admin::upload::upload` | Upload file |
| POST | /admin/media/{id}/delete | `admin::media::delete` | Delete media item |
| POST | /admin/media/folders/new | `admin::media::create_folder` | Create a media folder — now called via `fetch` from the island, not a form submit |
| POST | /admin/media/folders/{id}/delete | `admin::media::delete_folder` | Delete a folder (optionally its contents) — same, via `fetch` |
| GET | /admin/api/media | `admin::media::api_list` | JSON image-only list, for the Quill inline-image picker |
| POST | /admin/api/media/{id}/meta | `admin::media::api_update_meta` | Update alt/title/caption |
| POST | /admin/api/media/{id}/folder | `admin::media::api_update_folder` | Assign/clear folder |

## Database Schema

`media` table: `id UUID PK`, `site_id UUID`, `filename TEXT`, `mime_type TEXT`, `path TEXT`, `alt_text TEXT`, `title TEXT`, `caption TEXT` (both added migration 0024), `width INT`, `height INT`, `file_size BIGINT`, `uploaded_by UUID`, `folder_id UUID` (added migration 0036, `REFERENCES media_folders(id) ON DELETE SET NULL`), `created_at TIMESTAMPTZ`.

`media_folders` table (migration 0035): `id UUID PK`, `site_id UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE`, `name TEXT NOT NULL`, `created_at TIMESTAMPTZ`, `UNIQUE (site_id, name)`.

## Configuration

`max_upload_mb` (`synaptic.toml` / hot-reloadable app settings, default 25) caps multipart upload size via `DefaultBodyLimit`, applied to the `/admin/media/upload` route.

## Deployment / Caddy

In production, `/uploads/*` bypasses Axum entirely — Caddy serves it directly via `file_server` for performance. Each site's Caddyfile block roots that block at `{UPLOADS_DIR}/{DOMAIN}` (the deployed template) or `{UPLOADS_DIR}/{http.request.host}` (this repo's local hand-maintained dev catch-all Caddyfile, which has no per-site block to substitute a literal domain into) — either way, scoped to *that site's own* symlinked upload folder, matching `Media::url()`'s bare-filename convention. If a site's uploads stop resolving in production after an upload-URL-related code change, check whether the deployed Caddyfile actually got regenerated from the current template — this is not automatic on every deploy.

The dev-mode Axum `uploads::serve()` handler supports both URL shapes so local testing without Caddy still works: a bare filename resolves the site via the request's `Host` header (same pattern as everywhere else site context is derived from the request); the legacy `/uploads/{key}/{rest}` two-segment shape (UUID or hostname) is also still supported indefinitely, so already-published content with old-format links doesn't break.

## Testing

`tests/e2e/media_bulk_move.py` — a Playwright-driven regression test for the WASM island's bulk-move flow (select → move → in-place refresh → select a different item → move again, all in one page session with no reload in between). This scenario is exactly what surfaced the `window.ITEMS`/`selected`-staleness bugs described above; a Rust integration test can't reach this class of bug since it's purely client-side/WASM state. See `tests/e2e/README.md` for how to run it.

## Security Notes

- Site isolation is enforced throughout: non-global-admins cannot view/delete/update media, assign folders, or delete folders belonging to another site; folder assignment additionally verifies the target folder's `site_id` matches.
- Authors are further scoped to their own uploads for delete and default list views.
- Uploaded filenames are slugified (`slugify_name`) and capped at 80 chars before being written to disk, preventing directory traversal and unwieldy filenames.
- Files are stored under a per-site subdirectory (`uploads/{site_uuid}/`) rather than a single flat directory, limiting blast radius between sites on shared storage.
- `alt_text`/`title`/`caption` values are passed through `sanitize_media_text` before being stored.
- The WASM island is CSR-only and runs entirely client-side against the same session-cookie-authenticated API routes the old server-rendered page used — it introduces no new trust boundary or auth path of its own.

