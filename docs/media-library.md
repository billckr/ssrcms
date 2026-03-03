# Media Library — Technical Reference

This document covers the full implementation of the media library system in Synaptic
Signals: how files are stored, how ownership and access control work, how the admin
UI is built, and how the featured image picker integrates with the post editor. Intended
for any developer or AI agent that needs to maintain, debug, or extend this system.

---

## Table of Contents

1. [Database Schema](#database-schema)
2. [File Storage](#file-storage)
3. [Ownership and Access Control](#ownership-and-access-control)
4. [HTTP Routes](#http-routes)
5. [Rust Source Files — Map](#rust-source-files--map)
6. [Upload Flow](#upload-flow)
7. [Delete Flow](#delete-flow)
8. [Media List Query](#media-list-query)
9. [JSON API for the Picker](#json-api-for-the-picker)
10. [Admin UI — Media Page](#admin-ui--media-page)
11. [Featured Image on the Post Editor](#featured-image-on-the-post-editor)
12. [Media Picker Modal](#media-picker-modal)
13. [Template Context — `featured_image`](#template-context--featured_image)
14. [Theme CSS — Front-end Display](#theme-css--front-end-display)
15. [Known Gaps and Future Work](#known-gaps-and-future-work)

---

## Database Schema

Migration: `migrations/0002_create_media.sql`

```sql
CREATE TABLE media (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename      TEXT NOT NULL,          -- original filename from the browser
    mime_type     TEXT NOT NULL,
    path          TEXT NOT NULL,          -- relative path under uploads/ (UUID-renamed)
    alt_text      TEXT NOT NULL DEFAULT '',
    width         INTEGER,                -- null for non-image files (not yet auto-detected)
    height        INTEGER,                -- null for non-image files (not yet auto-detected)
    file_size     BIGINT NOT NULL DEFAULT 0,
    uploaded_by   UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_media_uploaded_by ON media(uploaded_by);
CREATE INDEX idx_media_mime_type   ON media(mime_type);
```

`site_id` was added later in `migrations/0010_add_site_id_to_content.sql`:

```sql
ALTER TABLE media ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE CASCADE;
```

`site_id` is **nullable**. A null value means the record pre-dates multi-site support or
belongs to the global/legacy single-site installation.

### Key points

- `uploaded_by` is a hard FK to `users`. Deleting a user who still owns media is blocked
  by `ON DELETE RESTRICT` — the user must have their media re-assigned or deleted first.
- `users.avatar_media_id` has a back-reference FK to `media.id` with `ON DELETE SET NULL`,
  set up at the bottom of `0002_create_media.sql` once the table exists.
- `width`/`height` columns exist but are **never populated** by the current upload handler.
  Image dimension detection is a future enhancement (see [Known Gaps](#known-gaps-and-future-work)).

---

## File Storage

Uploaded files are written to the directory configured in `synaptic.toml`:

```toml
uploads_dir = "uploads"   # relative to the binary's working directory
```

The directory is served as static files via Tower's `ServeDir` at the `/uploads/` prefix:

```
router.nest_service("/uploads", ServeDir::new(uploads_dir))
```

Files are **renamed on upload** to prevent collisions and path traversal:

```
<uuid-v4>.<original-extension>
```

Example: `button.png` → `9f1c432a-4b55-4797-8065-e629ce1debe6.png`

The `media.path` column stores only the renamed filename (not a full path). The full disk
path is always constructed as `{uploads_dir}/{media.path}`, and the public URL as
`/uploads/{media.path}`.

---

## Ownership and Access Control

Every media record carries two scoping fields:

| Field         | Purpose |
|---------------|---------|
| `uploaded_by` | UUID of the user who uploaded the file |
| `site_id`     | UUID of the site the file belongs to (nullable) |

Access rules are enforced in the handler layer (`core/src/handlers/admin/media.rs`):

### Viewing / listing

| Role | What they see |
|------|--------------|
| `super_admin` | All media across all sites (site_id filter is NULL = no filter) |
| `admin` / `editor` | All media belonging to their current site |
| `author` | Only their own uploads (`uploaded_by = current_user.id`) |

### Deleting

1. **Site isolation** — a non-global admin cannot delete media that belongs to a
   different site, even if they know the UUID.
2. **Author restriction** — authors can only delete their own uploads.

Both checks return HTTP 403 with the body `"Forbidden"` on violation and log a warning
via `tracing::warn!`.

### Uploading

Any authenticated admin user can upload. The record is created with:
- `site_id = admin.site_id` (the site the user is currently acting on)
- `uploaded_by = admin.user.id`

---

## HTTP Routes

All routes require an active admin session (enforced by the `AdminUser` extractor).

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET`  | `/admin/media` | `media::list` | HTML media library page |
| `POST` | `/admin/media/upload` | `upload::upload` | Multipart file upload |
| `POST` | `/admin/media/{id}/delete` | `media::delete` | Delete one file |
| `GET`  | `/admin/api/media` | `media::api_list` | **JSON** — image list for the picker |

The `/admin/api/media` route is registered in `core/src/router.rs` immediately before
the standard media routes.

---

## Rust Source Files — Map

```
core/src/
  handlers/admin/
    media.rs          — list, delete, api_list handlers
    upload.rs         — multipart upload handler
  models/
    media.rs          — Media struct, CreateMedia, DB functions

admin/src/           (compiled into the binary via include_str!)
  pages/
    media.rs          — HTML render for the /admin/media page
    posts.rs          — Post editor render (includes featured image section + picker modal)
  style/
    admin.css         — All CSS including .featured-image-* and .mpicker-* classes

migrations/
  0002_create_media.sql         — Initial table
  0010_add_site_id_to_content.sql — Added site_id column
```

> **Important:** `admin/style/admin.css` is embedded at compile time via
> `include_str!("../style/admin.css")` in `admin/src/lib.rs`. CSS changes require a
> full `./app.sh rebuild` — a restart alone is not enough.

---

## Upload Flow

```
Browser (multipart POST /admin/media/upload)
  └─► upload::upload handler (core/src/handlers/admin/upload.rs)
        ├─ Parse multipart fields: "file" and optional "alt_text"
        ├─ Derive extension from original filename
        ├─ Generate stored filename: {uuid}.{ext}
        ├─ Write bytes to {uploads_dir}/{stored_name}  (async tokio::fs::write)
        ├─ Build CreateMedia { site_id, filename, mime_type, path, alt_text,
        │                       width: None, height: None, file_size, uploaded_by }
        ├─ INSERT into media table via models::media::create()
        └─ Redirect to /admin/media
```

Upload size is limited by `max_upload_mb` in `synaptic.toml`, enforced via Axum's
`DefaultBodyLimit` layer applied to the upload route only.

---

## Delete Flow

```
Browser (POST /admin/media/{id}/delete)
  └─► media::delete handler
        ├─ Load record by UUID (404 if not found — silently redirects)
        ├─ Check site ownership (403 if mismatch)
        ├─ Check author restriction (403 if not own upload)
        ├─ Remove file from disk: std::fs::remove_file({uploads_dir}/{media.path})
        │    └─ Logs warning on failure but continues (DB record still deleted)
        ├─ DELETE from media table via models::media::delete()
        └─ Redirect to /admin/media
```

Note: deletion does **not** check whether the media is referenced as a post's
`featured_image_id`. Deleting an in-use image will leave `posts.featured_image_id` pointing
to a deleted record. This is a known gap — see [Known Gaps](#known-gaps-and-future-work).

---

## Media List Query

`core/src/models/media.rs` — `pub async fn list()`

```sql
SELECT * FROM media
WHERE ($1::uuid IS NULL OR site_id = $1)
  AND ($2::uuid IS NULL OR uploaded_by = $2)
ORDER BY created_at DESC
LIMIT $3 OFFSET $4
```

Passing `None` for either `site_id` or `uploaded_by` disables that filter (the `IS NULL`
check makes it a no-op). This single function is reused by the HTML list page, the JSON
API endpoint, and any future callers.

Current limits:
- HTML admin page: limit = 200, offset = 0
- JSON API (picker): limit = 500, offset = 0

Pagination is not yet implemented on either surface.

---

## JSON API for the Picker

`GET /admin/api/media` → `media::api_list`

Returns a JSON array. Only records where `mime_type` starts with `"image/"` are included
(PDFs and other non-image uploads are intentionally excluded from the picker).

### Response shape

```json
[
  {
    "id":        "6183da49-5274-4973-b613-5b5b2e8647b9",
    "filename":  "button.png",
    "url":       "/uploads/9f1c432a-4b55-4797-8065-e629ce1debe6.png",
    "alt_text":  "A button image",
    "mime_type": "image/png"
  }
]
```

- `url` is always a root-relative path (`/uploads/{path}`) suitable for use in `<img src>`.
- The same role-based filter applies as for the HTML list: authors only see their own images.
- This endpoint requires an active admin session. Unauthenticated requests are redirected
  to `/admin/login` by the `AdminUser` extractor.

---

## Admin UI — Media Page

`admin/src/pages/media.rs` — `render_list()`

The page renders:
1. A **drag-and-drop upload zone** with a hidden `<input type="file">` and JavaScript to
   show the chosen filename.
2. An **alt text input** (optional, sent with the upload form).
3. A **media grid** of `.media-card` components, one per file.
   - Images get a thumbnail via `<img src="/uploads/{path}">`.
   - Non-image files (e.g. PDF) get a generic file icon with the MIME type.
   - Each card has a delete button (POST form with a confirm dialog).

The upload form targets `POST /admin/media/upload` with `enctype="multipart/form-data"`.

---

## Featured Image on the Post Editor

`admin/src/pages/posts.rs` — `render_editor()`

### Data flow into the editor

`PostEdit` carries two new fields:

```rust
pub featured_image_id:  Option<String>,  // UUID string, empty = no image
pub featured_image_url: Option<String>,  // "/uploads/{path}" for preview
```

When rendering an **edit** form for an existing post, the handler (`edit_post_type` in
`core/src/handlers/admin/posts.rs`) fetches the media record by the post's
`featured_image_id` UUID to resolve the URL:

```rust
let featured_image_url = if let Some(img_id) = post.featured_image_id {
    crate::models::media::get_by_id(&state.db, img_id).await
        .ok()
        .map(|m| format!("/uploads/{}", m.path))
} else {
    None
};
```

### Form fields

Two hidden `<input>` elements are included in the post form:

```html
<input type="hidden" id="featured_image_id"    name="featured_image_id"    value="{uuid}">
<input type="hidden" id="featured_image_url_field" name="featured_image_url" value="/uploads/...">
```

On form submit these are sent alongside all other fields. The `PostForm` deserializer in
`core/src/handlers/admin/posts.rs` receives them:

```rust
pub featured_image_id:  Option<String>,
pub featured_image_url: Option<String>,
```

`featured_image_url` is only used to re-populate the editor UI on validation errors — it
is never written to the database. Only `featured_image_id` is parsed to a UUID and written:

```rust
featured_image_id: form.featured_image_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()),
```

An invalid or empty UUID string is silently treated as `None`.

### Sidebar box states

| State | CSS class on box | Content |
|-------|-----------------|---------|
| No image selected | `.featured-image-box` (default) | SVG placeholder + "No image selected" label |
| Image selected | `.featured-image-box.has-image` | `<img>` filling the box |

The "✕ Remove featured image" button is hidden (`display:none`) when no image is set and
shown when one is. Both states are set server-side on page load and updated client-side
when the user picks or removes an image.

---

## Media Picker Modal

The picker is rendered as inline HTML + a `<script>` block appended after the post editor
form in `render_editor()`. It is pure client-side — no page navigation or server round-trip
occurs during image selection.

### Structure

```
#media-picker-modal  (.mpicker-overlay)
  └─ .mpicker-dialog
       ├─ .mpicker-header
       │    ├─ .mpicker-title  "Featured Image"
       │    ├─ #mpicker-search  (live filter input)
       │    └─ .mpicker-close   ✕
       └─ .mpicker-body
            ├─ #mpicker-grid   (.mpicker-grid)   — thumbnail grid
            └─ #mpicker-detail (.mpicker-detail) — detail panel
```

### Lifecycle

1. **Open** — `openMediaPicker()` is called. On the first open, fetches
   `GET /admin/api/media` and stores the result in the `allMedia` JS array. Subsequent
   opens reuse the cached array (no re-fetch). The currently selected image (if any) is
   highlighted in the grid.

2. **Grid render** — `renderGrid(items)` builds `<div class="mpicker-thumb">` elements.
   Each stores `data-id`, `data-url`, `data-filename`, `data-alt` attributes.

3. **Thumbnail click** — `pickThumb(el)` marks the clicked thumb with
   `.mpicker-thumb-selected` and populates the detail panel with a large preview, the
   filename, and a "Set Featured Image" confirm button.

4. **Confirm** — `confirmFeaturedImage()` writes `selectedId` and `selectedUrl` into the
   two hidden form inputs, updates the sidebar box DOM, adds `.has-image` to the box,
   shows the remove button, then closes the modal.

5. **Remove** — `removeFeaturedImage()` clears both hidden inputs, removes `.has-image`,
   restores the SVG placeholder, and hides the remove button.

6. **Search** — `filterMedia(q)` filters `allMedia` by `filename` (case-insensitive
   substring match) and re-renders the grid. The `allMedia` array is never mutated.

7. **Close** — clicking the ✕ button or the dark overlay backdrop calls
   `closeMediaPicker()`. Closing without confirming preserves any previously confirmed
   selection.

### CSS classes (admin.css)

| Class | Purpose |
|-------|---------|
| `.mpicker-overlay` | Full-screen fixed backdrop |
| `.mpicker-dialog` | The modal box — `80vw` / `80vh`, capped at `1400px` / `900px` |
| `.mpicker-grid` | CSS grid, `auto-fill` with `minmax(140px, 1fr)` thumbnails |
| `.mpicker-thumb` | Individual thumbnail tile |
| `.mpicker-thumb-selected` | Blue ring on the currently selected thumbnail |
| `.mpicker-detail` | Right-side detail panel, `280px` wide |
| `.mpicker-loading` | Placeholder text shown while loading or when empty |
| `.featured-image-box` | Sidebar preview box (dashed border, `16/10` aspect ratio) |
| `.featured-image-box.has-image` | Solid border, `overflow:hidden`, black background |
| `.featured-image-remove` | Red "remove" link below the box |

---

## Template Context — `featured_image`

When a post is rendered on the public-facing site, `featured_image` is available in Tera
templates as part of the `post` context object:

```
{{ post.featured_image.url }}
{{ post.featured_image.alt_text }}
{{ post.featured_image.filename }}
{{ post.featured_image.width }}   {# may be null #}
{{ post.featured_image.height }}  {# may be null #}
```

The `PostContext::build()` function in `core/src/models/post.rs` receives a
`Option<MediaContext>` argument. The callers (home, single post, archive, search handlers)
fetch the related media record and pass it in. If `featured_image_id` is null the field is
absent from the context (`None` → the Tera key is missing, so guard with `{% if
post.featured_image %}`).

---

## Theme CSS — Front-end Display

**Recommended upload size:** **1920 × 1080 px** (16:9, standard 1080p). This fills the single-post hero at full width without upscaling on large screens. Minimum acceptable is **1200 × 675 px** — below that the image will visibly soften on retina/HiDPI displays. File format: JPEG for photos, PNG for graphics with transparency.

Both bundled themes (`default` and `Anthropic`) use three context-aware rules in `static/css/style.css`:

| Selector | Treatment |
|----------|-----------|
| `.featured-image` | Base: `width:100%; height:auto; border-radius` — fallback if neither context class is present |
| `.single-post .featured-image` | Hero: `aspect-ratio: 16/9; object-fit: cover; max-height: 520px` |
| `.post-item .featured-image` | Thumbnail in lists: `aspect-ratio: 16/9; object-fit: cover; max-height: 320px` |

The `aspect-ratio` + `object-fit: cover` approach means the image scales fluidly with its container — no fixed pixel heights that produce inconsistent results at different viewport widths.

All three templates that output posts (`index.html`, `single.html`, `archive.html`) wrap the image in `{% if post.featured_image %}` so nothing renders when no image is set.

### `.featured-image--sharp` modifier

Add this class alongside `featured-image` to remove rounded corners:

```html
<img src="{{ post.featured_image.url }}" alt="{{ post.featured_image.alt_text }}"
     class="featured-image featured-image--sharp">
```

---

## Known Gaps and Future Work

| # | Gap | Notes |
|---|-----|-------|
| 1 | **No in-use guard on delete** | Deleting a media record that is referenced as a post's `featured_image_id` leaves a dangling FK. The `posts.featured_image_id` column has no `ON DELETE` constraint currently. Fix: add `ON DELETE SET NULL` to the FK, or check for references before permitting deletion. |
| 2 | **Width/height never populated** | `media.width` and `media.height` are always `NULL`. Add image dimension detection (e.g. with the `image` crate) in `upload.rs` after writing the file. |
| 3 | **No pagination** | Both the HTML list and the JSON API load up to 200/500 records. Add cursor- or page-based pagination before media libraries grow large. |
| 4 | **Picker caches on first open** | `allMedia` is fetched once and never refreshed. If the user uploads a file in another tab during the same session, the new file won't appear in the picker until page reload. Fix: add a "Refresh" button or re-fetch on each open. |
| 5 | **Images only in picker** | The picker filters to `mime_type LIKE 'image/%'`. PDFs and other file types cannot be selected as featured images. This is intentional for now but may need revisiting for document-type sites. |
| 6 | **No inline upload in picker** | WordPress allows uploading directly from the featured image modal. This would require wiring an upload form inside the picker JS. |
| 7 | **Alt text not editable after upload** | `update_alt_text()` exists in `models/media.rs` but no admin UI exposes it. The detail panel in the picker is a natural place to add an editable alt text field. |
