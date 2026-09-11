---
title: Posts
group: feature
updated_by: codex
last_updated: 2026-09-10
---
# Posts

> Last updated: 2026-09-10 | Updated by: codex

## Overview

Posts are the primary content type. They are authored, optionally submitted for review, scheduled, and published by admin users. Each post belongs to a site and supports categories, tags, a featured image, comments, password protection, a saved/reading-list feature, and per-post view tracking. Posts share the `posts` table with Pages (`post_type = 'post'`).

## How It Works

### Data Model (`core/src/models/post.rs`)

The `Post` struct's key columns: `id`, `site_id`, `title`, `slug`, `content`, `content_format`, `excerpt`, `status` (`draft`, `pending`, `published`, `scheduled`, `trashed`), `post_type` (`post` or `page`), `author_id`, `featured_image_id`, `published_at`, `scheduled_at`, `submitted_at`, `template`, `post_password` (argon2 hash), `comments_enabled`, `parent_id`. `PostContext` is the Tera-facing view (adds `url`, `breadcrumbs`, `author`, `categories`, `tags`, `featured_image`, `reading_time`, `comment_count`, `meta`).

### URL Pattern

Posts are served at `/{slug}` with no `/blog/` prefix. Slugs are unique across both posts and pages within a site. `single_post` (`core/src/handlers/post.rs`) first checks whether the active Puck builder project owns a page at this slug (`page_composition::get_by_slug`) and renders it via the composer if so; otherwise it looks up the post/page and, if `post_type == "page"`, delegates to `page::render_page()`.

### AI Translation (updated 2026-09-10)

A plain post/page's `title`/`excerpt`/`content` can be AI-translated into another language and
served at `/{locale}/{slug}`. The editor deliberately separates two translation scopes:

- The globe action translates only missing or stale post prose. It is disabled, and the route
  refuses the request, when the localized post is already current.
- The layers action translates only missing or stale embedded forms and polls for a locale that
  already has a post translation. It never retranslates or rewrites post prose.

Each translated locale lists its embedded components as **Current**, **Missing**, or **Source
changed**, with links to their Designer screens. Adding, removing, or moving only an embed marker
does not make current post prose stale: the source post remains authoritative for component
placement, and public rendering reconciles those markers mechanically. A title, excerpt, content
format, or real prose change still marks the post translation stale.

See the **AI Post Translation** doc for provider configuration, the complete administrator
workflow, rendering behavior, telemetry, and troubleshooting. Builder/page-composition posts are
not covered because their content is a JSON block tree rather than the three flat post fields.

### Status Workflow

`draft` → `pending` (contributor submits; `submitted_at` is set the first time status becomes `pending`) → `published` (admin approves) or `scheduled` (`scheduled_at`/`published_at`-gated). `trashed` is a soft-delete-like status filtered out of the default "All" admin view via `ListFilter.exclude_trashed`.

### Sanitization

On create and update: `title` capped at 255 chars, `excerpt` at 500 chars (both plain strings, no explicit `clean_text` call visible in current source — truncation only). `content` is sanitized via `sanitize_content()`, which uses `ammonia::Builder::default()` with an added allowlist for `<audio>`/`<source>` tags/attributes so embedded audio players survive save/reload. Slugs default to `slugify(title)` and are capped at 200 chars.

### View Counting

`single_post` records a unique daily view for anonymous, non-bot visitors only (bot check via `is_bot()` on the User-Agent string; logged-in account users are excluded). The client IP is read from `X-Real-IP`/`X-Forwarded-For` (set by Caddy) or the socket address, then anonymized (`anonymize_ip`: zero the last IPv4 octet or last 80 bits of IPv6) before being sent through a non-blocking `state.view_buffer` `UnboundedSender` to a background flush task (post_views tracking, migration 0034).

### Saved Posts (Reading List)

Logged-in subscribers can save/unsave posts to a personal reading list (`crate::models::saved_post`, migration 0037). `render_post` looks up `is_saved` for the current session and exposes it to the template.

### Search (Admin List)

`core/src/models/post.rs::search_terms()` strips a stop-word list from admin search input before building `LOWER(title) LIKE '%term%'` clauses; empty-after-stripping input applies no filter. The admin posts list also paginates results (20 per page) and reports separate pending/scheduled counts for tab badges.

### Prev/Next and Related Posts

`render_post` fetches the chronologically previous/next published post (by `published_at`) and up to 5 related posts sharing a taxonomy term (`post::get_related`), excluding the current post.

### Post Template Override

If the active builder project has a page marked as the site's "post template" (`page_composition::get_post_template`), posts render through the Puck composer instead of `single.html`.

### Admin Editor — AJAX Save (2026-08-12)

