---
title: Pages
group: feature
updated_by: claude
last_updated: 2026-07-22
---
# Pages

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Pages are static content items (About, Contact, Privacy Policy, etc.). They share the `posts` table with `post_type = 'page'`, differentiated from Posts by having no author-only restriction, supporting hierarchical nesting via `parent_id`, custom Tera templates, and (as of the Puck visual builder work) potentially being owned by a builder page composition instead of rendering through the classic theme templates.

## How It Works

### URL Pattern

Pages live at `/{slug}` (top-level) or `/parent-slug/child-slug` (nested, via `parent_id`). Posts and pages share the same `/{slug}` namespace, so slugs must be unique across both types.

Before the classic post/page lookup runs, `single_post` (`core/src/handlers/post.rs`) checks whether the active builder project (`page_composition::get_by_slug`) owns a page composition at this slug and, if so, renders it via `composer::render_composition` instead. Otherwise, the explicit `/{slug}` route fires via `post_handler::single_post`; if the resolved `Post` record has `post_type = "page"`, the handler delegates to `page::render_page()` (`pub(super)` in `core/src/handlers/page.rs`). Nested page paths (`/a/b/c`) fall through to the `single_page` fallback handler registered last in the router, which calls `post::get_page_by_path()` to walk the parent chain segment by segment.

### Template Selection

If a page has a non-empty `template` value, `{template}.html` is used; otherwise `page.html`. The special `"feed"` template causes `page::render_page` to fetch the 20 most recent published posts (`post::list` with `PostType::Post`) and return `Content-Type: application/rss+xml`.

### Hierarchical Pages

`post::get_page_by_path` resolves multi-segment URLs by requiring the first segment to be a root page (`parent_id IS NULL`) and matching each subsequent segment as a child of the previous page. `post::get_full_page_path` and `post::get_page_breadcrumbs` build the full `/a/b/c` path and a Home → ancestors → current breadcrumb trail (used in `PostContext.breadcrumbs`) by walking `parent_id` upward.

### Admin Management

`/admin/pages/*` routes reuse the same handler functions as posts (`admin::posts::list_type`, `new_post_type`, `edit_post_type`, `bulk_delete_type`) parameterized by `post_type == "page"`. Page-only admin behavior:
- Gated behind the `admin.caps.can_manage_pages` capability — `list_pages`, `new_page`, `edit_page`, `delete_page`, and `bulk_delete_pages` all redirect to `/admin` if the current admin lacks this capability (this is stricter than Posts, which have no such capability gate).
- The editor additionally offers a **template picker** populated by `scan_templates()`, which recursively walks the active theme's `templates/` directory and excludes reserved template names (`base`, `page`, `index`, `single`, `archive`, `search`, `404`) and anything under `partials/`.
- The editor offers a **parent page selector** populated by `fetch_parent_options()`, which lists all published pages for the site (excluding the page being edited, to prevent a page becoming its own parent).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /{slug} | `post::single_post` → `page::render_page` | Top-level page (or builder composition) |
| POST | /{slug}/unlock | `post_unlock::unlock_page` | Unlock password-protected page |
| * | * (fallback) | `page::single_page` | Nested pages and unmatched paths |
| GET | /admin/pages | `admin::posts::list_pages` | Admin page list (requires `can_manage_pages`) |
| GET/POST | /admin/pages/new | `admin::posts::new_page` / `save_new` | Create page |
| GET/POST | /admin/pages/{id}/edit | `admin::posts::edit_page` / `save_edit` | Edit page |
| POST | /admin/pages/{id}/delete | `admin::posts::delete_page` | Delete page |
| POST | /admin/pages/bulk-delete | `admin::posts::bulk_delete_pages` | Bulk delete |

## Database Schema

`page_parent` (migration 0039) added the `parent_id UUID` column to `posts`, referencing `posts.id`, enabling the hierarchical nesting described above.

## Security Notes

- All page-only admin routes check `admin.caps.can_manage_pages` and redirect to `/admin` if absent.
- Password protection uses the same argon2 hash + signed-cookie mechanism as posts. Per the handler code, the password gate is applied only when `segments.len() == 1` (top-level pages) — nested page password protection is not implemented.
- Non-global-admin delete/bulk-delete is scoped to the admin's own site; authors may not delete published pages.