The admin editor (`render_editor` in `admin/src/pages/posts.rs`, shared by both `new_post_type` and `edit_post_type` — same function, `action` just points at a different save URL) used to submit as a plain native `<form>`, which reloaded the whole page on every save — including tearing down and re-initializing the Quill instance. Save now goes through `fetch` instead, with **no backend changes at all**:

1. `postForm`'s `submit` handler calls `preventDefault()` and does `fetch(postForm.action, { method: 'POST', body: new FormData(postForm) })`. Using `FormData(postForm)` (rather than hand-listing fields) means every input, hidden field, and checkbox in the form is captured automatically, exactly as a native submit would send it.
2. `save_edit`/`save_new` (`core/src/handlers/admin/posts.rs`) are completely unchanged — same validation, same redirects. The frontend just decides what to *do* with the redirect it gets back, using `Response.redirected`/`.url`, the same technique the Delete button already used (`deletePostConfirm`, same file):
   - **Redirected, same path** (`save_edit`'s success case — it redirects to `/admin/{posts|pages}/{id}/edit?success=saved`, the page you're already on) — stay put. Show a transient "Saved ✓" on `.unsaved-indicator`, clear the `formDirty` flag so `beforeunload` doesn't warn, re-disable the Save button. No reload, Quill untouched, scroll position preserved.
   - **Redirected, different path** (`save_new`'s success case — it redirects to `/admin/posts` or `/admin/pages`, the list) — navigate there via `window.location.href`, same one-time transition that already happens today for a brand-new post's first save.
   - **Not redirected** (validation failure, e.g. "content required before publishing," or a DB error — both return `Html(render_editor(...))` directly with no redirect) — fall back to a real `postForm.submit()`, a genuine native submission, so the existing error-flash rendering displays exactly as it always has. This intentionally costs a second request on the rare failure path in exchange for zero duplicated error-display logic and zero risk of the AJAX path silently diverging from what a real page load would show.

Path comparison uses `pathname` only (not the full URL) — `?success=saved` being appended to the same path must not be mistaken for "landed on a different page."

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /{slug} | `post::single_post` | Render post (or delegate to page/builder) |
| POST | /{slug}/comment | `comment::submit` | Submit comment |
| POST | /{slug}/save | `post::save_post` | Save to reading list |
| POST | /{slug}/unsave | `post::unsave_post` | Remove from reading list |
| POST | /{slug}/unlock | `post_unlock::unlock_page` | Unlock password-protected post |
| GET | /admin/posts | `admin::posts::list` | Admin post list (paginated, filterable, searchable) |
| GET/POST | /admin/posts/new | `admin::posts::new_post` / `save_new` | Create post — now saved via `fetch`, see "Admin Editor — AJAX Save" above |
| GET/POST | /admin/posts/{id}/edit | `admin::posts::edit_post` / `save_edit` | Edit post — now saved via `fetch`, see "Admin Editor — AJAX Save" above |
| POST | /admin/posts/{id}/delete | `admin::posts::delete_post` | Delete post |
| POST | /admin/posts/{id}/translate | `admin::posts::translate_post_action` | Translate missing/stale post prose; refuses a current translation |
| POST | /admin/posts/{id}/translate-embeds | `admin::posts::translate_embeds_action` | Translate only missing/stale embedded forms and polls |
| POST | /admin/posts/{id}/translations/{locale}/delete | `admin::posts::delete_translation` | Delete one localized post/page copy |
| POST | /admin/posts/bulk-delete | `admin::posts::bulk_delete_posts` | Bulk delete |

## Security Notes

- `content` sanitized with an ammonia allowlist (`sanitize_content`) before storage.
- `title`/`excerpt` are length-capped but not explicitly HTML-stripped in the current model code.
- Password-protected posts require a valid signed cookie (checked by `post_unlock::is_unlocked`) before rendering.
- Post ownership/permission checks in `admin::posts` (`delete_post`, `bulk_delete_posts`): non-global-admins are scoped to their own site; authors may only delete their own, non-published posts.
- View-count IP anonymization is applied before any storage or transmission.
- The AJAX save path hits the exact same `save_edit`/`save_new` handlers, with the exact same session-cookie auth and server-side validation, as a native submit always did — it introduces no new trust boundary.

## Known Limitations / TODOs

- `admin/src/pages/posts.rs` builds admin HTML via plain Rust string-building functions (`render_list`, `render_editor`), not a Leptos/WASM UI — this page is still server-rendered on every navigation, same as the rest of the admin. Only Save itself was converted to avoid a reload (see "Admin Editor — AJAX Save"); this is not a WASM island the way the media library (`/admin/media`, see the Media Library doc) is. Worth noting for anyone expecting Leptos rendering here from other project docs.
