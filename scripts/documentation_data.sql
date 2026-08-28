--
-- PostgreSQL database dump
--

-- Dumped from database version 13.23
-- Dumped by pg_dump version 13.23

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: documentation; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (2, 'routing', 'Routing', '# Routing

> Last updated: 2026-07-22 | Updated by: claude

## Overview

All HTTP routing is defined in `core/src/router.rs` via `build(state, session_layer) -> Router`.
Routes cover public content, admin CRUD (including the Puck visual page builder), account
management, API endpoints, static files, and dynamically registered plugin routes.

## How It Works

Global layers applied to the whole router (outermost-last): `no_store_for_protected` (adds
`Cache-Control: no-store` to all `/admin` and `/account` responses) → `maintenance_layer`
(`middleware::maintenance::gate`) → `ip_allowlist_layer` (`middleware::ip_allowlist::gate`) →
`ip_denylist_layer` (`middleware::ip_denylist::gate`) → `track_http_metrics` (increments
`synaptic_http_requests_total` / records `synaptic_http_request_duration_seconds`) →
`session_layer` (PostgreSQL-backed `tower_sessions`) → `TraceLayer`.

Static files: `/uploads/{*path}` via `uploads::serve`, `/theme/static/{*path}` via
`theme_static::serve`, `/admin/static` nest-serves the `admin/static` directory.

Plugin routes are registered dynamically from `state.plugin_routes`. Each path receives
`plugin_route::dispatch` as its GET handler, except `/sitemap.xml` which is a hardcoded route
(`plugin_route::sitemap`) and is skipped in the plugin-route registration loop.

`/{slug}/unlock` is registered as an explicit named route (must be registered before the
fallback — nested password-protected pages aren''t supported in MVP). The page fallback
(`page::single_page`) catches all unmatched paths, including nested page URLs like `/a/b/c`.

### Post and Page URL Unification

Posts and pages both live at `/{slug}` — no `/blog/` prefix. `post_handler::single_post`
resolves the slug and, if the record''s `post_type` is `page`, delegates to the page fallback
handling. The fallback also directly handles nested page paths that never match `/{slug}`.

### Page Builder Routes

The Puck visual page builder (`admin/src/pages/builder.rs`, `core/src/handlers/admin/builder.rs`)
is registered under `/admin/builder` and `/admin/builder2` (see table below) — project listing,
per-project page CRUD, homepage designation, duplication, and both `edit_page` (v1) and
`edit_page2` (v2/newer editor) views. See the dedicated `builder` doc slug for its internals
(zones, `builder_projects`/`page_compositions` models, Puck JSON schema).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /metrics | `metrics_handler::metrics` | Prometheus metrics |
| GET | / | `home::home` | Homepage |
| GET | /{slug} | `post_handler::single_post` | Public post or page view |
| POST | /{slug}/comment | `comment_handler::submit` | Submit a comment |
| POST | /{slug}/save | `post_handler::save_post` | Save post to reading list |
| POST | /{slug}/unsave | `post_handler::unsave_post` | Remove post from reading list |
| POST | /{slug}/unlock | `post_unlock::unlock_page` | Unlock password-protected post/page |
| GET | /category/{slug} | `archive::category_archive` | Category archive |
| GET | /tag/{slug} | `archive::tag_archive` | Tag archive |
| GET | /author/{username} | `archive::author_archive` | Author archive |
| GET | /search | `search::search` | Full-text search |
| GET | /sitemap.xml | `plugin_route::sitemap` | XML sitemap |
| POST | /form/{name} | `form_handler::submit` | Public form submission |
| GET/POST | /subscribe | `subscribe::subscribe_form / subscribe_post` | Subscriber signup |
| GET/POST | /login | `auth::public_login_form / public_login_post` | Public login |
| GET/POST | /admin/login | `auth::login_form / login_post` | Admin login |
| GET | /admin/logout | `auth::logout` | Admin logout |
| GET | /account | `account::dashboard` | Account area |
| GET | /account/profile | `account::profile_view` | Account profile |
| POST | /account/profile/update | `account::profile_update` | Update account profile |
| POST | /account/profile/change-password | `account::profile_change_password` | Change account password |
| GET | /account/saved-posts | `account::saved_posts` | Subscriber saved posts |
| GET | /account/my-comments | `account::my_comments` | Subscriber comment history |
| POST | /account/comments/{id}/delete | `account::delete_comment` | Delete own comment |
| GET | /account/logout | `auth::account_logout` | Account logout |
| GET | /admin/profile | `profile::view` | Admin profile |
| POST | /admin/profile/update | `profile::update_profile` | Update admin profile |
| POST | /admin/profile/change-password | `profile::change_password` | Change admin password |
| GET | /admin | `dashboard::dashboard` | Dashboard |
| GET | /admin/posts | `posts::list` | Posts list |
| GET/POST | /admin/posts/new | `posts::new_post / save_new` | New post |
| GET/POST | /admin/posts/{id}/edit | `posts::edit_post / save_edit` | Edit post |
| POST | /admin/posts/{id}/delete | `posts::delete_post` | Delete post |
| POST | /admin/posts/bulk-delete | `posts::bulk_delete_posts` | Bulk delete posts |
| POST | /admin/comments/{id}/delete | `admin_comments::delete` | Admin delete comment |
| GET | /admin/pages | `posts::list_pages` | Pages list |
| GET/POST | /admin/pages/new | `posts::new_page / save_new` | New page |
| GET/POST | /admin/pages/{id}/edit | `posts::edit_page / save_edit` | Edit page |
| POST | /admin/pages/{id}/delete | `posts::delete_page` | Delete page |
| POST | /admin/pages/bulk-delete | `posts::bulk_delete_pages` | Bulk delete pages |
| GET | /admin/api/media | `media::api_list` | Media JSON API |
| POST | /admin/api/media/{id}/meta | `media::api_update_meta` | Update media metadata (JSON) |
| POST | /admin/api/media/{id}/folder | `media::api_update_folder` | Move media to folder (JSON) |
| GET | /admin/media | `media::list` | Media library |
| POST | /admin/media/upload | `upload::upload` | Upload media (body-limited) |
| POST | /admin/media/folders/new | `media::create_folder` | New media folder |
| POST | /admin/media/folders/{id}/delete | `media::delete_folder` | Delete media folder |
| POST | /admin/media/{id}/delete | `media::delete` | Delete media |
| GET | /admin/categories | `taxonomy::categories` | Categories list |
| POST | /admin/categories/new | `taxonomy::create` | New category |
| POST | /admin/categories/{id}/delete | `taxonomy::delete_category` | Delete category |
| GET | /admin/tags | `taxonomy::tags` | Tags list |
| POST | /admin/tags/new | `taxonomy::create` | New tag |
| POST | /admin/tags/{id}/delete | `taxonomy::delete_tag` | Delete tag |
| GET | /admin/users | `users::list` | Users list (paginated, live search) |
| GET/POST | /admin/users/new | `users::new_user / save_new` | New user |
| GET/POST | /admin/users/{id}/edit | `users::edit_user / save_edit` | Edit user |
| POST | /admin/users/{id}/delete | `users::delete_user` | Delete user |
| POST | /admin/users/bulk-delete | `users::bulk_delete_users` | Bulk delete users |
| GET | /admin/users/{id}/site-access | `users::site_access_page` | Manage a user''s site access |
| POST | /admin/users/{id}/site-access/add | `users::add_site_access` | Grant site access |
| POST | /admin/users/{id}/site-access/remove | `users::remove_site_access` | Revoke site access |
| GET | /admin/documentation | `admin_documentation::list` | In-app documentation viewer |
| GET | /admin/themes | `themes::list` | Themes list |
| POST | /admin/themes/activate | `themes::activate` | Activate theme |
| POST | /admin/themes/get-theme | `themes::get_theme` | Fetch theme data (AJAX) |
| POST | /admin/themes/publish-theme | `themes::publish_theme` | Publish theme changes |
| POST | /admin/themes/delete | `themes::delete` | Delete theme |
| POST | /admin/themes/upload | `themes::upload_theme` | Upload theme zip (body-limited) |
| GET | /admin/theme-screenshot/{theme_name} | `themes::screenshot` | Theme screenshot image |
| GET/POST | /admin/themes/create | `themes::create_form / create_theme` | Create new theme |
| GET | /admin/themes/editor/{theme} | `themes::edit_file` | Theme file editor |
| POST | /admin/themes/editor/{theme}/save | `themes::save_file` | Save theme file |
| POST | /admin/themes/editor/{theme}/restore | `themes::restore_file` | Restore theme file |
| POST | /admin/themes/editor/{theme}/new-file | `themes::new_file` | New theme file |
| POST | /admin/themes/editor/{theme}/delete-file | `themes::delete_file` | Delete theme file |
| GET | /admin/builder | `admin_builder::list` | Builder projects list |
| POST | /admin/builder/create | `admin_builder::create_project` | Create builder project |
| POST | /admin/builder/deactivate | `admin_builder::deactivate_project` | Deactivate builder project |
| POST | /admin/builder/save | `admin_builder::save` | Save page composition |
| POST | /admin/builder/publish | `admin_builder::publish` | Publish page composition |
| GET | /admin/builder/load/{id} | `admin_builder::load` | Load composition JSON |
| GET | /admin/builder/{project_id} | `admin_builder::project_pages` | Project''s pages list |
| POST | /admin/builder/{project_id}/rename | `admin_builder::rename_project` | Rename project |
| POST | /admin/builder/{project_id}/activate | `admin_builder::activate_project` | Activate project |
| POST | /admin/builder/{project_id}/delete | `admin_builder::delete_project` | Delete project |
| GET/POST | /admin/builder/{project_id}/pages/new | `admin_builder::new_page_form / create_page` | New builder page |
| GET | /admin/builder/{project_id}/pages/{page_id} | `admin_builder::edit_page` | Edit builder page (v1) |
| GET | /admin/builder2/{project_id}/pages/{page_id} | `admin_builder::edit_page2` | Edit builder page (v2) |
| POST | /admin/builder/{project_id}/pages/{page_id}/set-homepage | `admin_builder::set_homepage` | Set page as homepage |
| POST | /admin/builder/{project_id}/pages/{page_id}/duplicate | `admin_builder::duplicate_page` | Duplicate page |
| POST | /admin/builder/{project_id}/pages/{page_id}/delete | `admin_builder::delete_page` | Delete builder page |
| GET/POST | /admin/menus | `admin_menus::list / create` | Nav menu list and create |
| GET/POST | /admin/menus/{id} | `admin_menus::edit / update` | Edit menu settings |
| POST | /admin/menus/{id}/delete | `admin_menus::delete` | Delete menu |
| POST | /admin/menus/{id}/items/new | `admin_menus::add_item` | Add menu item |
| POST | /admin/menus/{id}/items/{item_id}/edit | `admin_menus::edit_item` | Edit menu item |
| POST | /admin/menus/{id}/items/{item_id}/delete | `admin_menus::delete_item` | Delete menu item |
| GET/POST | /admin/settings | `settings::settings / save_settings` | System settings |
| GET/POST | /admin/sites | `admin_sites::list / create` | Sites list and create |
| GET | /admin/sites/go-home | `admin_sites::go_home` | Return to default site |
| GET | /admin/sites/new | `admin_sites::new_site` | New site form |
| POST | /admin/sites/switch | `admin_sites::switch` | Switch active site |
| GET | /admin/sites/{id}/settings | `admin_sites::site_settings` | Per-site settings (incl. maintenance, IP lists) |
| POST | /admin/sites/{id}/site-config | `admin_sites::save_site_config` | Save per-site config |
| POST | /admin/sites/{id}/delete | `admin_sites::delete` | Delete site |
| POST | /admin/sites/{id}/provision-ssl | `admin_sites::provision_ssl` | Provision SSL for site |
| GET | /admin/forms | `admin_forms::list_forms` | Forms list |
| GET | /admin/forms/{name} | `admin_forms::view_form` | View form submissions |
| POST | /admin/forms/{name}/{id}/delete | `admin_forms::delete_submission` | Delete submission |
| POST | /admin/forms/{name}/delete-all | `admin_forms::delete_all` | Delete all submissions |
| GET | /admin/forms/{name}/export | `admin_forms::export_csv` | Export submissions CSV |
| POST | /admin/forms/{name}/toggle-block | `admin_forms::toggle_block` | Block/unblock a form |
| GET | /uploads/{*path} | `uploads::serve` | Serve uploaded media |
| GET | /theme/static/{*path} | `theme_static::serve` | Serve theme static assets |
| — | /admin/static/* | `ServeDir` | Admin static assets (nested service) |
| GET | (dynamic) | `plugin_route::dispatch` | Plugin-registered routes |
| * | * (fallback) | `page::single_page` | Nested page paths and unmatched page slugs |

Admin plugins routes (`/admin/plugins/*`) remain commented out / disabled pre-launch.

## Security Notes

- `maintenance_layer`, `ip_allowlist_layer`, and `ip_denylist_layer` gate every request
  site-wide (see the `middleware` doc) before it reaches route handlers.
- `/admin/*` and `/account/*` responses always get `Cache-Control: no-store`.', '2026-07-22 19:51:43.816454-04', 'claude', 'system');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (3, 'database', 'Database Schema', '# Database Schema

> Last updated: 2026-08-18 | Updated by: claude

## Overview

The database is PostgreSQL. All schema changes are applied via numbered SQL migration files in
`migrations/`. Migrations are embedded into the binary at compile time via `sqlx::migrate!()`
and run automatically on startup or via `synap migrate`. There are 62 migrations in the
current codebase (up from 47).

## How It Works

Each migration is a plain SQL file named `NNNN_description.sql`. SQLx tracks applied migrations
in `_sqlx_migrations`. Migrations are strictly additive — earlier files are never modified. The
`synap install` and `synap migrate` commands both call
`sqlx::migrate!("../migrations")`. Because migrations are embedded at compile time, adding a new
migration file requires rebuilding the binary.

Per-site feature toggles introduced recently (maintenance mode, IP allow/deny lists) do **not**
have their own tables — they''re stored as rows in the existing generic `site_settings`
key/value table (keys: `maintenance_mode`, `maintenance_message`, `ip_allowlist_enabled`,
`ip_allowlist`, `ip_denylist_enabled`, `ip_denylist`), so no migration was required for them.

## Database Schema

### Core Tables (0001–0033, unchanged since last doc pass)

**users** (0001, extended by 0012, 0013, 0016, 0018)
- `id UUID PK`, `username TEXT UNIQUE`, `email TEXT UNIQUE`, `display_name TEXT`,
  `password_hash TEXT`, `bio TEXT`, `avatar_media_id UUID`, `role TEXT`
  (subscriber/author/editor/site_admin/super_admin), `is_active BOOL`, `is_protected BOOL`,
  `deleted_at TIMESTAMPTZ` (soft-delete), `default_site_id UUID`

**media** (0002, extended by 0024, 0036)
- `id UUID PK`, `site_id UUID`, `filename TEXT`, `mime_type TEXT`, `path TEXT`, `alt_text TEXT`,
  `title TEXT`, `caption TEXT`, `width INT`, `height INT`, `file_size BIGINT`, `uploaded_by UUID`,
  `folder_id UUID` (0036, FK to `media_folders`, `ON DELETE SET NULL`)

**posts** (0003, extended by 0005, 0010, 0019, 0025–0028, 0039)
- `id UUID PK`, `site_id UUID`, `title TEXT`, `slug TEXT`, `content TEXT`,
  `content_format TEXT` (html/markdown), `excerpt TEXT`,
  `status TEXT` (draft/pending/published/scheduled/trashed), `post_type TEXT` (post/page),
  `author_id UUID`, `featured_image_id UUID`, `published_at TIMESTAMPTZ`, `template TEXT`,
  `post_password TEXT`, `submitted_at TIMESTAMPTZ`, `comments_enabled BOOL`,
  `parent_id UUID` (self-referencing FK `ON DELETE SET NULL` — pages only, page hierarchy)

**taxonomies** (0004, extended by 0038 unique-name constraint)
- `id UUID PK`, `site_id UUID`, `name TEXT`, `slug TEXT`, `taxonomy TEXT` (category/tag),
  `description TEXT`. `UNIQUE (site_id, name, taxonomy)` added in 0038 so neither name nor slug
  can be duplicated within a site/type.

**post_meta** (0005) — `post_id UUID`, `meta_key TEXT`, `meta_value TEXT` — composite PK
`(post_id, meta_key)`

**site_settings** (0006, restructured by 0014) — `site_id UUID`, `key TEXT`, `value TEXT`. Generic
per-site KV store; now also holds maintenance-mode and IP allow/deny-list settings (see above).

**tower_sessions** (0007) — tower-sessions PostgreSQL store

**sites** (0008, extended by 0015) — `id UUID PK`, `hostname TEXT UNIQUE`, `owner_user_id UUID`

**site_users** (0009, PK changed by 0062) — `id UUID PK` (surrogate, added 0062, 2026-08-18),
`site_id UUID`, `user_id UUID`, `role TEXT` (admin/editor/author/subscriber — never
super_admin/site_admin, see 0062 note below), `invited_by UUID` — `UNIQUE (site_id, user_id,
role)` (0062; was `PRIMARY KEY (site_id, user_id)` before, which capped a user to exactly one
role per site)

**post_taxonomies** — `post_id UUID`, `taxonomy_id UUID` — composite PK

**form_submissions** (0020) — `id UUID PK`, `site_id UUID`, `form_name TEXT`, `data JSONB`,
`ip_address TEXT`, `read_at TIMESTAMPTZ`, `submitted_at TIMESTAMPTZ`

**form_blocks** (0021) — `site_id UUID`, `form_name TEXT` — composite PK. Presence of a row
blocks `POST /form/{name}` for that site (silent redirect to `?blocked=1`).

**app_settings** (0022) — `key TEXT PK`, `value TEXT` — installation-level settings (app_name,
timezone, max_upload_mb)

**site_plugins** (0023) — `site_id UUID`, `plugin_name TEXT`, `active BOOL`,
`installed_at TIMESTAMPTZ` — composite PK

**comments** (0028, extended by 0029–0031) — `id UUID PK`, `post_id UUID`, `site_id UUID`,
`author_id UUID`, `parent_id UUID`, `body TEXT` (1–400 chars, tightened from 2000 in 0029),
`deleted_at TIMESTAMPTZ` (soft-delete), `ip_address TEXT` (0031)

**documentation** (0032, extended by 0033) — `id SERIAL PK`, `slug VARCHAR UNIQUE`,
`title VARCHAR`, `content TEXT`, `grp VARCHAR`, `last_updated TIMESTAMPTZ`, `updated_by VARCHAR`

### Tables added since the last doc pass (0034–0047)

**post_views** (0034) — `post_id UUID`, `ip_hash TEXT` (anonymized IP), `viewed_date DATE` —
composite PK `(post_id, ip_hash, viewed_date)`. One row per visitor per post per day, used for
view-count analytics without storing raw IPs. Indexed on `post_id`.

**media_folders** (0035) — `id UUID PK`, `site_id UUID` (FK `sites`, cascade), `name TEXT`,
`created_at TIMESTAMPTZ`. `UNIQUE (site_id, name)`. Referenced by `media.folder_id` (0036).

**saved_posts** (0037) — `user_id UUID`, `post_id UUID`, `site_id UUID`, `saved_at TIMESTAMPTZ` —
composite PK `(user_id, post_id)`. Lets subscribers bookmark posts. Indexed on
`(user_id, site_id)`.

**nav_menus** (0040)
- `id UUID PK`, `site_id UUID NOT NULL REFERENCES sites ON DELETE CASCADE`, `name TEXT NOT NULL`,
  `location TEXT` (NULL = name-only, `''primary''`/`''footer''` = auto-loaded into template
  context), `created_at TIMESTAMPTZ`, `updated_at TIMESTAMPTZ`
- `UNIQUE (site_id, name)` — menu names unique per site
- Location uniqueness (one menu per location per site) enforced at the application layer, not DB

**nav_menu_items** (0040)
- `id UUID PK`, `menu_id UUID NOT NULL REFERENCES nav_menus ON DELETE CASCADE`,
  `parent_id UUID REFERENCES nav_menu_items ON DELETE CASCADE` (self-referencing, dropdown
  nesting), `sort_order INT DEFAULT 0`, `label TEXT NOT NULL`, `url TEXT` (ignored when
  `page_id` set), `page_id UUID REFERENCES posts ON DELETE SET NULL`, `target TEXT DEFAULT
  ''_self''`, `created_at TIMESTAMPTZ`
- Indexed on `menu_id` and `parent_id`

**page_compositions** (0041, extended by 0042, 0044, 0045, 0046→0047 reverted) — backs the Puck
visual page builder''s per-page content:
- `id UUID PK`, `site_id UUID` (FK `sites`, cascade), `name VARCHAR(255)`,
  `composition JSONB DEFAULT ''{}''` (live/published content), `is_homepage BOOL`,
  `created_by UUID`, `created_at`/`updated_at TIMESTAMPTZ`
- `project_id UUID REFERENCES builder_projects ON DELETE CASCADE` (0042) — links a composition
  to a project
- `slug VARCHAR(100)`, `page_type VARCHAR(20) DEFAULT ''page''` (0044) — homepage slug is always
  `/`; unique index `(project_id, slug)` where `slug IS NOT NULL`
- `draft_composition JSONB DEFAULT ''{}''` (0045) — separate work-in-progress column so in-flight
  edits don''t clobber the published `composition` until Publish; seeded from `composition` on
  migration
- `is_post_template BOOLEAN` was added in 0046 and then dropped again in 0047 (net no-op — the
  flag was added and removed across two migrations without ever being used)
- Unique index `page_compositions_homepage_idx` ensures only one `is_homepage = TRUE` row per
  site
- Indexed on `site_id` and `project_id`
- Full internals of the composition JSON schema are documented in the `builder` doc slug

**builder_projects** (0042, extended by 0043) — a named collection of builder pages/masters per
site:
- `id UUID PK`, `site_id UUID` (FK `sites`, cascade), `name VARCHAR(35)` (widened then narrowed:
  originally `VARCHAR(255)`, tightened to 35 chars in 0043), `description VARCHAR(100)`
  (originally unbounded `TEXT`, capped in 0043), `is_active BOOL`, `created_by UUID`,
  `created_at`/`updated_at TIMESTAMPTZ`
- Unique partial index ensures only one `is_active = TRUE` project per site
- Indexed on `site_id`

### Columns added by migrations 0060–0061 (2026-08-16)

(Migrations 0048–0059 in between — `forms`/`mail_log`/`email_providers` and related tables —
predate this doc''s last full pass; see the **Form Designer**, **Forms**, and **Email Providers**
docs for those in full instead of this file.)

**form_submissions.form_id** (0060) — `UUID NULL REFERENCES forms(id) ON DELETE SET NULL`,
backfilled for existing rows. Gives submissions an exact FK to the form that collected them,
alongside the pre-existing `form_name` (slug) text match, which stays the authoritative lookup
key everywhere (it''s immutable and still resolves even for orphaned/pre-FK rows where `form_id`
is NULL). See the **Forms** doc.

**forms.total_submissions** (0061) — `BIGINT NOT NULL DEFAULT 0`, backfilled from existing
`form_submissions` counts via the new `form_id` FK. A lifetime counter incremented once per
submission and never decremented, kept deliberately separate from the live submission count used
for the Submissions tab''s pagination — see the **Form Designer** doc.

### site_users PK widened for multi-role (0062, 2026-08-18)

`site_users`'' `PRIMARY KEY (site_id, user_id)` was replaced with a surrogate `id UUID` PK plus
`UNIQUE(site_id, user_id, role)`, so a user can now hold more than one role on the same site
(previously exactly one, enforced by the old composite PK). The `role` CHECK constraint itself
(`admin | editor | author | subscriber`) was deliberately left unchanged — it never permitted
`super_admin`/`site_admin` and still doesn''t; a matching guarantee was added independently at
the Rust type level via a `SiteRole` enum with no such variant. See the **Users & Roles** doc''s
"Multiple roles per user per site" section for the full session/login-picker flow this enabled.

## Known Limitations / TODOs

Because `sqlx::migrate!()` embeds migrations at compile time, adding a new migration file
requires rebuilding the binary. Running an old binary against a DB with newer migrations applied
will fail at startup.

0046/0047 illustrate that a migration is not the place to experiment — `is_post_template` was
added and dropped again within two consecutive migrations, meaning both are effectively dead
weight in the migration history (harmless, but worth knowing when reading migration history for
context on `page_compositions`).
', '2026-08-18 09:09:14.17626-04', 'claude', 'system');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (9, 'posts', 'Posts', '# Posts

> Last updated: 2026-08-12 | Updated by: claude

## Overview

Posts are the primary content type. They are authored, optionally submitted for review, scheduled, and published by admin users. Each post belongs to a site and supports categories, tags, a featured image, comments, password protection, a saved/reading-list feature, and per-post view tracking. Posts share the `posts` table with Pages (`post_type = ''post''`).

## How It Works

### Data Model (`core/src/models/post.rs`)

The `Post` struct''s key columns: `id`, `site_id`, `title`, `slug`, `content`, `content_format`, `excerpt`, `status` (`draft`, `pending`, `published`, `scheduled`, `trashed`), `post_type` (`post` or `page`), `author_id`, `featured_image_id`, `published_at`, `scheduled_at`, `submitted_at`, `template`, `post_password` (argon2 hash), `comments_enabled`, `parent_id`. `PostContext` is the Tera-facing view (adds `url`, `breadcrumbs`, `author`, `categories`, `tags`, `featured_image`, `reading_time`, `comment_count`, `meta`).

### URL Pattern

Posts are served at `/{slug}` with no `/blog/` prefix. Slugs are unique across both posts and pages within a site. `single_post` (`core/src/handlers/post.rs`) first checks whether the active Puck builder project owns a page at this slug (`page_composition::get_by_slug`) and renders it via the composer if so; otherwise it looks up the post/page and, if `post_type == "page"`, delegates to `page::render_page()`.

### Status Workflow

`draft` → `pending` (contributor submits; `submitted_at` is set the first time status becomes `pending`) → `published` (admin approves) or `scheduled` (`scheduled_at`/`published_at`-gated). `trashed` is a soft-delete-like status filtered out of the default "All" admin view via `ListFilter.exclude_trashed`.

### Sanitization

On create and update: `title` capped at 255 chars, `excerpt` at 500 chars (both plain strings, no explicit `clean_text` call visible in current source — truncation only). `content` is sanitized via `sanitize_content()`, which uses `ammonia::Builder::default()` with an added allowlist for `<audio>`/`<source>` tags/attributes so embedded audio players survive save/reload. Slugs default to `slugify(title)` and are capped at 200 chars.

### View Counting

`single_post` records a unique daily view for anonymous, non-bot visitors only (bot check via `is_bot()` on the User-Agent string; logged-in account users are excluded). The client IP is read from `X-Real-IP`/`X-Forwarded-For` (set by Caddy) or the socket address, then anonymized (`anonymize_ip`: zero the last IPv4 octet or last 80 bits of IPv6) before being sent through a non-blocking `state.view_buffer` `UnboundedSender` to a background flush task (post_views tracking, migration 0034).

### Saved Posts (Reading List)

Logged-in subscribers can save/unsave posts to a personal reading list (`crate::models::saved_post`, migration 0037). `render_post` looks up `is_saved` for the current session and exposes it to the template.

### Search (Admin List)

`core/src/models/post.rs::search_terms()` strips a stop-word list from admin search input before building `LOWER(title) LIKE ''%term%''` clauses; empty-after-stripping input applies no filter. The admin posts list also paginates results (20 per page) and reports separate pending/scheduled counts for tab badges.

### Prev/Next and Related Posts

`render_post` fetches the chronologically previous/next published post (by `published_at`) and up to 5 related posts sharing a taxonomy term (`post::get_related`), excluding the current post.

### Post Template Override

If the active builder project has a page marked as the site''s "post template" (`page_composition::get_post_template`), posts render through the Puck composer instead of `single.html`.

### Admin Editor — AJAX Save (2026-08-12)

The admin editor (`render_editor` in `admin/src/pages/posts.rs`, shared by both `new_post_type` and `edit_post_type` — same function, `action` just points at a different save URL) used to submit as a plain native `<form>`, which reloaded the whole page on every save — including tearing down and re-initializing the Quill instance. Save now goes through `fetch` instead, with **no backend changes at all**:

1. `postForm`''s `submit` handler calls `preventDefault()` and does `fetch(postForm.action, { method: ''POST'', body: new FormData(postForm) })`. Using `FormData(postForm)` (rather than hand-listing fields) means every input, hidden field, and checkbox in the form is captured automatically, exactly as a native submit would send it.
2. `save_edit`/`save_new` (`core/src/handlers/admin/posts.rs`) are completely unchanged — same validation, same redirects. The frontend just decides what to *do* with the redirect it gets back, using `Response.redirected`/`.url`, the same technique the Delete button already used (`deletePostConfirm`, same file):
   - **Redirected, same path** (`save_edit`''s success case — it redirects to `/admin/{posts|pages}/{id}/edit?success=saved`, the page you''re already on) — stay put. Show a transient "Saved ✓" on `.unsaved-indicator`, clear the `formDirty` flag so `beforeunload` doesn''t warn, re-disable the Save button. No reload, Quill untouched, scroll position preserved.
   - **Redirected, different path** (`save_new`''s success case — it redirects to `/admin/posts` or `/admin/pages`, the list) — navigate there via `window.location.href`, same one-time transition that already happens today for a brand-new post''s first save.
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
', '2026-08-12 18:13:00.325443-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (13, 'categories', 'Categories', '# Categories

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Categories are a taxonomy type used to classify posts. They are stored in the `taxonomies` table with `taxonomy = ''category''`. Each category has a name, a URL-safe slug, and an optional description. Categories are site-scoped and can be assigned to multiple posts; posts can have multiple categories.

## How It Works

### Model (`core/src/models/taxonomy.rs`)

Key structs:
- `Taxonomy` — DB row: `id`, `site_id`, `name`, `slug`, `taxonomy`, `description`, `created_at`
- `TermContext` — template-safe view with `id`, `name`, `slug`, `taxonomy`, `url` (e.g. `{base_url}/category/{slug}`), `post_count`
- `CreateTaxonomy` — input struct

Key functions: `create`, `get_by_id`, `get_by_slug`, `list` (filtered by taxonomy type and site), `for_post`, `attach_to_post`, `detach_from_post`, `post_count`, `delete`.

### Admin Handler (`core/src/handlers/admin/taxonomy.rs`)

- `categories` — lists all categories for the site with their published post counts. Requires `can_manage_taxonomies`.
- `create` — accepts `TermForm` with `name`, optional `slug`, and `taxonomy`. Validates slug via `is_valid_slug`. Handles duplicate-key errors with a user-friendly message.

The "Add Category" form (`admin/src/pages/taxonomy.rs::render`, updated 2026-07-22) is wrapped in the same `.profile-container` card used on `/admin/users/new`, the submit button stays disabled until Name has non-whitespace text, and the Slug field is live-filled from Name as you type (client-side `toSlug()`, mirroring the server''s `crate::utils::slugify::slugify`) rather than only being generated server-side on submit — editing Slug directly stops the auto-sync. The heading/button read "Add Category" (previously "Add New Categories").
- `delete_category` — enforces site ownership for non-global-admins before deletion.

Slug validation is performed by `crate::utils::slugify::is_valid_slug` — slugs must be lowercase letters, numbers, and hyphens only.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/categories | `taxonomy::categories` | List categories |
| POST | /admin/categories/new | `taxonomy::create` | Create category |
| POST | /admin/categories/{id}/delete | `taxonomy::delete_category` | Delete category |
| GET | /category/{slug} | `archive::category_archive` | Public category archive |

## Database Schema

`taxonomies` table: `id UUID PK`, `site_id UUID`, `name TEXT`, `slug TEXT`, `taxonomy TEXT` (category/tag), `description TEXT`, `created_at TIMESTAMPTZ`.

`post_taxonomies` join table: `post_id UUID`, `taxonomy_id UUID` — composite PK. Insert is idempotent via `ON CONFLICT DO NOTHING`.

## Security Notes

Only editors and above (`can_manage_taxonomies`) can manage categories. Site isolation: non-global-admins can only delete categories that belong to their current site.', '2026-07-22 19:39:08.487281-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (12, 'media', 'Media Library', '# Media Library

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

`Media::url(base_url)` builds the public URL. For real hostnames (containing a `.`, not `localhost`) it emits a **bare-filename path**, `/uploads/{filename}` — no hostname or UUID segment. Production Caddy scopes each site''s own `/uploads/*` root to that site''s symlinked upload folder (`uploads/{hostname}/` → `uploads/{site_uuid}/`, maintained by `ensure_hostname_symlink`), so the site is already implied by which domain served the request; repeating it in the path would be redundant. For local/dev hosts it falls back to the raw UUID-based `path`.

**Every other place that reconstructs an `/uploads/` URL from a stored `Media` row must follow this same bare-filename convention** — it is not automatic just because `Media::url()` does the right thing. As of 2026-08-12 this includes: the admin media grid (`list()` and `api_grid()` in `core/src/handlers/admin/media.rs`), the media-app WASM detail-panel sync (`media-app/src/window_items.rs`), and the post editor''s featured-image preview reconstruction (`edit_post_type` in `core/src/handlers/admin/posts.rs`). Each of these strips the UUID prefix (`path.splitn(2, ''/'').nth(1)`) rather than using the raw stored `path`. If a future call site builds an `/uploads/` URL some other way, it will silently produce broken images once accessed through Caddy in production — there is no single shared helper enforcing this yet; it''s currently a convention each call site has to follow by hand.

Key functions: `create`, `get_by_id`, `update_media_meta`, `unassign_folder`, `delete`, `list` (filterable by `site_id`, `uploaded_by`, and `folder_id`), `count`.

`MediaFolder` (`core/src/models/media_folder.rs`): `id`, `site_id`, `name`, `created_at`. `list`, `create`, `delete` are all scoped to a `site_id`.

### Upload Handler (`core/src/handlers/admin/upload.rs`)

`POST /admin/media/upload` — multipart form accepting `file`, optional `alt_text`, optional `folder_id`, and an internal `redirect` field (validated to start with `/admin/`). Generates a URL-safe stored filename: `{slugified-stem-capped-at-80-chars}-{8-char-uuid}.{ext}`. Writes bytes to a **per-site subdirectory** (`{uploads_dir}/{site_uuid}/`), creating it if needed; falls back to the flat uploads dir (with a warning) if no `site_id` is present. Reads image dimensions directly from the in-memory bytes via `imagesize::blob_size` for `image/*` MIME types. Body size is capped by `DefaultBodyLimit::max(config.max_upload_mb * 1MB)` (default 25 MB, configurable via app settings).

The media-app WASM island uploads to this same endpoint via `XMLHttpRequest` (not `fetch`) specifically to get `xhr.upload.onprogress` events for a real progress percentage on the dropzone — `fetch` has no upload-progress API.

### Admin Media Handler (`core/src/handlers/admin/media.rs`)

- `list` — the original server-rendered page. Still renders the initial HTML shell (SSR fallback) that the WASM island mounts into and takes over — folder list, type tabs, upload form, grid/list containers, and pagination/footer all keep their existing element IDs so the island can find and replace their contents on load. Authors (`site_role == "author"`) see only their own uploads; supports a `picker`/`browser` query mode for embedding in the rich-text/image picker (loaded in an iframe — see below).
- `api_grid` (`GET /admin/api/media/grid`) — **the WASM island''s actual data source.** JSON response: `items` (id/filename/type/isImage/path/alt/title/caption/size/dims/uploader/uploaded_at/folder_id — the same shape as the legacy embedded `ITEMS` array), `folders`, `type_counts`, `total`/`page`/`page_size`/`total_pages`. Takes `folder_id`/`type`/`page` query params. Mirrors `list()`''s filter/pagination logic but returns data instead of HTML, so folder/type/page changes can happen client-side without a full page reload.
- `delete` — enforces site ownership (403 if `media.site_id != admin.site_id` for non-global-admins) and author-only restriction (403 if an author tries to delete another user''s upload); removes the file from disk then the DB row.
- `api_list` (`GET /admin/api/media`) — JSON array of the caller''s accessible images (`image/` MIME prefix only), up to 500 items, used by the rich text editor''s image picker. Distinct from `api_grid` — this one is unfiltered by type/folder and images-only, built for the Quill inline-image picker rather than the media library grid.
- `api_update_meta` (`POST /admin/api/media/{id}/meta`) — JSON body `alt_text`/`title`/`caption`, sanitized via `sanitize_media_text`; enforces site ownership.
- `api_update_folder` (`POST /admin/api/media/{id}/folder`) — assigns/clears a media item''s folder; verifies both the media item and the target folder belong to the caller''s site before allowing the change.
- `create_folder` (`POST /admin/media/folders/new`) — folder name sanitized to alphanumerics/hyphens, 4–25 chars. Called from the WASM island via `fetch`, not a real form submit (see below).
- `delete_folder` (`POST /admin/media/folders/{id}/delete`) — optionally cascades to delete all media files/rows in the folder (`delete_media=true`), otherwise just unassigns the folder from its media so items fall back to "All Media". Also called via `fetch` from the island.

### The WASM Island (`media-app/` crate)

A separate workspace crate, compiled to `wasm32-unknown-unknown` and loaded via `wasm-bindgen` (`target: web`) from a `<script type="module">` at the bottom of the media library page. Built with Leptos in **CSR mode** (`features = ["csr"]`), not SSR/hydrate — there''s no server-rendered tree to match, so it just mounts fresh into a handful of specific element IDs in the existing page and replaces their contents:

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

All components share one set of signals (`media-app/src/state.rs`: `folder_id`, `type_filter`, `page`, `grid`, `loading`, plus per-modal `show_*`/error signals) via a `thread_local!` — changing one (e.g. clicking a type tab) triggers `state::refresh()`, which re-fetches `/admin/api/media/grid` and updates every mounted component reactively, even though they''re mounted as separate trees rather than one shared component tree.

**New folder / Delete folder are owned entirely by the island**, not legacy JS. `NewFolderModal`/`DeleteFolderModal` POST to `create_folder`/`delete_folder` (above) via `gloo_net`, then call `state::refresh()` in place — no `window.location.reload()`, unlike the original hand-written versions. Deleting a folder always resets to "All Media" afterward (`state::set_folder(None)`), since the delete button is only reachable while that folder is the one currently selected. This also fixed two bugs that existed under the old server-rendered version: the delete-confirmation message''s file count and the search-clear footer text were both frozen at page-load time (`FOLDER_TOTAL`/`{footer_info}` baked into the initial HTML) and would go stale the moment folder switching stopped triggering a full reload; both now read live state instead.

**Bridging to the legacy (pre-WASM) JS**: the item detail panel and bulk select/move/delete are still the original hand-written JS in `media.rs`''s inline `<script>` — that part was not rewritten. That script reads two globals, `window.ITEMS` and `window.FOLDERS`, indexed by array position (`data-idx` DOM attributes) to know what the user clicked. Two things make this work correctly with the island:
1. The legacy script''s own `<script>` block assigns to these via `window.ITEMS = ...` / `window.FOLDERS = ...`, **not** `var ITEMS = ...`. This is deliberate — `var` inside the script''s IIFE would make them closure-local, invisible to the WASM module''s own `window.ITEMS = ...` reassignments on every refresh (this was a real, hard-to-spot bug: the legacy JS silently kept reading a frozen page-load snapshot for a while before this was caught).
2. `media-app/src/window_items.rs` (`sync_items`/`sync_folders`) rewrites both globals every time the island fetches new data, in the exact array order the DOM was just rendered in, and rewrites `path` with the `/uploads/` prefix baked in (the grid''s own Rust-rendered `<img>` tags add that prefix themselves; the legacy detail panel does not, so it has to already be present in the value it reads).

Rendered items keep the same `data-idx`/`data-type`/`data-name`/`onclick="selectItem(this)"` attributes the legacy JS expects, so the bridge is invisible from that side — it just looks like a normal DOM node to click on.

Bulk move/delete (still legacy JS) originally ended with `window.location.reload()`. Now that folder/filter switching doesn''t reload the page, that stood out as a jarring full-page flash by comparison — both now call `window.mediaAppRefresh()` instead (a `#[wasm_bindgen]`-exported `refresh_grid()`, assigned to that name once the module loads), which re-fetches in place. They also now explicitly `selected.clear()` afterward — the bulk-selection Set stores array *positions*, and since the grid can reorder/shrink after a refresh, leftover stale positions from before a move could otherwise cause a later action to silently operate on the wrong item.

**Initial paint — avoiding a flash of empty content**: `mount()` is `async` and `await`s one grid fetch (`state::initial_load()`) *before* mounting any component, rather than mounting empty components and letting a background fetch catch up. Without this, every mount point would briefly clear its SSR fallback, paint blank (since `grid` starts as `None`), then repaint again a moment later once the fetch resolved — a visible flash between the SSR content disappearing and the real content appearing. Awaiting first means the very first paint already has real data.

**Build**: `./app.sh build`/`build`/`rebuild` all run a `build_wasm` step that compiles `media-app` (`cargo build -p media-app --target wasm32-unknown-unknown --release`) and regenerates `admin/static/media-app/{media_app.js,media_app_bg.wasm}` via `wasm-bindgen`. **Editing `media.rs` (the SSR page) without also running a real rebuild — i.e. using `./app.sh restart` instead of `rebuild` — leaves the running server serving stale HTML while the WASM bundle expects the new element IDs/markup.** This produced a real "mount point not found" bug during development; always use `rebuild` when both sides changed together. `install-vps.sh`''s `do_build()` step and its requirements check (for `wasm32-unknown-unknown` + `wasm-bindgen-cli`) were updated to match — a VPS deploy builds the WASM bundle locally, same as the main binary, and ships only the resulting static files.

### The Media Picker (`admin/src/lib.rs`)

Used from the post/page editor ("Set Featured Image") and Quill''s inline image/audio insert, plus the left-nav "Media" entry (a second, near-identical iframe: `media-browser-frame`/`openMediaBrowser`). Opens `/admin/media?picker=1` (or `?browser=1` for the nav browser) in an **iframe**; the selected item is returned to the parent page via `postMessage`. This is a clean boundary — the picker''s internals (now including the WASM island) don''t need the parent page, or vice versa, to know anything about each other. Converting the media library to WASM did not require any change to picker call sites.

**Keeping the iframe warm across opens (2026-08-12)**: originally both `openMediaPicker`/`openMediaBrowser` reset the iframe''s `src` to `about:blank` on every close, so reopening always meant a full page load — including re-running the *entire* WASM bootstrap (download, instantiate, initial fetch) from scratch every single time. Now, closing just hides the modal; the iframe (and its already-running WASM island) is left alone. On reopen, if the iframe was already loaded once (`data-loaded === ''1''`), instead of resetting `frame.src`, the parent posts a `{ type: ''resetPickerFilter'', typeFilter }` message into it. `media.rs`''s inline script forwards that to `window.mediaAppResetForPicker` (a `#[wasm_bindgen]`-exported `reset_for_picker()`), which resets to a clean "All Media" view scoped to the requested type filter (only the audio-insert picker mode actually filters by type) and refetches — in place, no reload. First open per admin page still pays the full bootstrap cost; every open after that in the same page reuses the warm iframe.

This does **not** persist across a real page navigation — navigating to a different admin page is a genuine browser page load, which tears down the iframe (and everything inside it) along with the rest of the page. The gain is "once per admin page visited," not "once ever." See `TODO.md` for a noted, not-yet-done partial mitigation (caching the last-fetched grid data in `sessionStorage` so even a fresh page''s first open skips the network round trip) and the longer-term option (converting the whole admin into a persistent client-side-routed WASM SPA, which is the only thing that fully solves this — see the project''s own planning notes on that, currently slated for after feature-complete, not incremental work).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/media | `admin::media::list` | Media library page — SSR shell that the WASM island mounts into |
| GET | /admin/api/media/grid | `admin::media::api_grid` | JSON grid data (items/folders/counts/pagination) — the island''s data source |
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

In production, `/uploads/*` bypasses Axum entirely — Caddy serves it directly via `file_server` for performance. Each site''s Caddyfile block roots that block at `{UPLOADS_DIR}/{DOMAIN}` (the deployed template) or `{UPLOADS_DIR}/{http.request.host}` (this repo''s local hand-maintained dev catch-all Caddyfile, which has no per-site block to substitute a literal domain into) — either way, scoped to *that site''s own* symlinked upload folder, matching `Media::url()`''s bare-filename convention. If a site''s uploads stop resolving in production after an upload-URL-related code change, check whether the deployed Caddyfile actually got regenerated from the current template — this is not automatic on every deploy.

The dev-mode Axum `uploads::serve()` handler supports both URL shapes so local testing without Caddy still works: a bare filename resolves the site via the request''s `Host` header (same pattern as everywhere else site context is derived from the request); the legacy `/uploads/{key}/{rest}` two-segment shape (UUID or hostname) is also still supported indefinitely, so already-published content with old-format links doesn''t break.

## Testing

`tests/e2e/media_bulk_move.py` — a Playwright-driven regression test for the WASM island''s bulk-move flow (select → move → in-place refresh → select a different item → move again, all in one page session with no reload in between). This scenario is exactly what surfaced the `window.ITEMS`/`selected`-staleness bugs described above; a Rust integration test can''t reach this class of bug since it''s purely client-side/WASM state. See `tests/e2e/README.md` for how to run it.

## Security Notes

- Site isolation is enforced throughout: non-global-admins cannot view/delete/update media, assign folders, or delete folders belonging to another site; folder assignment additionally verifies the target folder''s `site_id` matches.
- Authors are further scoped to their own uploads for delete and default list views.
- Uploaded filenames are slugified (`slugify_name`) and capped at 80 chars before being written to disk, preventing directory traversal and unwieldy filenames.
- Files are stored under a per-site subdirectory (`uploads/{site_uuid}/`) rather than a single flat directory, limiting blast radius between sites on shared storage.
- `alt_text`/`title`/`caption` values are passed through `sanitize_media_text` before being stored.
- The WASM island is CSR-only and runs entirely client-side against the same session-cookie-authenticated API routes the old server-rendered page used — it introduces no new trust boundary or auth path of its own.
', '2026-08-12 18:14:36.467535-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (16, 'sites', 'Sites & Multisite', '# Sites & Multisite

> Last updated: 2026-08-15 | Updated by: claude

## Overview

SynapCMS supports multiple sites from a single installation. Each site has its own hostname, content, users, themes, settings, and plugin activations.

## How It Works

### Data Model

- `sites`: `id` (UUID), `hostname`, `owner_user_id`, `created_at`, `updated_at`.
- `site_settings`: per-site settings (name, tagline, base URL, active theme, etc.). PK is `site_id`.
- `site_users`: maps users to sites with per-site roles.
- `site_plugins`: tracks active plugins per site.

### Ownership vs. Site Admin role (updated 2026-07-22)

`sites.owner_user_id` and `site_users.role = ''admin''` are related but distinct: a site can have **more than one** `admin`-role user (`core/src/handlers/admin/users.rs::add_site_access`), but only one of them is ever the `owner_user_id` — a lighter-weight "primary contact" marker, not a permission gate (permissions come entirely from `site_users.role` via `AdminCaps::from_roles` in `core/src/middleware/admin_auth.rs`, which only checks `site_role == "admin"`, never `owner_user_id`).

Whenever a user''s `site_users.role` is changed away from `''admin''` (via `add_site_access` or `save_edit`) and that user is the site''s current `owner_user_id`, the handler now also clears `owner_user_id` to `NULL`. Before this fix, demoting the owner left `owner_user_id` stale — `/admin/sites` reads `owner_user_id` (via `site::admin_email`) for its "admin" column independently of `site_users.role`, so the two views could silently disagree about who a site''s admin was. `remove_site_access` already cleared ownership correctly on full removal; the gap was specific to in-place role changes.

### In-Memory Cache

At startup, `AppState` loads all sites into `site_cache: Arc<RwLock<HashMap<String, (Site, SiteSettings)>>>` keyed by hostname. Rebuilt via `state.reload_site_cache()` after site create/delete.

### Site Resolution

The `CurrentSite` middleware extractor resolves hostname to `(Site, SiteSettings)` on every public request. Cache hits are validated against the DB. Unknown or unconfigured hostnames return HTTP 404. There is no empty-cache fallback.

### Admin Management

`core/src/handlers/admin/sites.rs` handles CRUD. Global admins manage all sites; site admins see only their site. `POST /admin/sites/switch` updates a session variable to change the active site in the admin panel. On the Sites list (`admin/src/pages/sites.rs`), the "Switch to this site" action icon is hidden for whichever site matches `ctx.current_site` (added 2026-08-05) — previously it was shown even for the site you were already viewing, which was harmless (the form would just re-switch to the same site) but pointless clutter.

### New-site Site Admin assignment (added 2026-07-22)

`/admin/sites/new` (`admin_sites::new_site` / `create`) now has a "Site Admin" section alongside the hostname field, styled to match `/admin/users/new` (shared `.profile-container` card, `.user-form-grid` layout): **Assign later** (unchanged default — the creating admin becomes temporary owner/admin, or if impersonating, the currently-visited site''s owner), **Existing user** (a dropdown of active non-super_admin users; picking one sets them as `owner_user_id` and registers them via `site_user::add`), or **New user** (inline username/email/display name/password fields, same live validation as `/admin/users/new` — username slugify-from-display-name, password requirements checklist, email format check; creates the account with global role `site_admin` and assigns them as owner). Previously the form only accepted a hostname, requiring a separate trip to `/admin/users/:id/site-access` to assign anyone.

### Dashboard Sites stat card (added 2026-07-22)

The admin dashboard (`core/src/handlers/admin/dashboard.rs`, `admin/src/pages/dashboard.rs`) now shows a **Sites** card between Pending and Users, linking to `/admin/sites`. Count is scoped like the sites list itself: total sites system-wide for a true super_admin (`site::count`), sites owned by the current site''s owner when a super_admin is impersonating (`site::count_by_owner`), or sites the user has any role on otherwise (`site_user::list_for_user(...).len()`). The stat panel grid grew from `.stat-panel-6` to a new `.stat-panel-7` CSS rule to fit the extra tile.

### Settings page tabs (updated 2026-08-15)

`/admin/sites/{id}/settings` is organized into three tabs — **General**, **Maintenance**, **Email Settings** — styled with the same `.page-tabs` JS-toggled-panel pattern Form Designer uses (all panels render into the DOM at once, a small inline script toggles `.active` on click; no page reload between tabs). General also carries the Support/Site-ID card that used to sit above the tabs as its own section.

### Multi-provider email (added 2026-08-15, replaces the old single-Mailgun-account model)

The Email Settings tab replaced the old single "Email (Mailgun)" card with a full provider system — a site can configure any number of named email accounts (Mailgun, SMTP, SendGrid, Postmark), verify each with a test send, and forms pick which one to use individually. Full details, including the data model, sending logic, and setup steps: see the **Email Providers** doc and `docs/email-providers-guide.md` in the repo. The old per-site Mailgun override (`site_settings.mailgun_domain`/`mailgun_api_key_encrypted`, `/admin/sites/{id}/mail-config`) is gone — the install-wide `.env` Mailgun account is now the only site-independent fallback, used when a form has no provider selected.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET/POST | /admin/sites | `admin_sites::list / create` | List and create sites |
| GET | /admin/sites/new | `admin_sites::new_site` | New site form |
| POST | /admin/sites/switch | `admin_sites::switch` | Switch active site |
| GET | /admin/sites/{id}/settings | `admin_sites::site_settings` | Per-site settings (General/Maintenance/Email Settings tabs) |
| POST | /admin/sites/{id}/site-config | `admin_sites::save_site_config` | Save site config (General tab) |
| POST | /admin/sites/{id}/maintenance | `admin_sites::save_maintenance` | Toggle maintenance mode (Maintenance tab) |
| POST | /admin/sites/{id}/email-providers | `admin_email_providers::create` | Add an email provider (Email Settings tab) — see the **Email Providers** doc |
| POST | /admin/sites/{id}/delete | `admin_sites::delete` | Delete site |
| POST | /admin/sites/{id}/provision-ssl | `admin_sites::provision_ssl` | Provision SSL |

## Security Notes

- Site deletion cascades to `site_settings`, `site_users`, `site_plugins`, `email_providers`, and removes the site plugin directory from disk.
- Only global admins can create or delete sites.
- Email provider credentials are encrypted (AES-256-GCM, keyed off `SECRET_KEY`) before being written to the database — see the **Email Providers** doc for details specific to that system.
', '2026-08-16 15:39:27.952779-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (17, 'forms', 'Forms', '# Forms

> Last updated: 2026-08-16 | Updated by: claude

## Overview

SynapCMS lets themes embed plain HTML `<form>` elements that POST to `/form/{name}` (`{name}` is always a form''s *slug*, whether it was hand-written into a theme template or created via Form Designer — see that doc). There is no required schema — any field names a theme author (or Form Designer) chooses are accepted and stored as JSONB, so a form''s fields can change without a migration. Submissions are scoped per site and viewable/exportable from the admin. Admins can also disable ("block") a named form so it silently stops accepting submissions (e.g. to stop spam) without touching the theme template or Form Designer definition.

Where to look at collected data has moved around a couple of times; as of 2026-08-16 the canonical place is the **Submissions tab** on a form''s own Analytics page (`/admin/analytics/form/{id}?tab=submissions`), reached via the Analytics icon on `/admin/analytics?tab=forms` or from inside the Form Designer editor. See **Admin views** below for how the older `/admin/form-data-analytics/{slug}` URL still fits in.

## How It Works

- **Public submission** (`core/src/handlers/form.rs`, `submit`): accepts any `Form<HashMap<String,String>>` posted to `/form/{name}`. Field names starting with `_` (e.g. a honeypot `_hp`) are stripped before storage so they never persist — a basic anti-spam trick. If all remaining fields are blank, nothing is stored. Before storing, it checks `form_submission::is_blocked` for the current site + form name; if blocked, it redirects back to the referring page with `?blocked=1` and never writes a row.
- **IP capture**: best-effort — prefers the `X-Real-IP` header, falls back to `X-Forwarded-For` (first entry), then falls back to the raw TCP peer address via `ConnectInfo`. In production Caddy sets the proxy headers; in local dev the peer address is used.
- **Email notifications**: `submit` looks up the form''s definition by slug (`form_def::get_by_slug`) to read its mail settings. Unless `settings.no_mail` is set (added 2026-08-16 — see the **Form Designer** doc), a `notify_email` address sends a plain-text admin notification, and `confirm_submitter` sends the submitter a templated confirmation — both `tokio::spawn`ed *after* the submission is already stored, so a slow or failed provider call never blocks the visitor''s redirect, and both route through `core::mail::send_for_site` using whichever provider the form''s `email_provider_id` selects (or the install-wide fallback). Failures are logged server-side only, never surfaced to the visitor. Full setup guide: `docs/mailgun-email-guide.md` in the repo.
- **Redirect UX**: on success, redirects back to the `Referer` (query string stripped) with `?submitted={slug}` appended (the form''s own slug, not a generic `?submitted=1`, so a page with multiple embedded forms shows the right one''s success message).
- **Lifetime counter**: after the submission row is inserted, `form_submission::create` also runs a best-effort `UPDATE forms SET total_submissions = total_submissions + 1 WHERE id = $1` when the submission resolved to a real `form_id`. This never decrements, even when a submission is later deleted — see the **Form Designer** doc''s Database Schema section for why, and where it''s displayed.
- **Storage** (`core/src/models/form_submission.rs`): `create` inserts one row per submission with the full field map as `data: JSONB`, plus (since 2026-08-16) a `form_id` FK — see **Database Schema**. Helper queries: `list_forms` (distinct form names per site with submission/unread counts, ordered by most recent), `count_for_form`/`list_submissions` (paginated, newest first — these still key off `form_name`, not `form_id`, since orphaned submissions with a NULL `form_id` still need to be listable by their old slug), `delete` / `delete_all`, `count_unread`, and `mark_all_read` (called automatically when an admin opens a form''s submissions).
- **Blocking** lives in the same model file, not a separate `form_block` model: `is_blocked`, `block`, `unblock`, and `blocked_names` all query the `form_blocks` table directly.
- **Admin views**:
  - `/admin/analytics?tab=forms` (`core/src/handlers/admin/analytics.rs::list`) is the main list — one row per distinct form name submitted on the site, with submission/unread counts, last-submitted time, and blocked state. A deleted form definition still shows up here (flagged, not hidden — see **Database Schema**), just without Edit/Analytics actions. Supports `?form={id}` to pre-filter to a single form (shown with a "Filtered to X — Clear" chip) — this is what the Form Designer editor''s Analytics icon links to.
  - `/admin/analytics/form/{id}` (`admin_analytics::form_detail`) is a form''s dedicated page, gated on having a real definition (an orphaned, definition-deleted form has no `{id}` to reach this by). Three tabs: **Stats** (total lifetime submissions, plus a Delivered/Failed email chart), **Delivery Results** (`mail_log` history, sortable/searchable), and **Submissions** (added 2026-08-16 — the actual collected data, replacing the old standalone page below). Its search/export/delete-all controls sit inline with the tab bar, same layout convention as Delivery Results'' search.
  - `/admin/form-data-analytics/{slug}` (`admin_forms::view_form`) still exists but as of 2026-08-16 redirects to the Submissions tab above whenever the slug still resolves to a live form definition — it only renders directly for an **orphaned** form (definition deleted), which has nowhere else to go. `collect_columns` (shared with the Submissions tab and CSV export) inspects every submission''s JSONB keys, prioritizes `name`, `email`, `subject`, `message` first, then adds any remaining keys alphabetically.
  - `export_csv` reuses the same column derivation to produce an RFC-4180-escaped CSV download (`form-{slug}.csv`). `toggle_block` flips the blocked state for a form. Deleting a single submission or all submissions for a form redirects back to the Submissions tab when a live definition exists, or the standalone page otherwise (same fallback logic as the view route).
- All admin form handlers require `admin.caps.can_manage_forms` and a resolved `site_id` from the `AdminUser` (returns 403/400 otherwise).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /form/{name} | form::submit | Public form submission endpoint (used by any theme `<form action="/form/{name}">`, `{name}` being the form''s slug) |
| GET | /admin/analytics?tab=forms | admin_analytics::list | List all forms for the current site with counts; `?form={id}` pre-filters to one form |
| GET | /admin/analytics/form/{id} | admin_analytics::form_detail | Stats / Delivery Results / Submissions tabs for one form (`?tab=stats\|results\|submissions`) |
| GET | /admin/form-data-analytics/{name} | admin_forms::view_form | Redirects to the Submissions tab above if the slug resolves to a live form; otherwise renders submissions directly (orphaned forms only) |
| POST | /admin/form-data-analytics/{name}/{id}/delete | admin_forms::delete_submission | Delete one submission |
| POST | /admin/form-data-analytics/{name}/delete-all | admin_forms::delete_all | Delete all submissions for a form |
| GET | /admin/form-data-analytics/{name}/export | admin_forms::export_csv | Download all submissions as CSV |
| POST | /admin/form-data-analytics/{name}/toggle-block | admin_forms::toggle_block | Block or unblock a form from accepting new submissions |

## Database Schema

- `form_submissions` (migration `0020_create_form_submissions`; `form_id` added by `0060_form_submissions_form_id.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `form_name TEXT` (the slug — still the authoritative lookup key everywhere, since it''s immutable and matches even for orphaned/pre-FK data), `data JSONB` (default `{}`), `ip_address TEXT`, `read_at TIMESTAMPTZ` (nullable — null means unread), `submitted_at TIMESTAMPTZ` (default now), `form_id UUID NULL` (FK → `forms.id`, `ON DELETE SET NULL`). Indexed on `(site_id, form_name, submitted_at DESC)` and on `form_id` (partial, where not null).
- `form_id` exists for exact joins/filtering (e.g. the `?form={id}` list filter) alongside `form_name`, not as a replacement for it — deleting a form definition sets existing rows'' `form_id` back to NULL via the FK, but `form_name` still matches and the data stays fully viewable/exportable, just under the "orphaned" fallback path described above. New rows get `form_id` populated automatically whenever `form::submit` can resolve the slug to a live definition at submit time.
- `form_blocks` (migration `0021_create_form_blocks`): `site_id UUID` (FK → `sites`, cascade delete), `form_name TEXT`, `blocked_at TIMESTAMPTZ` (default now), composite PK `(site_id, form_name)`. Presence of a row means the form is blocked.

## Security Notes

- Underscore-prefixed field names are stripped server-side, closing off internal/honeypot fields from ever being stored or overwriting real columns.
- Blocking is enforced before any DB write and fails silently from the visitor''s perspective (redirect only, no error page), avoiding tipping off spammers.
- All `/admin/form-data-analytics/*` and `/admin/analytics/*` form-related routes require `can_manage_forms` capability and a resolved site context; missing either returns 403 or 400 before touching the database.
- CSV export values are escaped per RFC 4180 to prevent malformed downloads from embedded commas/quotes/newlines (not a CSV-injection/formula-injection sanitizer — values are not prefixed to neutralize leading `=`/`+`/`-`/`@`).
- Submissions are strictly scoped by `site_id` in every query, so one site''s admins cannot see or delete another site''s form data.
- The notification/confirmation email sends (above) are fire-and-forget and failure-tolerant — a broken or unconfigured provider never prevents a submission from being stored or the visitor''s redirect from completing, and `no_mail` (see the **Form Designer** doc) gives an explicit, auditable way to guarantee a form never emails anyone at all.', '2026-08-16 15:39:27.952779-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (18, 'plugins', 'Plugins', '# Plugins

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Plugins are Tera template collections loaded from the filesystem at startup. They extend the CMS by registering template partials against named hook points (e.g. `head_end`, `after_content`). Plugins are installed per-site and tracked in the `site_plugins` table, and can register custom meta fields and HTTP routes via their manifest. The plugin system is designed for zero-compilation: plugin authors write Jinja2-style Tera templates, not compiled Rust.

**Paused indefinitely (confirmed 2026-07-22, not a pre-launch/post-launch timeline).** The admin plugin-management UI routes are commented out of the router — the handler code in `core/src/handlers/admin/plugins.rs` is fully implemented but not reachable via HTTP in the current build. This isn''t a launch-sequencing decision; it''s a genuine architectural problem the app hasn''t solved: because the core app is a compiled Rust binary, letting third parties add their own plugins is a real challenge (there''s no safe, dynamic extension mechanism the way an interpreted-language CMS has), and even the Tera-template plugins that do exist have very limited access to core app functionality precisely because of that compiled boundary. Revisiting this is possible someday but is a long way off and isn''t on any current roadmap.

## How It Works

### Manifest (`core/src/plugins/manifest.rs`)

Each plugin directory must contain a `plugin.toml` with a `[plugin]` section (`name`, `version`, `api_version`, `description`, `author`, `plugin_type`) and optional sections:
- `[hooks]` — maps hook names to template paths (relative to plugin dir)
- `[meta_fields]` — maps field keys to `MetaFieldDef` (label, type, description)
- `[routes]` — maps URL paths to `RouteRegistration` (template, content_type)

`plugin_type` is `"tera"` (default) or `"wasm"` (unimplemented — see Known Limitations).

### Loader (`core/src/plugins/loader.rs`)

`PluginLoader` scans the plugins directory, calls `PluginManifest::from_file` on each `plugin.toml`, adds all `.html` template files to the Tera instance (named relative to plugin root), registers hook handlers in `HookRegistry`, and stores `LoadedPlugin` metadata (manifest, directory, `source` — `"global"` or `"site"` — and optional `site_id`). `reload()` clears and re-scans for dev-mode hot reload.

### Hook Registry (`core/src/plugins/hook_registry.rs`)

`HookRegistry` wraps an `Arc<RwLock<HashMap<String, Vec<HookHandler>>>>`. `register` appends a `HookHandler` (plugin_name + template_path) to the list for a hook name. `handlers_for` returns handlers sorted alphabetically by `plugin_name`. `unregister_plugin` removes all handlers for a given plugin. Well-known hook name constants: `HEAD_START`, `HEAD_END`, `BODY_START`, `BODY_END`, `BEFORE_CONTENT`, `AFTER_CONTENT`, `FOOTER` — the list is open, plugins may define their own hook names.

### Site Plugin Model (`core/src/models/site_plugin.rs`)

`SitePlugin` struct: `site_id`, `plugin_name`, `active`, `installed_at`. Functions: `install` (idempotent, sets `active = false`), `activate`, `deactivate`, `delete`, `list_for_site`, `is_active`, `active_plugin_names`.

### Admin Handler (`core/src/handlers/admin/plugins.rs`)

Mirrors the same install/upload/activate/delete pattern used by `themes.rs`. Every handler requires `admin.caps.can_manage_plugins`.
- `list` — `?filter=my` (default) lists plugins installed for the current site with active state; `?filter=global` lists the shared library under `plugins/global/`, flagging which are already installed for this site.
- `install` — copies a plugin directory from `plugins/global/{name}/` to `plugins/sites/{site_id}/{name}/` (only if not already present), guarded against path traversal by canonicalizing and checking `starts_with` the expected parent, then records the install in `site_plugins`.
- `upload` — accepts a multipart zip (size-limited by `state.config.max_upload_mb`, minimum enforced floor of 25MB), extracts to a temp directory under the site''s plugin folder, validates `plugin.toml` exists and is well-formed TOML with a safe `name`, validates `plugin_type` (`"tera"` always OK, `"wasm"` requires an included `.wasm` file, anything else rejected), moves the temp dir to its final location (replacing any existing plugin of the same name), registers the plugin''s templates into the shared Tera engine (`register_plugin_templates` — globs `*.html`/`*.xml`, since the `glob` crate doesn''t support brace expansion), and records the install in the DB.
- `activate` / `deactivate` — flip the `active` flag in `site_plugins`.
- `delete` — refuses to delete an active plugin ("Deactivate it first"), guards against path traversal by checking the canonicalized plugin path''s parent equals the site''s plugin directory, removes the directory recursively and deletes the DB record.

## Routes / Endpoints

**Disabled indefinitely in `core/src/router.rs`** (not a pre/post-launch timeline — see Overview). No `/admin/plugins*` route currently exists in the running app. The handlers below exist in code and would be wired up like this if the plugin system is ever revisited:

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/plugins | plugins::list | List plugins (site-installed or global library) |
| POST | /admin/plugins/install | plugins::install | Install from global library |
| POST | /admin/plugins/upload | plugins::upload | Upload a plugin zip |
| POST | /admin/plugins/activate | plugins::activate | Activate an installed plugin |
| POST | /admin/plugins/deactivate | plugins::deactivate | Deactivate a plugin |
| POST | /admin/plugins/delete | plugins::delete | Delete an installed, inactive plugin |

Unrelated to admin management, `core/src/handlers/plugin_route.rs` dispatches plugin-registered public routes (declared under a manifest''s `[routes]` section) at runtime — `state.plugin_routes` is built from all loaded plugins'' route registrations and each path is registered individually in the router (`plugin_route::dispatch`), plus a dedicated `/sitemap.xml` route (`plugin_route::sitemap`). These public dispatch routes are unaffected by the admin-UI being disabled.

## Database Schema

`site_plugins` (migration `0023_site_plugins`): `site_id UUID` (FK → `sites`, cascade delete), `plugin_name TEXT`, `active BOOLEAN` (default false), `installed_at TIMESTAMPTZ` (default now) — composite PK `(site_id, plugin_name)`.

## Configuration

`AppState.config.plugins_dir` — base directory containing `global/` (shared plugin library) and `sites/{site_id}/` (per-site installed copies) subdirectories. `state.config.max_upload_mb` bounds plugin zip upload size (floor of 25MB enforced in the upload handler regardless of configured value).

## Security Notes

- Path traversal is guarded on both install and delete by canonicalizing paths and checking the resolved path starts with (or is a direct child of) the expected parent directory; plugin names must not contain `..`, `/`, or `\`.
- Uploaded zip entries are checked individually for `..`, leading `/`, or leading `\` before extraction.
- Uploaded zips are validated for a well-formed `plugin.toml` with a safe name before the extracted files are moved into place; invalid uploads are cleaned up.
- Active plugins cannot be deleted, preventing a live site from losing templates/hooks mid-request.
- All handlers require `can_manage_plugins`, but since the routes are currently commented out of the router, the entire admin plugin-management surface is unreachable regardless of capability.

## Known Limitations / TODOs

- Admin plugin-management routes are commented out of `core/src/router.rs`, indefinitely — install/upload/activate/deactivate/delete are unreachable via HTTP even though fully implemented. Not scheduled to change; see Overview for why.
- WASM plugin support (`plugin_type = "wasm"`) is validated for presence of a `.wasm` file at upload time but the loader (`core/src/plugins/loader.rs`) only loads Tera templates — there is no WASM execution path. Same status as the rest of the plugin system: paused, no timeline.', '2026-07-22 20:08:17.269869-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (15, 'users', 'Users & Roles', '# Users & Roles

> Last updated: 2026-08-18 | Updated by: claude

## Overview

Users are stored in the `users` table. Five global roles are defined: `subscriber`, `author`, `editor`, `site_admin`, and `super_admin`. Site-specific access is stored separately in the `site_users` table using a distinct, smaller set of site-scoped roles: `admin`, `editor`, `author`, `subscriber`. As of 2026-08-18 a user can hold **more than one** of these site roles on the same site simultaneously (e.g. both `editor` and `author` on the same site) — see "Multiple roles per user per site" below. `super_admin`/`site_admin` are global-only concepts and can never appear as a `site_users.role` value, enforced both by the `site_users.role` CHECK constraint and, independently, at the Rust type level (see below). Users can be soft-deleted (content preserved) or hard-deleted (content optionally reassigned).

## How It Works

### Model (`core/src/models/user.rs`)

Key structs:
- `User` — full DB row: `id`, `username`, `email`, `display_name`, `password_hash` (Argon2, never serialized), `bio`, `avatar_media_id`, `role`, `is_active`, `is_protected`, `deleted_at`, `default_site_id`
- `UserContext` — template-safe view: `id`, `username`, `display_name`, `bio`, `role`, `url` (`{base_url}/author/{username}`)
- `UserRole` enum with variants: `Subscriber`, `Author`, `Editor`, `SiteAdmin`, `SuperAdmin`
- `CreateUser`, `UpdateUser` — mutation structs

Password requirements (enforced by `validate_password`): 8–12 characters, at least one uppercase letter, one digit, and one symbol from `!@#$%&`. Passwords are hashed with Argon2 via `hash_password`. Verification via `User::verify_password`.

Key functions: `create`, `get_by_id`, `get_by_id_include_inactive` (added 2026-08-05 — see Suspend/Reactivate below), `get_by_username`, `get_by_username_include_inactive` (added 2026-08-06), `get_by_email`, `update`, `soft_delete`, `delete`, `delete_and_reassign`, `list`, `list_all` (added 2026-08-05), `deactivate`, `reactivate` (added 2026-08-05), `count`, `count_for_site`, `count_global_admins`, `set_default_site`, `hash_password`, `verify_password`.

`get_by_id`, `get_by_username`, and `get_by_email` all exclude soft-deleted **and suspended** users (`deleted_at IS NULL AND is_active = TRUE`) — this is what actually blocks a suspended account from logging in anywhere (admin, public `/login`, `/account`), since every login path resolves the user through one of these.

The initial 2026-08-05 Suspend/Reactivate rollout only swapped `get_by_id` for `get_by_id_include_inactive` in the couple of places suspension itself needed (editing the admin profile, the suspend/reactivate handlers'' own guard checks). It missed that the same strict, active-only lookups were also used to fetch a **post''s author** for rendering — not just for login. Suspending an author broke, site-wide, every page that rendered one of their posts: the public home page and single-post/page views (`build_post_context` in `handlers/home.rs`), the "recent posts" widget''s per-post author lookup and its `author=username` filter (`templates/functions.rs`), the public author-archive page `/author/{username}` (`handlers/archive.rs`), and the admin posts list/edit author display (`handlers/admin/posts.rs`, which already degraded to "Unknown" rather than 404ing, but still lost the name). Fixed 2026-08-06 by switching all of these to `get_by_id_include_inactive` / the new `get_by_username_include_inactive`, and by changing the `recent_posts` author-filter query directly (it queries `users` inline rather than through a model function) from `is_active = TRUE` to `deleted_at IS NULL`. The admin post-edit page also now shows a red "Suspended" badge next to the author''s name (reusing the badge markup from the Users list) when the author is suspended, mirroring how the Users list already flags them.

Rule of thumb going forward: suspension must only ever gate **login/session resolution**, never the visibility of already-published content. Any new lookup of a post/page''s author (or any other suspend-able user acting purely as an attribution/display value rather than an authenticating principal) should use the `_include_inactive` variant, not the strict one.

### Site User Model (`core/src/models/site_user.rs`)

`SiteUser` struct: `id` (surrogate PK, added 2026-08-18), `site_id`, `user_id`, `role`, `invited_by`, `created_at`.

`SiteRole` enum (added 2026-08-18): `Admin | Editor | Author | Subscriber` — the site-scoped role type. Deliberately has **no** variant for `SuperAdmin`/`SiteAdmin`; there is no `From<UserRole> for SiteRole` conversion either. This means any function typed to take a `SiteRole` (rather than a raw `&str`) cannot compile if handed a global role — the multi-role work leaned on this as defense-in-depth on top of the pre-existing `site_users.role` CHECK constraint, specifically so a bug can never let a global-only role be assigned or session-pinned as a per-site role.

Key functions: `add(pool, site_id, user_id, role: SiteRole, invited_by)` — idempotent per-role insert (as of 2026-08-18; previously an upsert that overwrote any existing role — see "Multiple roles per user per site" below), `remove` (removes ALL of a user''s roles on a site), `remove_role` (added 2026-08-18, removes just one), `has_any_role` (added 2026-08-18 — pure access check, replaces the old `get_role` for "is this user allowed on this site" gates), `list_roles_for_user_and_site` (added 2026-08-18, returns `Vec<SiteRole>` — the function the login role picker and the `AdminUser` extractor both call), `update_role` (replaces ALL of a user''s roles on a site with exactly one, used by the single-role dropdown flows described below), `list_for_site` (returns `Vec<(User, String)>` — one row per role, so a multi-role user now appears more than once per site in this list), `list_for_user` (returns `Vec<(Site, String)>`, same one-row-per-role caveat), `count_admins` (added 2026-07-22 — counts `role = ''admin''` rows for a site; used to warn before removing/demoting the last one), `sole_admin` (added 2026-07-23 — returns the single user_id if exactly one `role = ''admin''` row exists for a site, independent of `sites.owner_user_id`; used to warn before demoting an admin who isn''t the recorded site owner).

The old `get_role` (returned `Option<String>`, arbitrary-one-row semantics) was removed 2026-08-18 once multi-role made "the role" ambiguous — every call site was migrated to either `has_any_role` (access checks) or `list_roles_for_user_and_site` (anywhere the actual role value mattered).

### Auth Handler (`core/src/handlers/auth.rs`)

- `login_post` (`POST /admin/login`) — fetches user by email, verifies password, checks role allows admin access, resolves site from Host header, verifies site access via `site_user::has_any_role` for non-super-admins, writes `admin_user_id` and `current_site_id` to session, clears any stale `current_site_role` pin from a prior session. Does **not** itself decide whether a role pick is needed — that check happens lazily on the next `AdminUser`-guarded request (the post-login `/admin` redirect); see "Multiple roles per user per site" below.
- `public_login_post` (`POST /login`) — subscriber-only login, writes `account_user_id` to session.
- `logout` — removes `admin_user_id` and `current_site_id`.
- `account_logout` — removes `account_user_id`.

### Admin Users Handler (`core/src/handlers/admin/users.rs`)

Handlers: `list`, `new_user`, `save_new`, `edit_user`, `save_edit`, `delete_user`, `suspend_user`, `reactivate_user` (both added 2026-08-05), `bulk_delete_users`, `site_access_page`, `add_site_access`, `remove_site_access`. All require `can_manage_users` (admin or above). Super-admins see all users; site admins see users for their site only.

The `list` handler''s "all sites" branch (global admin, no site filter) uses `user::list_all` rather than `user::list` — `list` filters `is_active = TRUE`, which would make a suspended user vanish from the admin UI entirely with no way to reactivate them. `list_all` only excludes soft-deleted rows. Other call sites of `user::list` (e.g. `sites.rs`''s assignable-user dropdown for new-site ownership) correctly keep the active-only filter — you shouldn''t be able to hand site ownership to a suspended account.

### Role changes are exclusive to Site Access (changed 2026-07-22)

`save_edit` (`/admin/users/:id/edit`) no longer changes a user''s role at all — `admin/src/pages/users.rs::render_editor` renders Role as a read-only `<p>` with a "Change Role" button linking to `/site-access` for existing users (the new-user form at `/admin/users/new` still has an editable role dropdown, since that''s an initial assignment, not a change). Reason: the edit page has no site picker, so its old editable dropdown silently applied to whichever site the *acting* admin currently had selected in their own session — ambiguous and easy to misread as a global role change, especially for a target user with multiple site assignments. `/site-access` is explicit about which site is affected and is the only place site-scoped role changes happen now.

### Edit form: Role folded into the same panel as a 4th section (changed 2026-08-05)

`render_editor` renders both the new-user and edit-user forms inside a single `.card-boxed` panel (`.card-boxed-section` per field group — see the documentation page''s card-boxed-style conventions). Section order: (1) Display Name, Username, Email, Password (Display Name now comes first, swapped 2026-08-05 to match how people actually think about the fields), (2) Role + site assignment (new-user form only — an editable role dropdown plus new-site/existing-site picker), (3) a live requirements checklist (Username/Password/Role, `.form-note`/`.pw-dot` pattern), and on the **edit** form only, (4) a Role display section — current role (read-only `<p>`) + "Change Role" button linking to `/site-access`, plus a read-only table of the user''s existing site assignments (hostname/role, one row per site, sourced from `UserEdit.site_roles` populated via `site_user::list_for_user`). This is display-only; changing any of those roles still requires going through `/site-access`.

Previously (2026-07-23–2026-08-04) the Role section was a separate panel below the form — first as its own `.profile-container` card, later restyled to `.card-boxed` — but as a standalone panel it had no `max-width` constraint and stretched full page width while the form above it was capped at 580px. Folding it into the same panel as a 4th section fixed both the visual mismatch and the "why is this a separate page section" ambiguity — it''s part of the same "editing this user" task.

Server-side, `display_name` is now also length-validated (≤60 chars, `validate_display_name`) on both `save_new` and `save_edit` — previously unbounded.

### Multiple Site Admins per site (changed 2026-07-22)

`site_users.role = ''admin''` is no longer capped at one holder per site. `add_site_access`''s `"site_admin"` branch: if the target site has no owner yet, the new user becomes owner (`sites.owner_user_id`) and is promoted to global role `site_admin`, same as before. If the site already has an owner, the site-access page''s modal now offers three choices instead of forcing a swap: **Add as an additional Site Admin** (`displaced_action=add_additional` — existing owner and admin access untouched, new user just gets a second `site_users.role=''admin''` row), **Remove from site** (`displaced_action=remove` — existing admin loses access, ownership transfers), or **Demote to Author, transfer ownership** (`displaced_action=demote_author`).

### Multiple roles per user per site + login-time role picker (added 2026-08-18)

A user can now hold more than one `site_users` role on the same site at once (e.g. both `editor` and `author`) — previously `site_users.role` was capped at exactly one row per `(site_id, user_id)` pair by the table''s primary key.

**Schema (migration `0062_site_users_multi_role.sql`):** `site_users`'' primary key changed from `(site_id, user_id)` to a surrogate `id UUID`, with a new `UNIQUE(site_id, user_id, role)` constraint replacing the old composite PK''s uniqueness guarantee. The `role` CHECK constraint (`''admin'' | ''editor'' | ''author'' | ''subscriber''`) was deliberately left untouched — it never allowed `super_admin`/`site_admin` before and still doesn''t; see the `SiteRole` enum note above for the matching Rust-level guarantee.

**Session/UX model:** the chosen UX is "pick one role, act as if you only had that one for the rest of the session" — not "union of permissions across all held roles." Concretely:
- `AdminUser.site_role` (`core/src/middleware/admin_auth.rs`) is `Option<SiteRole>`, not a raw string. It is re-derived on every request (no role is cached in the session cookie) from `site_user::list_roles_for_user_and_site`:
  - 0 roles on the current site → falls back to trying the user''s *global* role as a site role (works for `editor`/`author`; correctly yields `None` for `site_admin`/`super_admin`, since those aren''t valid `SiteRole` values — this matches the pre-2026-08-18 fallback behavior for those roles, which never matched any site-role comparison either).
  - 1 role → used directly, no picker involved.
  - ≥2 roles → requires a valid `SESSION_CURRENT_ROLE_KEY` session value that is still one of the roles currently held (a role can be revoked after being pinned — re-validated every request, not just at pick time). If missing or invalid, the extractor returns `AdminAuthError::RolePickRequired`, which redirects to `/admin/pick-role` instead of failing the request.
- Global admins (`super_admin`) always get a synthetic `Some(SiteRole::Admin)` without ever consulting `site_users` — this bypass path is unchanged by the multi-role work.
- `GET/POST /admin/pick-role` (`core/src/handlers/admin/role_picker.rs`, page in `admin/src/pages/role_picker.rs`): shown when a role pick is required. Renders a `<select>` of the roles the user actually holds on the current site plus a single hexagon-icon pill submit button; shows the *site''s own hostname* as the page heading (not the app name), and applies the visitor''s saved theme preference before first paint, matching the standalone login page''s conventions. The POST handler re-validates the submitted role against `list_roles_for_user_and_site` before pinning it — a posted role the user doesn''t actually hold is rejected (logged, not silently accepted) rather than trusted from the form. Uses a separate lightweight `PickRoleUser` extractor (session user_id + site_id only, no role resolution) specifically so this route itself can''t recurse into `RolePickRequired`.
- `sites::switch` and `sites::go_home` (`core/src/handlers/admin/sites.rs`) both clear `SESSION_CURRENT_ROLE_KEY` whenever they change the session''s current site, since a different site can have an entirely different role set for the same user — the picker re-triggers on the next request if the new site also needs one.

**`site_user::add` semantics changed:** previously an upsert (`ON CONFLICT (site_id, user_id) DO UPDATE SET role = EXCLUDED.role`) that silently replaced any existing role — safe only because a user could hold exactly one. Now it''s an idempotent per-role insert (`ON CONFLICT (site_id, user_id, role) DO UPDATE SET invited_by = COALESCE(...)`): re-adding a role the user already holds is a no-op, and adding a *different* role no longer removes any role they already had. Two other raw-SQL `site_users` upserts that predated this change and still used the old 2-column conflict target were found and fixed in the same pass: `site::create_with_defaults` (site creation''s owner-as-admin seeding) and the CLI''s `synap user create`.

**Users list (`/admin/users`) display:** a user with multiple roles on the same site previously showed one duplicated domain badge per role. As of 2026-08-18 it shows a single badge per site with a `+` suffix when more than one role is held there, and the full role list (e.g. "beth.com — Author, Editor") in the badge''s hover tooltip — sourced from a small in-handler grouping fix in `admin/users.rs::list` (the raw membership query now also selects `su.role` and dedupes by `site_id` before building `UserRow.site_hostnames`/`site_role_labels`).

**Not changed by this work:** the Users-page single-role edit dropdown and `/site-access`''s Add/Remove flows still operate on "replace this user''s role(s) on this site with exactly one" (`update_role`) — assigning *multiple* roles to a user is currently only reachable by calling `site_user::add` more than once (e.g. via the dev-tools seeding endpoint or directly), not through a dedicated multi-select UI. A proper multi-role assignment UI on `/site-access` is a natural follow-up, not yet built.

### Owner/role desync bug (fixed 2026-07-22)

Demoting a user away from `''admin''` via `add_site_access`''s editor/author/subscriber branch (or previously via `save_edit`, before role changes were removed from that form) updated `site_users.role` but never cleared `sites.owner_user_id` — since `/admin/sites` reads `owner_user_id` independently of `site_users.role` for its admin display, a demoted user could keep showing as "admin" there while `/site-access` correctly showed their new (lower) role. Both `add_site_access` and `save_edit` now clear `owner_user_id` (`UPDATE sites SET owner_user_id = NULL WHERE id = $1 AND owner_user_id = $2`) whenever the demoted user is the site''s current owner — matching what `remove_site_access` already did on full removal. The site-access page''s JS also warns before this happens: *"{name} is currently the Site Admin and owner of {site}. Changing their role will remove that access and site ownership. Continue?"*

### Last-admin-on-a-site warning (added 2026-07-22)

`site_access_page` now computes an `is_last_admin` flag per site assignment (`role == "admin" && site_user::count_admins(site_id) <= 1`) and threads it into `SiteAssignmentRow`. The remove button''s confirm dialog uses a stronger message when it''s the only admin: *"{hostname} has no other Site Admin. Removing this access will leave the site with no one able to manage it (other than a super admin). Continue?"* This is a warning, not a hard block — a super_admin always retains access via `AdminCaps::is_global_admin` regardless, so the risk is losing the *site owner''s own* ability to manage their site, not a platform lockout.

### Sole-admin demotion warning gap on the Add form (fixed 2026-07-23)

The demotion warning described above only covered the **Remove** button. The **Add** form (re-selecting a site the user is already assigned to and choosing a different role, which upserts via `site_user::add`''s `ON CONFLICT ... DO UPDATE`) had its own, narrower warning that only fired when the target user equalled `sites.owner_user_id` (`SiteOption.existing_admin_id`, from `fetch_site_options`). That check missed two realistic, UI-reachable cases: a Site Admin added via **"Add as an additional Site Admin"** (never became the recorded owner), and an admin left over after the actual owner was removed from the site via `remove_site_access` (which clears `owner_user_id` but does not transfer it to a remaining admin). In both cases the user could be the site''s *only* admin yet not match `existing_admin_id`, so demoting them via the Add form went through silently, leaving the site with no site-level admin. Fix: `SiteOption` gained `sole_admin_id`/`sole_admin_name`, populated in `site_access_page` via the new `site_user::sole_admin(site_id)` (independent of ownership); the Add form''s submit handler now shows the same style of confirm — *"{name} is the only Site Admin for {site}. Changing their role will leave the site with no Site Admin. Continue?"* — whenever the selected site''s sole admin matches the user being edited, regardless of who owns the site.

### Suspend / Reactivate (added 2026-08-05)

A lightweight alternative to delete: `suspend_user` (`POST /admin/users/:id/suspend`) sets `is_active = FALSE`, immediately blocking login everywhere without touching the account''s content (posts, pages, media all untouched — unlike `delete_and_reassign`). `reactivate_user` (`POST /admin/users/:id/reactivate`) reverses it. Both are icon buttons (`user-x.svg`/`user-check.svg`, toggling based on `UserRow.is_active`) next to Edit/Delete on the Users list rows (`admin/src/pages/users.rs::build_staff_rows`/`build_sub_rows`), with a red "Suspended" badge and dimmed row (`opacity:.65`) for suspended accounts.

`suspend_user` guards mirror `delete_user`''s: no self-suspend, can''t suspend a protected account, only a global admin may suspend another global admin, and the last global admin can never be suspended (same lockout risk as deleting them — checked via `count_global_admins`, which only counts active admins, so a suspended super_admin doesn''t count toward the "last one" total). `reactivate_user` only requires `can_manage_users` — reactivating is the safe direction, no lockout risk to guard against. The underlying `is_active` column and a `deactivate()` model function already existed (added with the base user schema) but were never wired to any route or UI before this — `is_active = TRUE` was already a filter on every login-lookup query, so the blocking mechanism worked the moment it was called; only the CLI/admin-UI path to call it was missing.

Suspending an author was found (2026-08-06) to also break public rendering of their posts site-wide, not just their own login — see the `_include_inactive` discussion under Model above for the full list of affected pages and the fix. Suspension is meant to gate login only.

### Erase Personal Data — GDPR erasure (added 2026-08-19)

Subscribers only (staff accounts are a business relationship, not the self-service "forget me" case GDPR erasure targets). An "Erase Personal Data" icon (`shield.svg`) on each subscriber row in the Subscribers tab opens a review page (`GET /admin/users/{id}/erase-personal-data`) before anything happens — nothing is erased on click alone.

What erasure does (`user::erase_personal_data`): anonymizes the `users` row in place — username/email/display_name/bio/avatar replaced with placeholders, `password_hash` replaced with a random unusable value, `is_active` set false, `personal_data_erased_at` stamped. The row is **not deleted** — `posts.author_id`/`media.uploaded_by` are `ON DELETE RESTRICT` and `comments.author_id` is `ON DELETE CASCADE`, so a hard delete would either fail or silently wipe their comment history off other people''s posts. Since `comments` has no separate author-identity columns (pure FK to `users`), anonymizing the row already anonymizes every comment they left; only `comments.ip_address` needs clearing separately (`comment::clear_ip_for_author`), since that''s stored per-comment. Also deleted: `saved_posts` rows and pending `password_resets` for the account.

`form_submissions` and `mail_log` have no `user_id` FK at all (submitters/recipients aren''t required to have an account), so there''s nothing to erase automatically there. Instead, the review page searches both by the subscriber''s email (`form_submission::find_by_email`, `mail_log::find_by_email` — best-effort `ILIKE` text search, not an exact match) across every site the subscriber holds a role on, and shows matches as checkboxes (checked by default) for the admin to confirm or exclude before submitting.

Deliberately **not** touched: `audit_log` (the site''s own accountability trail — GDPR generally allows retaining logs needed for security/legal purposes) and active sessions (`tower_sessions` stores an opaque blob with no queryable `user_id`, so there''s no clean way to invalidate just this user''s session — same known gap as `suspend_user`, which has never invalidated sessions either; the random password + `is_active = false` block any *new* login, but an already-active session isn''t force-killed).

Every erasure is recorded to `audit_log` (`user.personal_data_erased`) with the original email as the target label, captured before it''s overwritten.

### `is_protected` fix for CLI-created super_admins (fixed 2026-07-22)

`synap user create` lets you pick `super_admin` from its role menu but previously never set `is_protected`, unlike `install` which hardcodes it `TRUE`. A super_admin created this way silently defaulted to `is_protected = FALSE` (the migration 0012 column default), making them invisible to `dev reset`''s admin lookup, exempt from delete-protection, and unable to be auto-assigned as site owner. `cli/src/commands/user.rs` now sets `is_protected = (role == "super_admin")` on insert.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/users | `users::list` | List users |
| GET | /admin/users/new | `users::new_user` | New user form |
| POST | /admin/users/new | `users::save_new` | Create user |
| GET | /admin/users/{id}/edit | `users::edit_user` | Edit user form |
| POST | /admin/users/{id}/edit | `users::save_edit` | Save user edits |
| POST | /admin/users/{id}/delete | `users::delete_user` | Delete user |
| POST | /admin/users/{id}/suspend | `users::suspend_user` | Suspend login (content untouched) |
| POST | /admin/users/{id}/reactivate | `users::reactivate_user` | Restore login access |
| GET/POST | /admin/users/{id}/erase-personal-data | `users::erase_personal_data_review` / `erase_personal_data` | GDPR erasure review + confirm (subscribers only) |
| POST | /admin/users/bulk-delete | `users::bulk_delete_users` | Bulk delete |
| GET | /admin/users/{id}/site-access | `users::site_access_page` | Site access management |
| POST | /admin/users/{id}/site-access/add | `users::add_site_access` | Add site role |
| POST | /admin/users/{id}/site-access/remove | `users::remove_site_access` | Remove site role |
| GET | /admin/pick-role | `role_picker::show` | Login-time role picker (added 2026-08-18) — shown when the user holds >1 role on the current site |
| POST | /admin/pick-role | `role_picker::submit` | Pin the chosen role (server-revalidated) to the session |

## Database Schema

`users` table: `id UUID PK`, `username TEXT UNIQUE`, `email TEXT UNIQUE`, `display_name TEXT`, `password_hash TEXT`, `bio TEXT`, `avatar_media_id UUID`, `role TEXT`, `is_active BOOL`, `is_protected BOOL` (migration 0012), `deleted_at TIMESTAMPTZ` (migration 0016), `default_site_id UUID` (migration 0018), `personal_data_erased_at TIMESTAMPTZ` (migration 0067, 2026-08-19).

`site_users` table: `id UUID PK` (surrogate, migration 0062, 2026-08-18), `site_id UUID`, `user_id UUID`, `role TEXT` (CHECK: `admin | editor | author | subscriber` — never `super_admin`/`site_admin`), `invited_by UUID`, `created_at TIMESTAMPTZ` — `UNIQUE(site_id, user_id, role)` (migration 0062; previously `PRIMARY KEY (site_id, user_id)`, which is what capped a user to one role per site before 2026-08-18).

## Security Notes

`password_hash` has `#[serde(skip_serializing)]` so it never appears in JSON or template context. `is_protected` users cannot be deleted or suspended (enforced in handlers). Soft-delete preserves all authored content. `delete_and_reassign` reassigns posts and media before removing the user row. Suspension is fully reversible and never touches content — the more surgical option when delete is too destructive (e.g. a subscriber flagged for abuse, or a staff member on leave).





', '2026-08-19 13:28:57.373329-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (22, 'subscriptions', 'Subscriptions', '# Subscriptions

> Last updated: 2026-08-05 | Updated by: claude

## Overview

The subscriptions feature allows visitors to create a `subscriber` role account via `/subscribe`. New subscribers are assigned to the current site via a `site_users` row. Existing users from another site are linked to the new site without creating a duplicate account. The form includes bot protection via a honeypot field and a human-check checkbox, plus a required Terms of Service checkbox. A live requirements checklist (display name/email/password/human-check, same `.form-note`/`.pw-dot` pattern as the admin New User form) updates as the visitor types. The page cross-links to `/login` ("Already a member? Sign in") and, since 2026-08-05, `/login` itself cross-links back to `/subscribe` ("Join today!") and to the new `/recover` password-recovery flow (documented below).

## How It Works

### Handler (`core/src/handlers/subscribe.rs`)

Two handlers are defined:
- `subscribe_form` (`GET /subscribe`) — renders the signup form (`admin::pages::subscribe::render`) or a success page (`render_success`) if `?subscribed=1` is present. Site resolution comes from the Host header via the `CurrentSite` extractor, so posting to a given site''s host automatically scopes the new subscriber to that site — no extra query params or hidden fields required.
- `subscribe_post` (`POST /subscribe`) — validates and processes the signup form.

Validation and processing flow:
1. Honeypot check: `website` field must be empty; non-empty silently redirects to `?subscribed=1` (bots are not told they were caught).
2. `human_check` must be `"on"`.
3. `terms` (ToS agreement) must be `"on"`.
4. `display_name` must be non-empty and satisfy `validate_display_name` (≤60 chars, added 2026-08-05 — previously unbounded, so an arbitrarily long display name could reach the DB and break layout in admin lists/author URLs/email templates). Also enforced client-side via `maxlength="60"` on the field.
5. `email` must be non-empty and contain `@` (lowercased before use).
6. `password` must equal `confirm_password`.
7. `validate_password` enforces complexity rules (8–12 chars, uppercase, digit, symbol) — identical to admin user creation.
8. If the email already exists: checks for an existing `site_users` row for this site; if present, returns "already subscribed" error; if absent, adds the row and redirects to the success page.
9. If the email is new: generates a username via `generate_username`, creates the user with `UserRole::Subscriber`, and adds a `site_users` row (skipped only for the nil-UUID fallback used in single-site mode).

### Password Recovery (`/recover`, added 2026-08-05)

A separate handler, `core/src/handlers/recover.rs`, lets a subscriber reset a forgotten password:
- `GET /recover` / `POST /recover` — request form. On POST, looks up the email; if found **and the account''s role is `subscriber`**, generates a single-use token (`core/src/models/password_reset.rs`, stored as a SHA-256 hash with a 1-hour expiry, never the raw token), emails a `/recover/{token}` link via `crate::mail::send_for_site` (the site''s own Mailgun account, falling back to the install-wide one), and returns the same "check your email" message either way — including for staff accounts and unregistered addresses — so the form can''t be used to enumerate which emails exist or which belong to staff. Staff password resets remain CLI-only (`synap user reset-password`) by design.
- `GET /recover/{token}` / `POST /recover/{token}` — shows a "set a new password" form if the token is still valid (unexpired, unused); POST validates the new password with the same `validate_password` rule as everywhere else, consumes the token (marks it used so it can''t be replayed), updates the password hash, and redirects to `/login` with a success flash message.

`generate_username` derives a base username by slugifying the display name (e.g. "Steve Miller" → "steve-miller") and now (fixed 2026-08-05) always produces a result that satisfies `validate_username` (8–15 chars, lowercase/digits/hyphens, no leading/trailing hyphen) — previously it had no length enforcement at all, so a short display name like "Bo" produced an invalid 2-char username. Names under 8 chars are padded with hex from a fresh UUID; names over 15 chars (after slugifying) are truncated, with any resulting trailing hyphen stripped. If the base is taken, it tries sequential numeric suffixes (`steve-miller2`, `steve-miller3`, … up to 9999, trimming the base as needed to stay within 15 chars), then falls back to a guaranteed-valid, guaranteed-unique `user{11 hex chars}` (15 chars total). Every candidate is re-validated against `validate_username` before being accepted. Uniqueness is checked via `username_taken`, a simple `SELECT EXISTS` query. Since the username is never shown to or confirmed by the visitor, it must be generated valid rather than relying on them to fix it.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /subscribe | `subscribe::subscribe_form` | Subscription signup form |
| POST | /subscribe | `subscribe::subscribe_post` | Process signup |
| GET | /recover | `recover::request_form` | Password recovery request form |
| POST | /recover | `recover::request_post` | Send recovery email (subscriber accounts only) |
| GET | /recover/{token} | `recover::reset_form` | Set-new-password form, if token valid |
| POST | /recover/{token} | `recover::reset_post` | Consume token, update password |

## Security Notes

Honeypot: bots that auto-fill the hidden `website` field are silently accepted and redirected without storing any data (both `/subscribe` and `/recover` use the same pattern). The human-check checkbox and ToS agreement checkbox are both server-side validated. Password complexity is enforced identically to admin user creation. Duplicate-email signups to a site the user isn''t yet linked to succeed silently (adding a `site_users` row); duplicate-email signups to a site the user is already linked to return an explicit "already subscribed" message.

`/recover` deliberately excludes staff accounts (`super_admin`/`site_admin`/`editor`/`author`) — it silently no-ops for them exactly as it does for an unregistered email, rather than returning a distinct error, so the form can''t be used to fingerprint which addresses belong to staff. Relatedly, the public `/login` page''s own staff-account branch (tried to sign in as staff via the subscriber-facing form) was changed from a distinct "Staff accounts sign in at /admin/login." message to the same generic "Invalid email or password." used for any other failure — that branch only runs after a *correct* password match, so the distinct message was telling anyone testing valid credentials whether a given login belonged to a staff account.
', '2026-08-05 16:48:00.914408-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (23, 'observability', 'Observability & Metrics', '# Observability & Metrics

> Last updated: 2026-07-22 | Updated by: claude

## Overview

SynapCMS exposes application metrics in Prometheus text format at `GET /metrics`. Metrics are collected using the `metrics` crate with a `metrics-exporter-prometheus` backend. An optional bearer token protects the endpoint. HTTP request counts/durations are tracked globally via middleware; search query counts are tracked per query.

## How It Works

### Metrics Handler (`core/src/handlers/metrics.rs`)

`GET /metrics` — reads `state.metrics_handle.render()` (a `PrometheusHandle`) and returns the Prometheus text exposition format (version 0.0.4) with `Content-Type: text/plain; version=0.0.4; charset=utf-8`.

If `state.metrics_token` is `Some(token)`, the request must include `Authorization: Bearer <token>`; the provided value is extracted with `headers.get(AUTHORIZATION).and_then(...).and_then(|v| v.strip_prefix("Bearer "))`. A missing or incorrect token returns `401 Unauthorized` with a plain-text body. If `metrics_token` is `None`, the endpoint is open.

### Metrics Collected

| Metric | Type | Labels | Source |
|--------|------|--------|--------|
| `synaptic_http_requests_total` | Counter | `method`, `status` | `track_http_metrics` middleware (router.rs) |
| `synaptic_http_request_duration_seconds` | Histogram | `method` | `track_http_metrics` middleware (router.rs) |
| `synaptic_search_queries_total` | Counter | (none) | `search::render_search` (core/src/handlers/search.rs) |

The `track_http_metrics` Tower middleware records these on every request by extracting the method before calling `next.run(req)`, then reading the response status after completion.

### AppState Integration

`AppState` carries both `metrics_handle: PrometheusHandle` and `metrics_token: Option<String>`, both read-only after startup. The `PrometheusHandle` is initialized when `AppState` is constructed and shared via `Arc`.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /metrics | `metrics::metrics` | Prometheus metrics endpoint |

## Configuration

`metrics_token` in `AppConfig` (optional). Set via `METRICS_TOKEN` environment variable or `synaptic.toml`. If unset, the endpoint is open — restrict access at the Caddy/network level in production.

## Security Notes

Token comparison is a plain string equality check (`provided != Some(token.as_str())`) — not constant-time. Since this is a low-sensitivity metrics-scraping token (not an auth credential for user data), this is an acceptable tradeoff but worth noting if the token model changes.', '2026-08-16 15:39:27.952779-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (20, 'settings', 'System Settings', '# System Settings

> Last updated: 2026-08-21 | Updated by: claude

## Overview

System settings control global application behavior and are split into two stores: `app_settings` (global, non-site-scoped — app name, timezone, max upload size) and `site_settings` (per-site key/value pairs — site name, description, URL, language, active theme, posts-per-page, date format). The `/admin/settings` admin page has three tabs: General (app name + timezone, under one tab bundling both `general` and `localisation` sub-forms), Security (placeholder, no fields yet), and Advanced (max upload size, an on-demand Search Index rebuild button, plus Seed Users/Seed Posts/Clear Test Data dev-data tools). SMTP and most `AppConfig` fields are still not exposed in this UI (file/env-var + restart only). Access requires `can_manage_settings`.

## How It Works

### AppConfig (`core/src/config.rs`)

`AppConfig` is deserialized at startup via the `config` crate, loaded through `AppConfig::load()`: layer order is (1) serde field defaults, (2) `synaptic.toml` in the working directory (or the path in the `CONFIG_FILE` env var, file is optional), (3) environment variables (`config::Environment` with `__` separator, `.env` loaded via `dotenvy`) — later layers win.

Fields: `host` (default `0.0.0.0`), `port` (default `3000`), `database_url` (required), `secret_key` (default is an insecure placeholder — must be overridden in production), `themes_dir` (default `themes`), `plugins_dir` (default `plugins`), `uploads_dir` (default `uploads`), `sites_dir` (default `sites` — the base for each site''s `{uuid}/themes/` and `{uuid}/uploads/` subdirectories), `dev_mode` (bool, default false), `log_level` (default `info`), `log_format` (default `text`), `search_index_path` (default `search-index`), `pid_file` (default `synaptic.pid`, used by `synap` for live reload), `caddyfile_path` (default `/etc/caddy/Caddyfile`, used for SSL provisioning from the admin panel), `metrics_token` (optional bearer token for `/metrics`), `max_upload_mb` (default `25` — since 2026-08-05 this is only a first-boot seed value for the DB-backed `app_settings.max_upload_mb`; once an admin saves a value on the Advanced tab, the DB value is authoritative and this field is no longer consulted for enforcement, only as a fallback), `admin_email` (optional, reply-to/notification address), and a full SMTP block (`smtp_host`, `smtp_port` default `587`, `smtp_username`, `smtp_password`, `smtp_from_name`, `smtp_from_email`, `smtp_encryption` default `starttls`) — outbound mail is disabled entirely if `smtp_host` is unset, and password-reset/form-notification code paths log a warning instead of sending. `bind_addr()` composes `host:port`.

### AppState (`core/src/app_state.rs`)

`AppState` is `Clone` (internally `Arc`-wrapped) and passed to every handler via the `State` extractor. Fields: `db: PgPool`, `templates: TemplateEngine`, `settings: Arc<SiteSettings>` (default/fallback), `config: Arc<AppConfig>`, `cookie_key` (HMAC signing key for post-unlock session cookies), `plugin_routes: Arc<HashMap<String, RouteRegistration>>`, `search_index: Arc<SearchIndex>`, `loaded_plugins: Arc<Vec<LoadedPlugin>>`, `active_theme: Arc<RwLock<String>>` (live-updated on theme switch), `site_cache: Arc<RwLock<HashMap<String, (Site, SiteSettings)>>>` (hostname-keyed), `metrics_handle: PrometheusHandle`, `metrics_token: Option<String>`, `app_settings: Arc<RwLock<AppSettings>>` (hot-reloadable), and `view_buffer: mpsc::UnboundedSender<(Uuid, String, NaiveDate)>` (a lock-free channel feeding a background task that batches post-view writes into `post_views` every 60s — deliberately not an `Arc<Mutex<HashSet>>` because a blocking std Mutex in async code starves the whole Tokio thread pool under load).

`SiteSettings` (per site): `site_name`, `site_description`, `base_url` (from key `site_url`), `language` (from `site_language`), `active_theme`, `posts_per_page` (i64, parsed with fallback), `date_format`. Loaded via `SiteSettings::load(pool, site_id)` (filters `site_settings WHERE site_id = $1`) or `SiteSettings::load_global(pool)` (filters `WHERE site_id IS NULL`, used at startup before any site is configured / for legacy pre-migration rows).

`AppSettings` (global): `app_name`, `timezone`, `max_upload_mb` — loaded from the un-scoped `app_settings` table.

Helper functions `set_app_setting(pool, key, value)` and `set_site_setting(pool, site_id, key, value)` upsert into their respective tables. `set_site_setting` uses `ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL` targeting a partial unique index, reflecting that `site_settings.site_id` can be `NULL` for legacy/global rows.

`AppState` methods: `resolve_site(hostname)`, `active_theme_for_site(site_id)` (falls back to the global in-memory `active_theme` if the site isn''t cached), `site_hostname(site_id)`, `get_site_by_id(site_id)` (linear scan of the cache), `update_site_theme_in_cache(site_id, theme)` (called after a theme activation so static asset serving picks up the change without a restart), `reload_app_settings()` and `reload_site_cache()` (re-read from the DB into the in-memory caches).

### Admin Settings Handler (`core/src/handlers/admin/settings.rs`)

- `settings` (`GET /admin/settings`) — requires `can_manage_settings`; reads `app_name`, `timezone`, and `max_upload_mb` from the cached `app_settings` RwLock (all three are DB-backed and hot-reloadable now), plus `admin_email` straight from `AppConfig` (still read-only in this UI — no save path for it), and renders `admin::pages::settings::render`.
- `save_settings` (`POST /admin/settings`) — requires `can_manage_settings`; handles three `tab` values: `"general"` (saves `app_name`), `"localisation"` (saves `timezone`), and `"uploads"` (saves `max_upload_mb`, validated to be an integer between 1 and 1000). Each case calls `set_app_setting` then `state.reload_app_settings()` so the change is live immediately, no restart. Any other `tab` value re-renders the page unchanged (no-op).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/settings | settings::settings | View settings (General tab) |
| POST | /admin/settings | settings::save_settings | Save settings (only `tab=general` is processed) |

## Database Schema

- `app_settings`: `key TEXT` (PK), `value TEXT`. Keys written by this handler: `app_name`, `timezone`, `max_upload_mb`. `AppSettings::load` seeds `max_upload_mb` from `AppConfig.max_upload_mb` only if no row exists yet (first boot) — once a row exists, the DB value is authoritative.
- `site_settings`: `site_id UUID` (nullable — legacy/global rows), `key TEXT`, `value TEXT`, with a partial unique index on `(site_id, key) WHERE site_id IS NOT NULL` used for per-site upserts. Keys read by `SiteSettings::load`/`load_global`: `site_name`, `site_description`, `site_url`, `site_language`, `active_theme`, `posts_per_page`, `date_format`.

## Configuration

All `AppConfig` fields are set via `synaptic.toml` or environment variables (env vars win) — see the field list above. `SECRET_KEY` must be overridden in production (the compiled-in default is an insecure placeholder). `MAX_UPLOAD_MB`/`max_upload_mb` in `.env`/`synaptic.toml` only matters on first boot now (see above) — after that, change it from the Advanced tab on `/admin/settings` instead, which takes effect immediately.

## Security Notes

- `can_manage_settings` gates both routes; unauthorized requests get a 403 with a plain HTML body.
- `SECRET_KEY` and SMTP credentials are environment/file-only — never exposed in the admin UI or written to the database.
- The settings page still displays `admin_email` read-only from `AppConfig`; there is no handler code path that lets an admin change it through this UI (config-file/env-var edit + restart is required). `max_upload_mb` is no longer in this category — it moved to the DB-backed, hot-reloadable `app_settings` store (Advanced tab) on 2026-08-05.
- The actual upload size cap is enforced by a dynamic `core/src/middleware/upload_limit.rs` layer that re-checks the live `app_settings.max_upload_mb` value against each request''s `Content-Length`, paired with a fixed 1GB `DefaultBodyLimit` as an absolute safety net against unbounded/chunked bodies. See the middleware doc for the full layering.

## Known Limitations / TODOs

- `save_settings` implements `general`, `localisation`, and `uploads`; any other `tab` value is accepted by the form but silently produces no change (falls through to the "re-render unchanged" branch). The Security tab has no fields yet — it''s a placeholder for session timeout/login lockout/password policy config.
- Per-site maintenance mode and IP allow/block-list configuration (added in recent middleware work) are not part of this handler — they live in `core/src/middleware/maintenance.rs`, `core/src/middleware/ip_allowlist.rs`, and `core/src/middleware/ip_denylist.rs`, which are documented separately (middleware), not under this System Settings doc.

', '2026-08-21 17:27:34.529427-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (21, 'search', 'Search', '# Search

> Last updated: 2026-08-22 | Updated by: claude

## Overview

Full-text search is powered by Tantivy, an embedded Rust search engine (no external Elasticsearch dependency). The index is rebuilt from all published posts on startup and kept in sync on every publish, update, or delete operation. Search queries are capped at 25 characters. Results are filtered by `site_id` to enforce multisite isolation.

## How It Works

### Index (`core/src/search/index.rs`)

`SearchIndex` wraps a Tantivy `Index` with a thread-safe `Arc<RwLock<IndexWriter>>`. The schema has six fields: `id` (STRING STORED), `site_id` (STORED), `title` (indexed+stored), `content` (indexed only), `slug` (STORED), `post_type` (STRING STORED).

A custom tokenizer chain named `"en_stop"` is registered on the index: `SimpleTokenizer` → `LowerCaser` → `StopWordFilter` (removes ~70 common English stop words, defined in the `EN_STOP_WORDS` static) → `Stemmer(English)`. This is applied at both index time and query time.

Key methods:
- `open_or_create` — opens existing index or creates new; detects schema mismatch (e.g. tokenizer changed) and wipes/recreates the index directory
- `search(query_str, site_id, limit)` — parses query via `QueryParser` on title+content fields; falls back to `parse_query_lenient` on parse errors (special characters like `+`, `-`, `:`); ORs in an as-you-type prefix match on the last word (see below); fetches `limit * 4 + 20` docs when `site_id` filtering is needed, then post-filters by `site_id`; returns `Vec<SearchResult>` (id, title, slug, post_type, score)
- `rebuild_all` — deletes all documents, batch inserts, single commit (used for startup rebuild — avoids one disk flush per document)
- `upsert` — delete-by-id-term then add document, commit
- `delete` — delete by id term, commit

Tantivy only allows a single `IndexWriter` to hold the index directory''s lockfile at a time (in the same process or a different one). `SearchIndex::open_or_create` acquires that writer up front and holds it for the caller''s whole lifetime — this is why a second process (e.g. the CLI, see below) can''t open the index while the server is already running.

### Indexer (`core/src/search/indexer.rs`)

- `index_post` — strips HTML via `ammonia::clean_text`, calls `index.upsert`
- `delete_post` — calls `index.delete`
- `rebuild_index(index, pool) -> Option<usize>` — async function that fetches all posts with `status = ''published''` from the DB and calls `index.rebuild_all`; returns the number of documents indexed, or `None` on failure. Runs as a background task on startup so it doesn''t block server start with large post counts, and is also callable on demand (see On-Demand Reindex below) — added 2026-08-21 so a full rebuild no longer requires waiting for the next process start.

### On-Demand Reindex (added 2026-08-21)

Two ways to trigger `rebuild_index` outside of startup, for content added or changed outside the normal admin handlers (a WordPress import, a seed script, a direct DB write) that would otherwise stay unsearchable until the next restart:

- **Admin UI** — Settings → Advanced → "Search Index" card → "Rebuild Search Index" button (`super_admin` only). Calls `POST /admin/settings/dev-tools/reindex-search` (`core/src/handlers/admin/dev_tools.rs::reindex_search`), which clones the running server''s own `Arc<SearchIndex>`/`PgPool` from `AppState` and awaits `rebuild_index` in-process — no second writer involved, so it works anytime the app is up. Returns `{"ok": true, "indexed": <count>}` as JSON, shown in the card.
- **CLI** — `synap search reindex` (`cli/src/commands/search.rs`). Loads `AppConfig` (for `database_url` and `search_index_path`), opens its own `PgPool` and `SearchIndex`, and calls `rebuild_index`. Because of the single-writer constraint above, this **only works while the app is stopped** — if the server is running, `open_or_create` fails with a Tantivy `LockBusy` error, and the CLI surfaces a message pointing at the admin UI button as the live-app alternative. Intended for offline/scripted use (e.g. immediately after a bulk import or DB restore performed with the app down).

### As-You-Type Prefix Matching (added 2026-08-22)

Because the index stores stemmed terms, an exact-word search requires typing (or stemming down to) the whole word — e.g. matching "Advanced" required typing all the way to its stem `advanc`, since neither the query parser nor the index does substring/prefix matching. `search()` now also extracts the last whitespace-separated word of the query (`last_token_prefix`: lowercased, punctuation stripped, `None` if under 2 chars) and ORs a `tantivy::query::PhrasePrefixQuery` for that word into the result, one per searched field (title, content), alongside the normal parsed query.

This needed no schema change, no new field, and no reindex: a `PhrasePrefixQuery` built from a single term degrades internally to a `RangeQuery` over the term dictionary bounded by the prefix bytes (see tantivy''s `PhrasePrefixQuery::weight` — the phrase-adjacency path only applies when 2+ terms are supplied), which is a cheap FST prefix walk capped at the default 50-term expansion, not a full-index scan. It also still matches correctly against the *stemmed* dictionary without stemming the query itself: English suffix-stripping stemmers never modify the front of a word, so a lowercased raw prefix of the user''s in-progress word remains a valid byte-prefix of the stemmed dictionary term it will eventually complete into (e.g. `"adv"` is a valid prefix of the stem `"advanc"`).

This was chosen over indexing edge n-grams for real substring autocomplete — n-grams give true substring matching but multiply the indexed token count (and therefore index size and rebuild time) roughly by average word length; the prefix-query approach adds negligible per-query cost with zero storage/indexing overhead, at the cost of only matching from the start of a word, not the middle. Covered by unit tests in `core/src/search/index.rs`''s `mod tests`.

### Search Handler (`core/src/handlers/search.rs`)

`GET /search?q=...` — enforces the 25-character query limit server-side (mirrors the HTML input''s `maxlength`), calls `state.search_index.search(&query, Some(&site_id_str), 20)`, then fetches full `Post` records from the DB by the returned IDs (re-verifying `status == "published"` before including them) and builds a `PostContext` per result via `build_post_context`. Renders `search.html` with `query`, `results`, and `result_count` context variables, plus the standard nav/session/site context. Increments the `synaptic_search_queries_total` metric on non-empty queries. Renders active-plugin hook outputs (`head_start`, `head_end`, `body_start`, `body_end`, `before_content`, `after_content`, `footer`) for the resolved theme before returning HTML.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /search | `search::search` | Full-text search |
| POST | /admin/settings/dev-tools/reindex-search | `dev_tools::reindex_search` | Rebuild the search index on demand (`super_admin` only) |

## Configuration

`search_index_path` in `AppConfig` — path to the Tantivy index directory (default: `search-index`).

## Known Limitations / TODOs

The index does not support phrase queries or exact-match strings out of the box. Stop-word-only searches (e.g. "the") return zero results by design. Schema changes require a full index rebuild. Posts created outside the normal admin handlers (seed scripts, direct SQL, imports) are no longer stuck waiting for a restart — use the admin UI''s "Rebuild Search Index" button while the app is running, or `synap search reindex` while it''s stopped (see On-Demand Reindex above).

', '2026-08-22 15:59:22.811389-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (30, 'account', 'Account Area', '# Account Area

> Last updated: 2026-07-22 | Updated by: claude

## Overview

The account area (`/account/*`) is for authenticated subscribers (any logged-in role via the `AccountUser` extractor). It provides a dashboard, profile management, a saved posts reading list, and comment history. It is rendered entirely in Rust (`admin/src/pages/account.rs`), never through a site''s Tera theme — because it handles authenticated user data (profile, passwords), a site admin cannot modify these templates.

## How It Works

### Dashboard

`GET /account` — the default landing page. Renders a welcome message with the subscriber''s display name.

### Profile Management

`GET /account/profile` shows the profile view; the form fields appear in this order: **Display Name**, then **Email** (reordered from the previous Email-first layout). `POST /account/profile/update` accepts `email` and an optional `display_name`, builds an `UpdateUser` (leaving `username`, `password_hash`, `role`, and `bio` untouched), and on success shows the flash message **"Profile updated!"** — the wording was simplified from a longer message in a recent change. On failure it shows "Error saving profile. Please try again."

`POST /account/profile/change-password` requires `current_password`, `new_password`, `confirm_password`. It verifies the new/confirm passwords match, verifies the current password via `account.user.verify_password`, validates the new password with `validate_password` (8–12 chars, uppercase, digit, symbol), and hashes it with `hash_password` before updating. Flash: "Password changed successfully!" or an error string.

### Saved Posts

`GET /account/saved-posts` — paginated (20/page) list of posts the subscriber has saved, with an optional `search` query param and a `partial=1` mode that returns only the inner list fragment (used by the live-search JS). Each row shows a view link to the post URL and an unsave form. `derive_unsave_url()` strips the scheme/host from the stored absolute post URL and appends `/unsave`, pointing at the public `POST /{slug}/unsave` route (`core/src/handlers/post.rs`), which removes the post from the subscriber''s reading list.

### My Comments

`GET /account/my-comments` — paginated (20/page) list of comments the subscriber has posted, with the same `search` and `partial=1` live-search pattern. View icon links to `/{slug}#comments`. `POST /account/comments/{id}/delete` soft-deletes the subscriber''s own comment, but only if: the comment belongs to them (`is_owner`), it was created within the last 15 minutes (`within_window`), and it isn''t already deleted.

Both Saved Posts and My Comments use small inline JS (`crate::live_search_script`) for progressive enhancement: 300ms-debounced `fetch()` calls that swap the list `<div>` without a full page reload. This is an intentional placeholder — when the account pages are ported to Leptos, the JS is meant to be replaced with reactive signals/server functions.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /account | `account::dashboard` | Dashboard |
| GET | /account/profile | `account::profile_view` | View profile |
| POST | /account/profile/update | `account::profile_update` | Update profile |
| POST | /account/profile/change-password | `account::profile_change_password` | Change password |
| GET | /account/saved-posts | `account::saved_posts` | Saved posts list (supports `?search=`, `?page=`, `?partial=1`) |
| GET | /account/my-comments | `account::my_comments` | Comment history (supports `?search=`, `?page=`, `?partial=1`) |
| POST | /account/comments/{id}/delete | `account::delete_comment` | Delete own comment (15-minute window) |
| GET | /account/logout | `auth::account_logout` | Log out |

## Security Notes

- All routes require an active account session via the `AccountUser` extractor.
- Comment deletion is scoped to the authenticated user''s own comments and time-limited to 15 minutes after posting.
- Password changes require the current password to be re-verified server-side before a new hash is written.', '2026-07-22 19:48:52.874972-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (8, 'middleware', 'Middleware & Auth', '# Middleware & Auth

> Last updated: 2026-08-18 | Updated by: claude

## Overview

Middleware handles cross-cutting concerns: site resolution, admin authentication, account
authentication, per-site maintenance mode, and per-site IP allow/deny lists. All middleware
lives under `core/src/middleware/` (`account_auth.rs`, `admin_auth.rs`, `ip_allowlist.rs`,
`ip_denylist.rs`, `maintenance.rs`, `site.rs`).

## How It Works

### Site Resolution (`site.rs`)

`CurrentSite` is an Axum extractor implementing `FromRequestParts<AppState>`. It resolves the
current site from the `Host` request header.

1. Parse the `Host` header; strip port for DB lookup (`beth.com:3000` -> `beth.com`).
2. Check the in-memory `site_cache` via `state.resolve_site()`.
3. On a cache hit, validate against the DB via `site::get_by_hostname()`.
   - Valid: return `CurrentSite` immediately.
   - Stale (DB miss): reload cache via `state.reload_site_cache()` and retry once. If still not
     found, return `SiteResolutionError::UnknownHostname` (404) — it does **not** fall back to
     the empty-cache/default-theme path.
4. No cache entry at all: return `SiteResolutionError::UnknownHostname` (404).

`base_url` is derived from the configured `site_url` in DB settings if it differs from the
localhost default, otherwise from the raw `Host` header value (preserving port).

### Admin Auth (`admin_auth.rs`)

`AdminUser` is the `FromRequestParts<AppState>` extractor required by every admin handler (the
old doc''s `RequireAdmin` name no longer exists in source). It:

1. Reads `admin_user_id` (`SESSION_USER_ID_KEY`) from the session; redirects to `/admin/login`
   (`AdminAuthError::NotAuthenticated`) if absent or the user lookup fails.
2. Rejects with `403 Forbidden` unless the user''s global `role` is one of
   `super_admin | site_admin | editor | author`.
3. Resolves the **current site** (`site_id`): prefers `current_site_id` from the session
   (re-validated against the DB, clearing the key if the site was deleted); otherwise, for a
   global admin, resolves via the request''s `Host` header (reloading the stale site cache once
   on a miss) or falls back to the first site in the DB; for a non-admin site user, picks their
   first accessible site from `site_user::list_for_user`. The resolved id is written back into
   the session. This user/site resolution is shared (`resolve_user_and_site`) with the lighter
   `PickRoleUser` extractor used by `/admin/pick-role` (added 2026-08-18, see below).
4. Resolves `site_role: Option<SiteRole>` for that site (added 2026-08-18, was a plain `String`
   before). Global admins are always `Some(SiteRole::Admin)`, resolved without ever consulting
   `site_users`. Otherwise, `site_user::list_roles_for_user_and_site` is queried live (no
   caching in the session cookie): 0 roles falls back to the user''s global role parsed as a
   `SiteRole` (yields `None` for `site_admin`/`super_admin`, since those aren''t valid site
   roles); 1 role is used directly; ≥2 roles requires a `SESSION_CURRENT_ROLE_KEY` session value
   that still matches one of the currently-held roles, re-checked on every request — if missing
   or stale (e.g. the pinned role was since revoked), the extractor returns
   `AdminAuthError::RolePickRequired`, which redirects to `GET /admin/pick-role` instead of
   denying the request outright. See the "Multiple roles per user per site" section of the
   Users & Roles doc for the full picker flow, including why `SiteRole` has no
   `super_admin`/`site_admin` variant.
5. Builds `AdminCaps` via `AdminCaps::from_roles(global_role, site_role, visiting_foreign,
   is_on_default_site)` — a single capability struct (`is_global_admin`, `is_impersonating`,
   `can_manage_users`, `can_manage_sites`, `can_manage_plugins`, `can_manage_settings`,
   `can_manage_content`, `can_manage_themes`, `can_manage_taxonomies`, `can_manage_forms`,
   `can_manage_pages`) computed once at the auth boundary and passed downstream rather than
   recomputed per handler. `can_manage_settings` requires both global-admin **and**
   `is_on_default_site` (system settings are restricted to a super_admin''s own default/home
   site). `is_impersonating` is true when a super_admin is viewing a site other than their
   `default_site_id` (drives the "visiting" badge in the admin UI).

### Account Auth (`account_auth.rs`)

`AccountUser` extractor for any authenticated non-admin user (subscriber and above), keyed on
its own session key `account_user_id` (`SESSION_ACCOUNT_USER_ID_KEY`) — entirely separate from
the admin session key, so a browser can be logged into `/admin` and `/account` as two different
users simultaneously. Also resolves `site_id`, `site_name`, and `site_base_url` from the `Host`
header via `state.resolve_site()` for "back to site" links. Rejects to `/login`.

### Session Timeouts & Logout

Admin and account logins use two entirely separate `tower_sessions` cookies/layers
(`core/src/main.rs`), sized to risk level per OWASP session-management guidance rather than
sharing one timeout: higher-privilege accounts get a shorter leash.

- **`admin_session`** — 2h inactivity timeout. Used by `/admin/*`.
- **`session`** — 24h inactivity timeout. Used by everything else (public content, `/login`,
  `/account/*`).

Both are `Expiry::OnInactivity` **and** `with_always_save(true)`. The `always_save` flag is
required for "inactivity" to mean what it says: `tower_sessions` only recomputes a session''s
expiry when a request *writes* to it, and most page views (`AdminUser`/`AccountUser` extractors)
only *read* the session to check who''s logged in. Without `always_save`, the timeout would
silently behave like a fixed timer from login instead of a rolling window — an actively-working
admin would still get booted at a fixed point regardless of activity.

Because a single shared session''s expiry can''t vary by login type (the layer''s config, not the
DB record, determines expiry on every save), two distinct `SessionManagerLayer`s were required.
`router.rs::build()` reflects this: routes are split into `public_router` (content routes,
`/login`, `/account/*`, static/plugin routes, and the `page::single_page` fallback) and
`admin_router` (`/admin/*`), each wrapped with its own session layer *before* being merged —
any handler extracting `Session` must live in the group whose layer actually inserts that
extension, or extraction fails at runtime. (This bit a first pass: the fallback route was
originally registered on the merged router, outside both layers, and 500''d on any unmatched
`/{slug}` since `page::single_page` extracts `Session`.)

`logout` (`auth::logout`, `auth::account_logout`) calls `session.flush()`, not
`session.remove(key)`. Removing just the auth key looked like a fix but wasn''t: it leaves the
session record (and cookie) alive, and `tower_sessions`'' `is_empty()` check only triggers cookie
removal when a request arrives with *no* session id at all — a request carrying an existing
cookie never qualifies, even with an empty data map. The old code was quietly **renewing** a
full-length "logged out" cookie on every logout instead of invalidating it. `flush()` deletes the
store row, clears the session id, and does trigger `is_empty()` — the browser gets a real
`Set-Cookie: ...; Max-Age=0` and the DB row is gone, not just emptied.

### Maintenance Mode (`maintenance.rs`)

`gate()` is a `middleware::from_fn_with_state` layer applied globally in `router.rs`. For every
request it checks (live, no cache — a single indexed `site_settings` query) whether the
resolved site has `maintenance_mode = ''true''`; if so it renders a branded 503 page (custom
`maintenance_message` setting, default message, `Retry-After: 3600`) instead of continuing.
`/admin*`, `/theme/static*`, `/uploads*`, and `/metrics` are always exempt (`is_exempt()`) so an
operator can still log in to disable it and so the maintenance page''s own assets still load.
Requests with no resolvable `Host` or site pass through unaffected. Toggled via
`synap site maintenance on/off` — takes effect immediately, no restart.

### IP Allowlist (`ip_allowlist.rs`) / IP Denylist (`ip_denylist.rs`)

Two symmetric per-site gates, also applied globally and checked live (no cache) on every
request:
- **Allowlist**: if `ip_allowlist_enabled = ''true''` for the resolved site, the caller''s IP must
  match an entry in the comma-separated `ip_allowlist` setting (parsed as bare IPs or CIDR
  ranges, IPv4/IPv6, via the shared `matches_entry()`) or the request is rejected with a
  branded `403` page.
- **Denylist**: inverse — if enabled, an IP matching `ip_denylist` is rejected with `403`;
  everyone else passes.

Both derive the client IP via the shared `real_ip()` helper: prefers `X-Real-Ip`, then the
first hop of `X-Forwarded-For`, finally the raw socket `ConnectInfo` address — trusted because
Axum binds only to a private interface behind Caddy, so these headers cannot be forged by an
external caller. **Neither list exempts `/admin`** — unlike maintenance mode, locking yourself
out requires shell/SSH access to the box to disable it. Toggled via `synap site allow-ip
on/off` / `synap site block-ip on/off`.

### Layer Order

The per-route-group session layers (`account_session_layer` on `public_router`,
`admin_session_layer` on `admin_router`) are applied and merged first. The remaining layers wrap
the merged router, outermost-last (closest to `.with_state`):
`no_store_for_protected` → `maintenance_layer` → `ip_allowlist_layer` → `ip_denylist_layer` →
`track_http_metrics` → `TraceLayer`.

## Security Notes

- Stale site-cache entries (e.g. after `dev reset` without restart) are caught by DB validation
  in both `CurrentSite` and `AdminUser` and return 404 / re-resolve rather than serving or
  authenticating against a ghost site.
- `Cache-Control: no-store` is applied to all `/admin/*` and `/account/*` responses
  (`no_store_for_protected` in `router.rs`) to prevent back-button cache leakage after logout.
- Maintenance mode always exempts `/admin/*` so it can''t be used to lock out the operator;
  IP allowlist/denylist intentionally make no such exception.
- `real_ip()` trusts `X-Real-Ip`/`X-Forwarded-For` only because Caddy is the sole thing able to
  reach the Axum process — this assumption breaks if the app is ever exposed directly.
- Renaming the session cookies (`id` → `admin_session`/`session`) orphaned every pre-existing
  session on deploy — a one-time, expected side effect, not a bug. Old rows linger in
  `tower_sessions.session` (schema `tower_sessions`) until their original expiry passes; they''re
  inert since no layer reads a cookie named `id` anymore.

', '2026-08-18 09:09:14.174501-04', 'claude', 'system');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (26, 'cli', 'CLI Tool (synaptic-cli)', '# CLI Tool (synap)

> Last updated: 2026-08-21 | Updated by: claude

## Overview

`synap` is the maintenance and operations tool for SynapCMS. It handles initial installation, database migrations, user management, site management, plugin/theme inspection, search index rebuilds, and Caddy SSL permissions. It is a separate compiled binary from the main server and must be rebuilt and reinstalled whenever new migrations are added.

## How It Works

Built with `clap` using a subcommand tree. On startup it loads `.env` via `dotenvy` (non-fatal if missing). All database-touching commands call `commands::connect_db()`, which reads `DATABASE_URL` from the environment and opens a `PgPool` with `max_connections=2`. Migrations are embedded at compile time via `sqlx::migrate!("../migrations")` — adding a migration file requires rebuilding the binary (`cargo install --path cli --force`).

## Commands

### `install`
Interactive installation wizard. Collects domain, port, install directory, database URL, admin credentials, and branding. Steps performed:
1. Validates install directory ownership (Unix only — checks UID against dir owner).
2. Connects to the database and runs all pending migrations.
3. Creates the `super_admin` user (Argon2-hashed password, `is_protected=TRUE`).
4. Inserts the initial `sites` row with default `site_settings` (site_name, site_url, active_theme, posts_per_page, etc.).
5. Seeds `app_settings` (app_name, timezone, max_upload_mb).
6. Copies `themes/global/default/` into `themes/sites/<site_id>/default/`.
7. Sets `users.default_site_id` for the super_admin.
8. Writes a `Caddyfile` and `synaptic-signals.service` systemd unit from templates in `deployment_templates/` (or the current directory).
9. Optionally runs `caddy setup` if `--app-user` is provided.
10. Updates `.env` with `INSTALL_DIR`, `MAX_UPLOAD_MB`, and `ADMIN_EMAIL`.

Supports `--non-interactive` mode for scripted deployments; reads values from flags or env vars (`SYNAPTIC_DOMAIN`, `ADMIN_EMAIL`, `ADMIN_PASSWORD`, `DATABASE_URL`, `PORT`, `INSTALL_DIR`, `APP_NAME`, `APP_USER`, `NOTIFICATION_EMAIL`, `ADMIN_USERNAME`, `ADMIN_DISPLAY_NAME`, `SITE_URL`). If `ADMIN_PASSWORD` is omitted in non-interactive mode, a compliant password is generated and printed once.

`site_url` (used to build permalinks) defaults to deriving from `domain`+`port` (`http://domain` for 80, `https://domain` for 443, else `http://domain:port`) — this is wrong whenever a reverse proxy (Caddy) fronts the app on a different public port than Axum''s internal listen port, since it bakes the internal port into every link. Pass `--site-url`/`SITE_URL` explicitly (e.g. `https://example.com`) to override; interactive mode now also prompts for it directly (with the same derived value as the default) rather than only being settable via the non-interactive flag. Like all `site_settings`, this is `ON CONFLICT DO NOTHING` — fixing it via flag/prompt only affects new installs; an already-wrong value needs a manual `UPDATE site_settings SET value=... WHERE key=''site_url''` plus a service restart (settings are cached in memory at startup).

Password policy: 8–12 characters, at least one uppercase, one digit, one symbol from `!@#$%&*-_+`.

**Restart required after every `install` run.** A running server loads its site cache once at startup and does not watch the database — `install`''s DB writes (new site, new/updated settings) have no effect on an already-running process until it''s restarted (`systemctl restart synaptic-signals`, or `./app.sh restart` in dev). `install` now prints this reminder unconditionally after the summary, in both interactive and `--non-interactive` mode, since skipping it silently produces a confusing "No site found for hostname" on the homepage with no error anywhere.

**Interactive mode''s "Next Steps" is now aware of an already-deployed service.** It used to always print a generic 6-step "copy the binary/systemd unit/Caddyfile, run caddy setup, enable the service" checklist, even on a re-run against a VPS that''s already fully deployed and running — which read as required steps and was actively misleading (re-copying the freshly generated systemd unit in that case would have downgraded the service to run as whoever invoked `install`, often `root`, instead of the correct app user). It now checks whether `/etc/systemd/system/synaptic-signals.service` already exists: if so, it diffs the freshly generated Caddyfile/service unit against what''s live and prints only what actually changed (or a one-line "nothing to copy" if they match); the full checklist only appears for a genuinely fresh install with no unit installed yet.

**Always run `install` (and `dev reset`) as the actual service user**, not bare `root` — e.g. `sudo -u www-data bash -c ''cd /var/www/bckr.dev && ./synap install''` (use the relative/full path; `sudo`''s `secure_path` on RHEL/AlmaLinux strips `/usr/local/bin` so the bare `synap` symlink isn''t found under `sudo -u`). Running as root re-creates `search-index/`, `uploads/`, and `sites/` as root-owned; the live systemd service still runs as the app user and loses write access to the search index, causing it to crash-loop with `Error: Index already exists` until you `chown -R <app-user>:<app-user>` the install dir back.

**Re-running `install` without pinning `ADMIN_PASSWORD`/typing a new password silently rotates the admin password** — the user insert is `ON CONFLICT (email) DO UPDATE SET password_hash = ...`, so every re-run invalidates the previous password and prints a new `GENERATED_ADMIN_PASSWORD` (non-interactive) or prompts for a fresh one (interactive). Easy to lock yourself out by re-running `install` for an unrelated reason (e.g. just to test something) without noticing.

`install` is idempotent against a re-run on an existing site: the `sites` insert uses `ON CONFLICT (hostname) DO NOTHING`, then looks up the row''s real `id` by hostname before seeding `site_settings` — this lookup was added to fix a foreign-key violation that occurred on re-install when the freshly-generated (but never-inserted) UUID was used instead of the existing site''s actual id.

### `migrate`
Runs pending migrations against the database. Accepts `--database-url` flag or reads `DATABASE_URL` from the environment. Used when upgrading without re-running the full installer.

### `dev reset`
**Destructive — development only.** Wipes all data rows from every table except `_sqlx_migrations` and `documentation` (both are intentionally preserved). Verifies the `super_admin` password before proceeding. Shows a summary of what will be wiped (sites, user count, post count, media count). Also removes `themes/sites/`, `themes/private/`, and `uploads/` subdirectories if `INSTALL_DIR` is set. Supports `--force` to skip the confirmation prompt.

### `user create`
**Requires the super-admin password (added 2026-08-05)** — before any prompts, verifies a password (via `--password <PASSWORD>` for scripting, or an interactive prompt otherwise) against the current `is_protected = TRUE` user''s Argon2 hash. Bails with a clear error if no super_admin exists yet (run `synap install` first). Having server/DB access to run the CLI is no longer sufficient on its own to mint accounts — reuses a new shared `verify_super_admin_password` helper in `cli/src/commands/mod.rs`, the same pattern `dev reset` already used.

Interactively creates a new user. Prompts for:
- **Username** (8–15 chars, lowercase/digits/hyphens) — validated with a retry loop, mirroring `validate_username` from the web forms (duplicated locally in `cli/src/commands/user.rs` since the CLI doesn''t depend on the core crate).
- **Email**.
- **Display name** (≤60 chars, defaults to the username) — validated with a retry loop, mirroring `validate_display_name`.
- **Password** (8–12 chars, uppercase, digit, symbol from `!@#$%&`) — validated as before.
- **Role** (super_admin / editor / author / subscriber).
- **Site assignment (added 2026-08-05)** — for any role except `super_admin` (which has global access, not site-scoped), if any sites exist, prompts to assign the new user to one (or leave "(Unassigned)", the previous default/only behavior). Writes a `site_users` row with `invited_by = NULL` (CLI-seeded, no attributable inviter) using the same role string. Previously there was no way to do this at creation time — new CLI users always started unassigned, requiring a separate step via the admin UI''s Site Access page.

Hashes password with Argon2. Choosing `super_admin` also sets `is_protected=TRUE` on the row (fixed 2026-07-22) — previously only `install`''s admin-creation step set this flag, so a super_admin created via `user create` instead of `install` silently ended up unprotected: invisible to `dev reset`''s super_admin lookup (`WHERE is_protected = TRUE`, which would then falsely report "database already reset"), exempt from the delete-protection checks in the admin Users page, and unable to be picked up by the owner-auto-assignment logic in `site create`/site rename (all of which key off `is_protected`, not `role`).

### `user list`
Lists all users (id, username, email, role, created_at) ordered by creation time.

### `user reset-password`
Looks up a user by email and sets a new Argon2-hashed password interactively.

### `site create`
Adds a new empty site by hostname. Auto-assigns the protected super_admin as owner. Optionally copies `themes/global/default/` into the new site''s theme folder (`--themes-dir`).

**Removed command: `site init`** (2026-07-22). It existed to backfill a pre-existing single-site database''s content with a `site_id` after multi-site migrations (0008–0011) ran, and to create that first site row — but `install` already creates the first site directly (`INSERT INTO sites ... ON CONFLICT (hostname) DO NOTHING`), making `site init` redundant for any install using the current `install` flow, fresh or otherwise: there has never been a pre-multisite install of this app (only the local dev machine and the shared test VPS, both routinely wiped/reinstalled), so the backfill case it existed for never occurred in practice. It was also a latent footgun — if ever run before `install` on a hostname `install` would later reuse, the site it created would end up with `owner_user_id = NULL` (no admin existed yet to claim ownership at that point), and `install`''s `ON CONFLICT (hostname) DO NOTHING` would then silently preserve that ownerless row instead of fixing it, permanently breaking the owner-gated `can_manage`/`is_owner` checks in `core/src/handlers/admin/sites.rs` for that site. New installs now provision their first site exclusively via `install`; additional sites via `site create` or the admin UI''s `/admin/sites/new`.

### `site list`
Lists all sites (id, hostname, post count) ordered by creation time.

### `site delete`
Deletes a site and all its content via `CASCADE`. Prompts for confirmation.

### `site maintenance on|off|status`
WordPress-style maintenance mode, toggled per site. `on [--hostname] [--message]` and `off [--hostname]` write `maintenance_mode` (`true`/`false`) and `maintenance_message` into `site_settings` via a plain upsert (`ON CONFLICT ... DO UPDATE`, unlike the install-time seed rows). `--hostname` is required only if more than one site exists — with a single site it''s auto-selected. `status` prints the current mode and stored message. If `--message` is omitted on `on`, the previous message is reused, falling back to a default WP-style sentence.

Scoped to the target site''s `site_id` only — other sites on the same multi-site install keep serving normally while one is in maintenance mode. Verified: with one site''s `maintenance_mode` set, a request for a different site''s hostname on the same server still returned 200.

Enforced by `core/src/middleware/maintenance.rs`, a global Axum middleware layered via `middleware::from_fn_with_state` in `router.rs`. It runs a live, uncached query (`SELECT value FROM site_settings WHERE site_id=$1 AND key=''maintenance_mode''`) on every request, resolving the site from the `Host` header via `state.resolve_site()` — deliberately **not** cached like `active_theme`, so the CLI toggle takes effect immediately with no restart and no reload signal. Exempts `/admin*`, `/theme/static*`, `/uploads*`, and `/metrics` so an operator can still log in to turn it back off and static assets keep loading. Renders a hand-written HTML page (not a Tera theme template) with a `503 Service Unavailable` status and `Retry-After: 3600` header.

### `site allow-ip on|off|add|remove|status`
Per-site IP allowlist — like an `.htaccess` Allow/Deny list. `on --ip <cidr>` (repeatable for multiple entries) blocks **all** traffic to the site except from the given IPs/CIDRs (IPv4 or IPv6, e.g. `203.0.113.9` or `10.0.0.0/8`); `off` restores open access; `status` prints on/off and the stored list. `add --ip <cidr>` appends a single entry without touching the rest of the list (and turns the allowlist on if it wasn''t already); `remove --ip <cidr>` deletes a single entry, leaving the rest in place. Values are written to `site_settings` as `ip_allowlist_enabled` (`true`/`false`) and `ip_allowlist` (comma-separated), via the same live-upsert pattern as `site maintenance`. `--hostname` is required only if more than one site exists.

`remove` **refuses to delete the last remaining IP** while the allowlist is still enabled — that would leave the allowlist on with nobody, including you, able to reach `/admin`. Run `allow-ip off` instead if the intent is to fully reopen the site.

Unlike maintenance mode, **`/admin` is not exempt** — if enabled and your own IP isn''t on the list, you lock yourself out of the admin too, with no remote escape hatch; recovery requires shell/SSH access to the server to run `allow-ip off` directly. This is intentional: the use case is hard isolation of a test/staging deploy (e.g. a VPS site you don''t want anyone else reaching yet), not a "the operator can still log in" gate like maintenance mode.

Enforced by `core/src/middleware/ip_allowlist.rs`, layered in `router.rs` so it runs *before* the maintenance-mode check (an IP block takes priority even if maintenance mode is also off). Determines the real client IP by checking `X-Real-IP`, then the first hop of `X-Forwarded-For`, finally the raw socket address — trustworthy here because Axum only binds to a private interface behind Caddy, so an outside caller can never reach Axum directly to forge those headers themselves. CIDR matching is a small hand-written IPv4/u32 and IPv6/u128 bitmask comparison (no external crate), shared with `block-ip` below. Verified locally and live on the VPS (bckr.dev): blocks by default, matching single IPs and CIDR ranges pass, a spoofed out-of-range `X-Real-IP`/`X-Forwarded-For` is rejected, another site on the same server is unaffected, `add`/`remove` adjust the list without disturbing other entries, and `off` restores access — all live, no restart.

**Examples:**
```
# Lock a VPS test deploy down to just your own IP (run as the app''s service user)
sudo -u www-data bash -c ''cd /var/www/bckr.dev && ./synap site allow-ip on --hostname bckr.dev --ip 203.0.113.9''

# Allow a CIDR range instead (e.g. an office network) — quotes matter, / is a shell no-op but some shells still complain without them
sudo -u www-data bash -c ''cd /var/www/bckr.dev && ./synap site allow-ip on --hostname bckr.dev --ip "203.0.113.0/24"''

# Allow more than one IP/CIDR at once (repeat --ip)
./synap site allow-ip on --hostname bckr.dev --ip 203.0.113.9 --ip 198.51.100.0/24

# Trust a second teammate''s IP without retyping the whole list
./synap site allow-ip add --hostname bckr.dev --ip 198.51.100.42

# That teammate leaves — remove just their IP, yours stays allowed
./synap site allow-ip remove --hostname bckr.dev --ip 198.51.100.42

# Check what''s currently allowed
./synap site allow-ip status --hostname bckr.dev
#   Site: bckr.dev
#   IP allowlist: ON
#   Allowed: 203.0.113.9,198.51.100.0/24

# Done testing — reopen the site to everyone
./synap site allow-ip off --hostname bckr.dev

# Single-site installs can omit --hostname entirely (auto-selected):
./synap site allow-ip on --ip 203.0.113.9
```

### `site block-ip on|off|add|remove|status`
The inverse of `allow-ip` — an IP **denylist**: everyone can reach the site *except* the given IPs/CIDRs. Same subcommand shape as `allow-ip` (`on --ip <cidr>` replaces the whole list and turns it on, `off` disables, `add`/`remove` adjust one entry at a time, `status` reports state), backed by `ip_denylist_enabled` / `ip_denylist` in `site_settings`.

Unlike `allow-ip remove`, `block-ip remove` **auto-disables** the denylist when the last entry is removed — an empty denylist safely means "block nobody," so there''s no lockout risk to guard against. Use `allow-ip` when you want to restrict a site to a small trusted set (e.g. isolating a VPS test deploy); use `block-ip` when the site should stay public but a specific IP (e.g. an abusive scraper) needs to be kept out.

Enforced by `core/src/middleware/ip_denylist.rs`, layered in `router.rs` outermost of the three IP/maintenance gates (denylist checked before allowlist, before maintenance). Reuses `real_ip()` and `matches_entry()` from `ip_allowlist.rs` (marked `pub(crate)`) rather than duplicating the header-parsing/CIDR logic. Verified locally and live on the VPS: a blocked IP gets 403 (including `/admin`), unrelated IPs still get 200, `add`/`remove` adjust the list correctly, and removing the last entry auto-flips `ip_denylist_enabled` back to `false`.

**Examples:**
```
# Ban a single abusive/scraping IP while the site stays public
./synap site block-ip on --hostname bckr.dev --ip 198.51.100.13

# Ban a whole subnet instead
./synap site block-ip on --hostname bckr.dev --ip "198.51.100.0/24"

# Ban more than one at once (repeat --ip)
./synap site block-ip on --hostname bckr.dev --ip 198.51.100.13 --ip 203.0.113.66

# Add one more bad IP later without disturbing the existing bans
./synap site block-ip add --hostname bckr.dev --ip 203.0.113.77

# That IP turned out to be a false positive — unban just it
./synap site block-ip remove --hostname bckr.dev --ip 203.0.113.77

# Check what''s currently blocked
./synap site block-ip status --hostname bckr.dev
#   Site: bckr.dev
#   IP denylist: ON
#   Blocked: 198.51.100.13,203.0.113.66

# Remove the last blocked IP — denylist auto-turns itself OFF (no explicit `off` needed)
./synap site block-ip remove --hostname bckr.dev --ip 198.51.100.13
./synap site block-ip remove --hostname bckr.dev --ip 203.0.113.66
#   Removed ''203.0.113.66'' from the denylist for ''bckr.dev''.
#   Denylist is now empty — turned OFF automatically.

# Or turn it off explicitly while keeping the list on file for later reuse
./synap site block-ip off --hostname bckr.dev
```

**allow-ip vs block-ip, side by side:**
| | `allow-ip` | `block-ip` |
|---|---|---|
| Default state | Blocks everyone | Lets everyone through |
| `--ip` entries mean | The only IPs let in | The only IPs kept out |
| Use case | Isolate a test/staging deploy to just you | Keep a public site open but ban a specific abuser |
| Removing the last IP | Refuses — would lock out `/admin` with no recovery | Auto-disables — safe, since empty = block nobody |

### `plugin list`
Reads `plugin.toml` manifests from `./plugins/` subdirectories and prints name, version, api_version, and description for each.

### `theme list`
Reads `theme.toml` manifests from `themes/global/` and `themes/sites/*/`. Marks the active theme with `*` (compares against `ACTIVE_THEME` env var). Falls back to flat `themes/` for pre-multisite installs.

### `theme activate <name>`
Finds a theme by its `theme.toml` name field across `themes/global/` and `themes/sites/*/`. Updates `site_settings` in the database and sends `SIGUSR1` to the running server (reads PID from `synaptic.pid`) to trigger a live template reload without restart.

### `theme reload`
Sends `SIGUSR1` to the running server to reload templates from disk without a restart.

### `search reindex` (added 2026-08-21)
Rebuilds the Tantivy search index from the database (all published posts/pages, every site) — same rebuild the server runs once at startup, just triggered manually. Loads `AppConfig` (`database_url`, `search_index_path`), opens its own `PgPool` and `SearchIndex`, calls `search::indexer::rebuild_index`, and prints the number of documents indexed.

**Only works while the app is stopped.** Tantivy allows exactly one `IndexWriter` on the index directory at a time, and a running server holds it open for its entire process lifetime — so `open_or_create` fails with a `LockBusy` error if the server is up, and the command prints a message pointing at the admin UI''s "Rebuild Search Index" button (Settings → Advanced) as the live-app alternative. Intended for offline/scripted use, e.g. right after a bulk import or DB restore done with the app down:
```
./app.sh stop && synap search reindex && ./app.sh start
```

### `caddy setup`
Grants the app system user SSL provisioning capability. Runs as root: adds the user to the `caddy` group (`usermod -aG`), makes `/etc/caddy/Caddyfile` group-writable, creates `/var/log/caddy/` with `caddy:caddy` ownership. Idempotent. Also called automatically by `install` if `--app-user` is provided.

### `caddy teardown`
Reverses `caddy setup`: removes `/etc/sudoers.d/synaptic-caddy`, restores Caddyfile to `640`, removes the app user from the `caddy` group.

## Deploying to a VPS (`scripts/install-vps.sh`)

A local-machine driver script that builds this repo and pushes it to a test VPS over SSH, invoking `synap` remotely to finish the install. Replaces the deleted `deploy-vps.sh` (2026-08-05) — an interactive installer wizard: a welcome screen choosing default vs. interactive settings, validated field prompts, an upfront pass/fail requirements table (local toolchain + remote systemd/Caddy/Postgres13+/passwordless-sudo/glibc — checked before anything destructive runs), per-step progress, and a final summary with the admin login (shown once) and next steps. Always rebuilds fresh before shipping, since migrations are compiled into the binary via `sqlx::migrate!` rather than read from disk at runtime — a binary built before a migration existed will never apply it even if the `.sql` file is present on the target machine. Full usage/examples: `./scripts/install-vps.sh --help`.

### Flags
- Default (no flags, TTY present): welcome screen asks "use default settings or interactive setup?"
- `--defaults`: skip the menu, use built-in/env-var defaults, no prompts. Also automatic whenever stdin isn''t a TTY, so still CI-safe.
- `--interactive`: skip the menu, go straight to the field-by-field wizard.
- `--update` (renamed from `--no-install`): push a code update to an already-running install — rebuild, re-ship, apply pending migrations, restart. Does **not** create a site/admin or touch existing data. This is the intended production shape: get the app running first, then configure it via `synap install` as a separate deliberate step.
- `--clean`: force-drops the DB (`WITH (FORCE)`, PG13+) and wipes `INSTALL_DIR` entirely.

### Destructive-migration heads-up
Before applying migrations against an already-populated DB (skipped on `--clean`/fresh installs, since there''s nothing to lose), a static keyword scan (`DROP TABLE`/`DROP COLUMN`/`TRUNCATE`/`DELETE FROM`/`RENAME`) of not-yet-applied `.sql` files prompts to confirm in a TTY (declining aborts before the DB is touched), or just warns loudly in `--defaults`/non-TTY runs (surfaced in the final summary). It''s a heuristic scan, not a certified safety check.

### Known gotcha: running `synap install` by hand
Must run as the service user (`synap` checks it owns `$INSTALL_DIR`) and must be invoked via a relative/full path (`./synap install`), not the bare command — `sudo`''s `secure_path` on RHEL/AlmaLinux strips `/usr/local/bin`, so `sudo -u www-data synap install` fails with "command not found" even though the symlink exists.

### Fixed bug: `/theme/*` Caddy bypass 404''d every theme
`deployment/Caddyfile.template` (and its compiled-in fallback `cli/deployment_templates/Caddyfile.template`) used to have `handle /theme/* { file_server }`, bypassing Axum for performance. But every theme''s `base.html` links the identical `/theme/static/css/style.css` — there''s no theme name in the URL. Which theme''s files actually get served is resolved dynamically per-request in `core/src/handlers/theme_static.rs` (Host header → site → active theme), which a flat file_server can never do. Fixed by removing that block so `/theme/*` falls through to `reverse_proxy` → Axum. `/uploads/*` correctly stays on the Caddy bypass — the app maintains an `uploads/{hostname}/ → uploads/{site-uuid}/` symlink specifically so a flat file_server works there.

## Security Notes

- `dev reset` requires the current `super_admin` password (Argon2-verified) before wiping data.
- `caddy setup` / `caddy teardown` must run as root.
- Password generation excludes `$` and `!` to avoid shell variable expansion issues in env files and URL strings.
- Install-time admin users are inserted with `is_protected=TRUE` to prevent accidental deletion.

## Known Limitations / TODOs

- Migrations are embedded at compile time. Every new migration file requires `cargo install --path cli --force` before `synap migrate` or `install` will see it (or, for VPS deploys, an unconditional rebuild — see `scripts/install-vps.sh` above).
- `theme activate` updates `site_settings` using the old single-column conflict key (`ON CONFLICT (key)`) which may not work correctly in multi-site installs where `site_id` scoping is required.

', '2026-08-21 17:27:34.525196-04', 'claude', 'system');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (14, 'tags', 'Tags', '# Tags

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Tags are a taxonomy type used to label posts with fine-grained keywords. They are stored in the `taxonomies` table with `taxonomy = ''tag''`. Tags are site-scoped and otherwise behave identically to categories in storage and code paths.

## How It Works

Tags share all model functions in `core/src/models/taxonomy.rs` with categories — the `TaxonomyType::Tag` enum variant routes queries to filter `taxonomy = ''tag''`. Template context is built via `TermContext::from_taxonomy`, which produces a `url` of the form `{base_url}/tag/{slug}`.

### Admin Handler (`core/src/handlers/admin/taxonomy.rs`)

- `tags` — lists all tags for the site with published post counts. Requires `can_manage_taxonomies`.
- `create` — the same handler is used for both categories and tags. The `taxonomy` form field (`"category"` or `"tag"`) determines which type is created.

The "Add Tag" form (`admin/src/pages/taxonomy.rs::render`, updated 2026-07-22) shares its template with categories: same `.profile-container` card styling as `/admin/users/new`, submit disabled until Name is non-empty, and a live client-side slug preview as you type Name (stops auto-syncing once Slug is edited directly). Heading/button read "Add Tag" (previously "Add New Tags").
- `delete_tag` — enforces site ownership for non-global-admins before deletion.

### Post Editor Integration

When creating or editing a post, `fetch_term_options` queries both `TaxonomyType::Category` and `TaxonomyType::Tag` for the current site and passes them to the `PostEdit` view as selectable options. On save, `save_post_terms` detaches all existing taxonomy associations and reattaches the submitted set.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/tags | `taxonomy::tags` | List tags |
| POST | /admin/tags/new | `taxonomy::create` | Create tag |
| POST | /admin/tags/{id}/delete | `taxonomy::delete_tag` | Delete tag |
| GET | /tag/{slug} | `archive::tag_archive` | Public tag archive |

## Database Schema

Tags use the same `taxonomies` table as categories, with `taxonomy = ''tag''`. Post-tag associations are in `post_taxonomies`.

## Security Notes

Identical to categories: editor or above required, site isolation enforced on delete.', '2026-07-22 19:39:08.506676-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (10, 'pages', 'Pages', '# Pages

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Pages are static content items (About, Contact, Privacy Policy, etc.). They share the `posts` table with `post_type = ''page''`, differentiated from Posts by having no author-only restriction, supporting hierarchical nesting via `parent_id`, custom Tera templates, and (as of the Puck visual builder work) potentially being owned by a builder page composition instead of rendering through the classic theme templates.

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
- The editor additionally offers a **template picker** populated by `scan_templates()`, which recursively walks the active theme''s `templates/` directory and excludes reserved template names (`base`, `page`, `index`, `single`, `archive`, `search`, `404`) and anything under `partials/`.
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
- Non-global-admin delete/bulk-delete is scoped to the admin''s own site; authors may not delete published pages.', '2026-07-22 19:46:37.919119-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (11, 'comments', 'Comments & Replies', '# Comments & Replies

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Registered account users (subscribers or above) can submit comments on published posts that have `comments_enabled = true`. Comments support one level of threading via `parent_id` (replies to top-level comments only). Comments can be soft-deleted by their author or hard-deleted by admins with the `can_manage_content` capability.

## How It Works

### Submission (`core/src/handlers/comment.rs`, `POST /{slug}/comment`)

Steps performed by `submit`:
1. Requires an active account session (`SESSION_ACCOUNT_USER_ID_KEY`); redirects to `/login?redirect={post_url}` if absent.
2. Rejects if the "I''m human" checkbox (`human_check`) was not ticked.
3. Validates `body` is non-empty and trimmed length ≤ 400 chars.
4. Fetches the post and confirms `comments_enabled` is true.
5. Rate-limits to 2 comments (top-level or reply) per user per 10 minutes, checked via a live COUNT query against `comments.created_at`.
6. Records the submitter''s IP (`X-Real-IP` → `X-Forwarded-For` → socket address) into `ip_address` (migration 0031).
7. Inserts the comment and redirects to `/{slug}#comments`.

### Data Model (`core/src/models/comment.rs`)

`Comment` columns: `id`, `post_id`, `site_id`, `author_id`, `parent_id`, `body`, `ip_address`, `created_at`, `updated_at`, `deleted_at` (soft-delete, migration 0030). Body length is capped at the database level too (migration 0029).

`list_for_post()` builds a two-level (top-level + replies) tree and applies soft-delete display rules:
- A deleted **reply** is excluded entirely.
- A deleted **top-level comment with remaining (non-deleted) replies** is kept with body blanked and `is_deleted = true` (template renders "[deleted]").
- A deleted **top-level comment with no replies** is excluded entirely.

Results are paginated (10 per page, `CommentPage`).

### Account Comment History

`list_for_user` / `count_for_user` support the subscriber-facing "My Comments" page with search across comment body and post title (stop words stripped via `search_terms`), always excluding soft-deleted comments.

### Moderation

`POST /admin/comments/{id}/delete` — hard-delete, requires `admin.caps.can_manage_content` (redirects to `/admin` otherwise).
`POST /account/comments/{id}/delete` — soft-delete, only succeeds if the comment belongs to the requesting `author_id` and isn''t already deleted (`soft_delete` uses an atomic `UPDATE ... WHERE id = $1 AND author_id = $2 AND deleted_at IS NULL`).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /{slug}/comment | `comment::submit` | Submit comment (requires account session) |
| GET | /account/my-comments | `account::my_comments` | Subscriber''s own comment history (searchable) |
| POST | /account/comments/{id}/delete | `account::delete_comment` | Author self-delete (soft) |
| POST | /admin/comments/{id}/delete | `admin_comments::delete` | Admin hard delete |

## Database Schema

`comments` table (migration 0028, extended by 0029–0031): `id`, `post_id`, `site_id`, `author_id`, `parent_id` (self-referential, one level), `body` (length-limited), `ip_address`, `created_at`, `updated_at`, `deleted_at`.

## Security Notes

- Submission requires an active account session — no anonymous comments.
- Simple honeypot-style human check (`human_check` checkbox) rather than a CAPTCHA.
- Rate limit: 2 comments per user per 10-minute rolling window (checked against the DB directly, not the site-wide IP allow/block lists, which live in `core/src/middleware/ip_allowlist.rs` / `ip_denylist.rs` and apply at the request level, not comment-specific).
- Admin hard-delete requires the `can_manage_content` capability; author self-delete is scoped to their own `author_id` via the SQL `WHERE` clause, not just an application-level check.', '2026-07-22 19:46:37.931891-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (56, 'email-providers', 'Email Providers', '# Email Providers

> Last updated: 2026-08-15 | Updated by: claude

## Overview

A site can configure any number of named third-party email accounts — Mailgun, generic SMTP, SendGrid, Postmark — instead of the app being locked to a single install-wide Mailgun account. Each **form** (see the **Form Designer** doc) independently picks which configured provider its notify/confirmation emails send through, on that form''s own Mail Settings tab. There''s no single "default" provider per site — different forms on the same site may legitimately want different accounts (e.g. a client''s own account vs. the agency''s shared one). A form with no provider selected falls back to the install-wide Mailgun account set once in `.env`, same as the original single-account model.

This doc covers the provider system itself. See the **Sites & Multisite** doc for where it''s configured (Site Settings → Email Settings), and the **Form Designer** doc for how a form picks one.

## How It Works

### Data model (`core/src/models/email_provider.rs`)

- `email_providers` table (migration `0058_create_email_providers.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `provider_type TEXT` (`mailgun` | `smtp` | `sendgrid` | `postmark`), `label TEXT` (admin-chosen name), `config_encrypted TEXT`, `verified BOOLEAN`, `created_at`, `updated_at`.
- `forms.email_provider_id` (migration `0059_add_email_provider_to_forms.sql`): nullable FK → `email_providers`, `ON DELETE SET NULL` — deleting a provider a form was using silently reverts that form to the install-wide fallback rather than erroring or orphaning the form.
- Credentials vary by provider type, so rather than a wide sparse column set they''re serialized to one JSON blob (`ProviderConfig` enum, `#[serde(tag = "provider_type")]`) and encrypted as a single opaque string (`config_encrypted`) via the same `crypto::encrypt`/`decrypt` (AES-256-GCM, keyed off `SECRET_KEY`) the original per-site Mailgun key used.

### Sending (`core/src/mail.rs`)

- `send_via()` dispatches on the `ProviderConfig` variant: Mailgun and SendGrid/Postmark are plain `reqwest` HTTP calls (multipart for Mailgun, JSON for the other two); SMTP goes through `lettre`''s async transport (STARTTLS / implicit TLS / none, per the provider''s saved `tls_mode`).
- `resolve_provider()` — given a form''s `email_provider_id` (or `None`), loads and decrypts that `email_providers` row, or falls back to `AppConfig.mailgun_api_key`/`mailgun_domain` from `.env` when unset.
- `send_for_site()` is the existing entry point (used by Form Designer''s notify/confirm sends and by password recovery) — unchanged in shape, just now resolves through a provider instead of being hardcoded to Mailgun. Every send attempt is still recorded to `mail_log` regardless of which provider handled it.
- `send_test_email()` — a separate, `mail_log`-free path used only by the "Test" button in Email Settings; success marks the provider `verified = true`.

### Verification gate

Only providers with `verified = true` appear in a form''s "Send via" dropdown — a provider with typo''d or revoked credentials can''t accidentally be selected without ever having sent a real message through it. Editing a provider''s credentials resets `verified` back to `false` (the new values haven''t been proven to work yet), so it drops out of every form''s dropdown until re-tested — existing forms keep their `email_provider_id` pointing at it, they just silently fall back to the install-wide account until it''s re-verified.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /admin/sites/{id}/email-providers | `admin_email_providers::create` | Add a new provider |
| POST | /admin/sites/{id}/email-providers/{provider_id} | `admin_email_providers::update` | Full-overwrite update of a provider''s label/credentials; resets `verified` |
| POST | /admin/sites/{id}/email-providers/{provider_id}/test | `admin_email_providers::test` | Send a test email; marks `verified = true` on success |
| POST | /admin/sites/{id}/email-providers/{provider_id}/delete | `admin_email_providers::delete` | Delete a provider (forms using it fall back via `ON DELETE SET NULL`) |

## Database Schema

- `email_providers` (migration `0058_create_email_providers.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`), `provider_type TEXT`, `label TEXT`, `config_encrypted TEXT`, `verified BOOLEAN DEFAULT FALSE`, `created_at`, `updated_at`.
- `forms.email_provider_id` (migration `0059_add_email_provider_to_forms.sql`): nullable `UUID` FK → `email_providers(id) ON DELETE SET NULL`.

## Security Notes

- Every provider''s credentials are encrypted (AES-256-GCM) before being written to `config_encrypted`, keyed off `SECRET_KEY` — never stored or displayed in plaintext once saved, and never sent back to the browser (the Edit form is always blank, a full overwrite rather than a prefill).
- If `SECRET_KEY` is ever rotated, previously-saved provider credentials can no longer be decrypted — sends through that provider fail and are logged (not silently swallowed), and the form does **not** automatically fall back to the install-wide account the way an unresolvable provider row does; re-enter credentials via Edit after a rotation.
- The `POST .../test` endpoint sends to the requesting admin''s own account email, not an arbitrary address — it can''t be used to relay email to a third party.
', '2026-08-15 16:52:35.550796-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (55, 'form-designer', 'Form Designer', '# Form Designer

> Last updated: 2026-08-16 | Updated by: claude

## Overview

Form Designer lets a non-developer build a reusable form — an ordered list of fields plus a few behavior settings — in the admin panel, then insert it into any post or page from a picker in the content editor, instead of hand-writing `<form>` HTML per theme page. It''s the *definition* side of the forms system: `models::form_def` owns a form''s shape (fields/settings); the pre-existing `models::form_submission` (see the **Forms** doc) owns the data visitors actually send in. The two are linked by the form''s `slug` string (a submission''s `form_name` matches a `forms.slug`) **and**, since 2026-08-16, also by a real `form_submissions.form_id` foreign key populated at submit time — see **Database Schema** below and the **Forms** doc for why the slug link is still the authoritative one.

The nav item for this page is labeled **Form Builder** (renamed from "Forms" 2026-08-15, to avoid confusion with the separate **Forms**/Analytics nav item) and sits directly above Page Builder.

Field types cover the common cases plus two visual-only elements for structuring longer forms:

| Type | Renders as | Notes |
|------|-----------|-------|
| `text`, `email`, `number`, `phone`, `date` | `<input type="...">` | phone maps to `type="tel"`; email additionally gets a `pattern` attribute (added 2026-08-04) requiring a dot in the domain, since HTML5''s native `type="email"` validation alone accepts a domain-less address like `a@b` |
| `textarea` | `<textarea>` | |
| `select` | `<select>` | options declared as value/label pairs |
| `radio` | radio button group | options declared as value/label pairs |
| `checkbox` | single checkbox | |
| `toggle` | checkbox styled as a switch | options textarea holds exactly two lines — off state label/value, then on state |
| `separator` | `<hr>`, with an optional section title above it | visual only — no `name`, nothing submitted |
| `note` | a tinted callout box (info text/instructions) | visual only — no `name`, nothing submitted |

## How It Works

### Data model (`core/src/models/form_def.rs`)

- `forms` table (migration `0052_create_forms.sql`; `email_provider_id` added by `0059_add_email_provider_to_forms.sql`; `total_submissions` added by `0061_forms_total_submissions.sql`): `id, site_id, name, slug, fields JSONB, settings JSONB, email_provider_id UUID NULL, total_submissions BIGINT, created_at, updated_at`, `UNIQUE (site_id, slug)`.
- `FormField { label, name, field_type, required, options: Vec<(String, String)> }` — `options` is only meaningful for select/radio (arbitrary value/label pairs) and toggle (exactly the off/on pair).
- `FormSettings { success_message, button_label, include_honeypot, notify_email, confirm_submitter, confirm_subject, confirm_body, no_mail }` — deliberately has no button-color field; the submit button is styled from the active theme''s own CSS (`.themed-form button`/`.btn`), not chosen per-form. Which **provider** these emails send through is `email_provider_id`, a real column (not inside `settings`, so it can carry a real FK) — see **Mail Settings tab** below. `no_mail` (added 2026-08-16) is a hard override: when set, the form still saves submissions normally but `notify_email`/`confirm_submitter` are both skipped entirely at submit time, regardless of their own values.
- The slug is generated once from the name on creation (with `-2`, `-3`, ... suffixing on collision, same convention as post/page slugs) and is **immutable after creation** — both `form_submissions.form_name` and post/page embeds reference it, so renaming it later would silently orphan both.

### Editor (`/admin/form-designer`)

- List page: search + pagination (in-memory, 20/page — same pattern as `/admin/sites`, since a site''s form count is small). Each row''s name is plain text (no longer a link, as of 2026-08-16); the row action is a single **Edit** icon linking to the editor — the analytics-icon-on-the-list-row pattern was removed in favor of reaching per-form analytics from inside the editor itself (see below).
- Edit page (`/admin/form-designer/{id}`, shared with the create page at `/new`): a **Fields** card on the left, and a **Form Settings** card on the right with three tabs (**General Settings**, **Mail Settings**, **Preview**) — all-panels-in-DOM, JS-toggled, same `.page-tabs`/`.form-tab-panel` pattern used elsewhere in admin. An **Analytics** icon button (bar-chart-2) sits next to Save on the General Settings tab, linking to `/admin/analytics?tab=forms&form={id}` — the Forms tab pre-filtered to just this one form (see the **Forms** doc).
  - **General Settings**: form name, success message, button label, honeypot toggle. Its own **Save** icon-pill (also holds Analytics and Delete) submits the whole editor form.
  - **Mail Settings**: **Don''t send any email for this form** toggle (`no_mail`, greys out the rest of the tab when on) at the top; then **Send via** (the provider picker — see below), **Notify on new submission**, the **Email the submitter a confirmation** toggle, and — moved here from General Settings on 2026-08-16 — the confirmation email''s subject/body fields. Has its own **Save** icon-pill at the bottom of the tab, styled identically to General Settings''; both buttons submit the same underlying `<form>` (all tabs'' inputs are always present in the DOM, just hidden via `display:none` on the inactive panel), so either one saves everything, not just that tab''s fields.
  - **Preview**: a live, disabled-input mockup of the public form.
- Fields are edited entirely client-side and serialized to one hidden JSON field (`fields_json`) on submit — no per-field DB rows, matching the JSONB-column approach `form_submissions` already uses.
- Save stays disabled until something actually differs from what loaded (`snapshot()`/`checkDirty()` in the page''s script — compares a JSON snapshot of every field + setting, including `email_provider_id` and `no_mail`, against the one captured on load), mirroring the dirty-check pattern the theme customizer''s per-card Save buttons use.
- Separator/note rows skip the usual "both label and name are empty, discard this row" guard on submit — a titleless separator is a normal, common case, not an accidentally-added blank row.

### Inserting a form into a post or page

The Quill editor on `/admin/posts/*` and `/admin/pages/*` (both post types share one editor, `admin/src/pages/posts.rs`) registers a custom embed format:

- `FormEmbedBlot` (`admin/src/pages/posts.rs`) — a Quill `BlockEmbed`, `blotName: ''form-embed''`, serializes as the tag `<ss-form data-slug="..." data-label="...">` (mirrors the pre-existing `AudioBlot` pattern used for inline `<audio>` embeds). It renders as an inert placeholder chip in the editor (styled via CSS `::before` reading `data-label`, so the node itself stays empty) and is **not** editable text — an author can''t mistype it into invalid state.
- A toolbar button (clipboard icon, next to the audio button) opens a dropdown of the site''s saved forms — sourced from `PostEdit.saved_forms: Vec<(slug, name)>`, populated by `fetch_saved_forms()` in `core/src/handlers/admin/posts.rs` at every post/page editor entry point — and calls `quill.insertEmbed(range.index, ''form-embed'', {slug, label}, ''user'')` at the cursor.
- **The HTML sanitizer must allowlist the embed or it silently vanishes on save.** `core/src/models/post::sanitize_content()` runs on every save as defense-in-depth against unsafe HTML; `ss-form` and its `data-slug`/`data-label` attributes are explicitly added to ammonia''s allowlist (same mechanism already used for `audio`/`source`). This was a real bug hit during development — the embed round-tripped fine until save, which stripped the then-unlisted tag.

### Render-time expansion

- `form_def::expand_embeds(pool, site_id, content)` scans for `<ss-form data-slug="...">` and replaces each with the real rendered `<form>` for that definition (`FormDef::render_html()`). It''s a plain substring check (`content.contains("<ss-form")`) before touching the database, so posts/pages without an embed — the overwhelming majority — cost nothing extra.
- Called from `build_post_context()` in `core/src/handlers/home.rs`, which runs on every single post/page render (gated only on the post having a `site_id`, which every real post/page does) — so classic Tera-rendered posts and pages both pick up embeds automatically, with no theme-template changes required.
- `render_html()`''s markup deliberately matches the hand-written-form convention themes already used for contact/newsletter/subscribe pages (`.themed-form`, `.form-field`, `.form-required`, `.form-checkbox-label`, `.honeypot-field`, `.form-success`, and the theme''s generic `.btn`) instead of inventing a parallel class scheme — a theme that already styles those (Leisure, Symantic Signals) renders a fully-styled embedded form with zero new CSS. **The Default theme does not have this "shared form styles" CSS block yet** — a form inserted into a page using the Default theme currently renders unstyled; the CSS would need to be added there the same way it was to Leisure and Symantic Signals if that theme goes into real use.
- The honeypot field (when enabled) is a real, visually-hidden `<div class="honeypot-field">` + `<input name="_honeypot">`; the underlying `/form/{slug}` submit handler already strips any field name starting with `_` before storage (see the **Forms** doc), so no changes were needed there.
- Success message: `form::submit` redirects with `?submitted={slug}` (URL-encoded form slug, not a generic `?submitted=1`) so a page with more than one embedded form shows the right one''s success message. `render_html()` emits a small inline script per form that checks `location.search` for that exact slug and swaps the form for `.form-success` — automatic, unlike hand-written theme pages which do this server-side themselves via `{% if request.query.submitted %}`.

### Mail Settings tab: no-mail override, notify + confirm + provider picker (updated 2026-08-16)

Everything about whether/how a form emails anyone now lives on the **Mail Settings** tab, in this order:

1. **Don''t send any email for this form** (`no_mail`, added 2026-08-16) — a hard off switch. When on, the form behaves exactly as if it had no email settings at all: submissions still save to the database and are visible/exportable in the admin as normal, but `form::submit` skips both the notify and confirm sends entirely, even if `notify_email` is filled in or `confirm_submitter` is checked. It exists as an explicit, discoverable "definitely no mail" setting rather than relying on an admin remembering to blank out both fields separately.
2. **Send via** — which configured email provider (or the install-wide fallback) both sends below go through; see next paragraph.
3. **Notify on new submission** (`notify_email: Option<String>`) — when set, every submission triggers a background email with the submitted fields as plain-text `key: value` lines, subject `New submission: {form name}`.
4. **Email the submitter a confirmation** (`confirm_submitter: bool`) — auto-replies to whichever value the form''s first `email`-type field collected, using the `confirm_subject`/`confirm_body` templates (moved to this tab 2026-08-16 — previously on General Settings; `{{field_name}}` placeholders get filled from the submission).

Both sends go through **whichever email provider the form''s `email_provider_id` points at** (the **Send via** dropdown — lists "Install-wide default account" plus every *verified* provider configured on the site) — or the install-wide Mailgun fallback in `.env` if none is selected. See the **Email Providers** doc for the full provider system, and the **Sites & Multisite** doc for where providers are configured.

The send happens in the background, after the submission is already stored and the visitor is redirected — a slow or failed provider call never delays or blocks the actual form submission. A failure is logged server-side, not shown to the visitor.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/form-designer | form_designer::list | List forms for the site (search via `?search=`, `?page=`, `?partial=1` for the live-search AJAX fragment) |
| POST | /admin/form-designer | form_designer::create | Create a form |
| GET | /admin/form-designer/new | form_designer::new_form | New-form editor |
| GET | /admin/form-designer/{id} | form_designer::edit_form | Edit-form editor |
| POST | /admin/form-designer/{id} | form_designer::update | Save changes to a form (fields, settings — including `no_mail` — and `email_provider_id`) |
| POST | /admin/form-designer/{id}/delete | form_designer::delete | Delete a form definition (does not delete its submissions) |

## Database Schema

- `forms` (migration `0052_create_forms.sql`; `email_provider_id` added by `0059_add_email_provider_to_forms.sql`; `total_submissions` added by `0061_forms_total_submissions.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `name TEXT`, `slug TEXT`, `fields JSONB` (default `[]`), `settings JSONB` (default `{}`), `email_provider_id UUID NULL` (FK → `email_providers`, `ON DELETE SET NULL`), `total_submissions BIGINT NOT NULL DEFAULT 0`, `created_at`, `updated_at`. Unique on `(site_id, slug)`. `notify_email`/`confirm_submitter`/`confirm_subject`/`confirm_body`/`no_mail` live inside `settings` JSONB; `email_provider_id` and `total_submissions` are real columns — the former so it can carry a real foreign key, the latter so it can be atomically incremented.
- `total_submissions` is a **lifetime counter, incremented once per public submission and never decremented** (see `form_submission::create` in the **Forms** doc) — it deliberately diverges from the live submission count shown on the Submissions tab, so deleting old responses for cleanup doesn''t erase the historical record of how much a form was actually used. Surfaced on the Stats tab of `/admin/analytics/form/{id}`.

## Security Notes

- The embed tag and its two attributes are the only theme-injectable HTML explicitly allowlisted beyond ammonia''s defaults (alongside the pre-existing `audio`/`source`) — everything else in post content is still sanitized normally.
- Field definitions themselves are never rendered as Tera template source (per the project''s cardinal template-injection rule) — `render_html()` builds plain escaped HTML strings in Rust, never passes user-supplied field labels/options through the template engine.
- Deleting a form definition does not touch `form_submissions` — existing collected data for that form name is untouched and still visible/exportable (see the **Forms** doc for where), it just has no matching `forms` row anymore (`form_id` on those rows is set NULL via the FK''s `ON DELETE SET NULL`, and the slug-based lookup no longer resolves either).', '2026-08-16 15:18:54.640188-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (54, 'builder', 'Visual Page Builder', '# Visual Page Builder

> Last updated: 2026-07-22 | Updated by: claude

## Overview

The Visual Page Builder is a drag-and-drop page composer for site admins/theme managers. It pairs a React admin UI built on **Puck** (`@puckeditor/core`) with a Rust backend that stores page layouts as JSON and renders them to public HTML via Tera block templates. It lets a site owner assemble pages (homepage, regular pages, and special "post template" / "archive template" wrapper pages) from a fixed palette of blocks (Hero, Header, Footer, Columns, Posts, Categories, Tags, Search, Form, etc.) without writing any Tera or HTML themselves.

## How It Works

### Project / Page / Draft-vs-Live model

- A **`builder_project`** (`core/src/models/builder_project.rs`, table `builder_projects`) is a named collection of pages for a site. Exactly one project per site can be `is_active` at a time (enforced by a partial unique index) — that is the project the live site actually serves.
- A **`page_composition`** (`core/src/models/page_composition.rs`, table `page_compositions`) belongs to a project and has a `page_type`: `"homepage"`, `"page"` (regular, with a unique `slug` per project), `"post_template"` (wraps every post/page URL), or `"archive_template"` (wraps category/tag archive URLs). Only one of each special type is allowed per project (enforced in the create-page handler, not the DB).
- Each composition has **two JSON columns**: `composition` (live — what visitors see) and `draft_composition` (work in progress — what the Puck editor reads and writes). `save_composition` only updates the draft; `publish_composition` copies the draft into both columns atomically, promoting it to live. This lets admins edit freely without affecting the public site until they explicitly click Publish.
- A project can only be activated (`activate_project` handler) if `count_published` finds at least one page whose live `composition->''content''` array is non-empty — you can''t go live with zero published pages.
- Pages can be duplicated (`page_composition::duplicate`) — always as a new regular `"page"` type, copying the source''s `draft_composition` and deriving a new slug (`{slug}-copy` or a slugified name).

### Admin UI (`admin/src/pages/builder.rs`, `core/src/handlers/admin/builder.rs`)

Routes are grouped: `/admin/builder` (project list), `/admin/builder/{project_id}` (page list), `/admin/builder/{project_id}/pages/new` (new page form, with an optional "Copy layout from" existing page), and `/admin/builder/{project_id}/pages/{page_id}` (the editor). All admin routes require `admin.caps.can_manage_themes`. The editor page (`render_editor`) is a thin HTML shell that mounts the React app at `#root`, passing initial state through `window.__builderInit` (page id/name, project id, site id, project/site labels, and the site''s nav menus as JSON).

**Removed (2026-07-22): `/admin/builder2/{project_id}/pages/{page_id}` (`edit_page2`).** This near-duplicate "pure mode" route (hid the editor chrome via a `pureMode` flag) had no working entry point anywhere in the app — nothing linked to it — and the team doesn''t recall what it was originally meant for. Deleted along with the now-dead `pure_mode` parameter on `render_editor` (always `false` now).

The editor talks to a small JSON API:
- `GET /admin/builder/load/{id}` — returns the page''s `draft_composition`.
- `POST /admin/builder/save` — saves `data` (the Puck JSON) into the draft column via `save_composition`.
- `POST /admin/builder/publish` — promotes `data` to live via `publish_composition`; the frontend refuses to call this if `data.content` is empty.

### React editor (`admin/builder-ui/src/App.jsx`)

Wraps `@puckeditor/core`''s `<Puck>` component, configured with a `components` map from block name → React component (imported from `admin/builder-ui/src/blocks/`). On load it fetches the draft via `GET /admin/builder/load/:id`. Changes trigger `isDirty`/status-text state; an auto-save timer (`AUTO_SAVE_MS = 30_000`) calls `doSave` 30s after the last change, and a `beforeunload` handler warns on navigating away with unsaved changes. `handlePublish` posts to `/admin/builder/publish`. Header actions include a manual "Save Draft" button, a status indicator, and a link back to the site (skipped entirely in `pureMode`).

### Block architecture (React + Tera pairs)

Each block is a pair of files: a React component in `admin/builder-ui/src/blocks/` (defines the Puck field schema and the WYSIWYG render) and a matching Tera template in `themes/builder/blocks/` (renders the same block for real visitors). Current blocks: ArchivePosts, Button, Card/Cards, Categories, Columns, Div, Footer, Form, Header, Hero, Menu, Paragraph, PostContent, PostNavigation, Posts, Search, Tags, Text. `Sidebar.jsx` also exists in the blocks directory but is a planned block that was never finished — not imported/registered in `App.jsx`''s `config.components`, no matching Tera template. There''s a lot more Puck builder work planned generally; this is one item on that list, not a bug, and isn''t scheduled soon. Note also the naming mismatch: the Puck component key is `Cards` (mapped to `CardBlock` from `Card.jsx`) and its Tera template is `Cards.html`, while the source file itself is `Card.jsx`.

Per **CLAUDE.md**''s shared-layout convention, every top-level (section) block imports `PADDING_OPTIONS` and `MAX_WIDTH_OPTIONS` from `admin/builder-ui/src/blocks/ColorField.jsx` rather than defining local copies, so that setting e.g. "Standard (1200px)" produces the same max-width across different block types and content edges align down a page. `ColorField.jsx` also exports the `ColorField` component itself (a hex color swatch + `react-colorful` picker) used by blocks needing color pickers. Blocks dropped *inside* a zone (e.g. Text nested in Columns) don''t need padding/max-width fields — they inherit layout from their container.

### Public rendering (`core/src/templates/composer.rs`, `core/src/templates/loader.rs`)

`composer::render_composition(composition_json, templates, site_ctx)` deserializes the saved Puck JSON into `PuckData { content: Vec<PuckBlock>, zones: HashMap<String, Vec<PuckBlock>> }` (`zones` are keyed `"{block_id}:{zone_name}"` for blocks like Columns that accept nested blocks via Puck''s DropZone). For each top-level block it calls `render_block`, which:
1. Recursively pre-renders any of the block''s zones into a `zone_html` map (stripping the `"{block_id}:"` prefix from zone keys).
2. Clones the site context and inserts `block_config` (the block''s `props`), `block_id`, and `zone_html`.
3. Renders `{block_type}.html` via `templates.render_builder_block(...)`.

If a block''s template fails to render, the error is logged and an HTML comment placeholder is emitted instead of failing the whole page. Some blocks (currently `Hero`, `Posts`) contribute extra responsive CSS injected once per block type into a `<style>` tag in the page `<head>` (via `block_css`). An empty composition (`content` array empty) renders a minimal blank HTML shell.

`TemplateEngine::render_builder_block` (`core/src/templates/loader.rs`) lazily loads every `.html` file in `themes/builder/blocks/` into a dedicated `"__builder__"` Tera instance (cached under that key, separate from the per-site/per-theme instances used for normal theme rendering), keyed by filename so template names match `{block_type}.html` exactly.

### Where composed pages get served

- **Homepage**: `core/src/handlers/home.rs` — if `page_composition::get_homepage(site_id)` finds an active project''s homepage composition, it renders it via `composer::render_composition` instead of the theme''s `index.html`.
- **Post/page single view**: `core/src/handlers/post.rs` — if a `post_template` composition exists for the site''s active project, it wraps individual post/page URLs via the composer instead of `single.html`/`page.html`.
- **Category/tag archives**: `core/src/handlers/archive.rs` — if an `archive_template` composition exists, it wraps archive URLs via the composer instead of `archive.html`.

In all three cases, `enrich_builder_context` (`core/src/handlers/home.rs`) is called first to populate the Tera context with live DB data the blocks need: `builder_posts` (recent `PostContext` list), `builder_categories`/`builder_tags` (`TermContext` lists with post counts), and `builder_menus` (all nav menus for the site, for the Menu block to pick from).

## Database Schema

- **`builder_projects`** (migrations 0042, 0043): `id`, `site_id` (FK → sites, cascade delete), `name` (VARCHAR 35), `description` (VARCHAR 100, nullable), `is_active` (bool, partial-unique per site), `created_by` (FK → users, SET NULL), `created_at`/`updated_at`.
- **`page_compositions`** (migrations 0041, 0042, 0044, 0045, 0046→0047): `id`, `site_id` (FK, cascade), `project_id` (FK → builder_projects, cascade, added in 0042), `name`, `slug` (VARCHAR 100, nullable, unique per project when set, added in 0044), `page_type` (VARCHAR 20, default `''page''`, added in 0044 — replaces an earlier `is_homepage`-only model), `composition` (JSONB, live), `draft_composition` (JSONB, added in 0045 — seeded from `composition` on migration, so no in-progress work was lost), `is_homepage` (bool, partial-unique per site in 0041), `created_by`, `created_at`/`updated_at`. Migration 0046 added an `is_post_template` boolean flag which migration 0047 immediately dropped again in favor of the generalized `page_type` column.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/builder | `admin_builder::list` | Project list for the site |
| POST | /admin/builder/create | `admin_builder::create_project` | Create a project |
| POST | /admin/builder/deactivate | `admin_builder::deactivate_project` | Deactivate the site''s live project |
| POST | /admin/builder/save | `admin_builder::save` | JSON API: save draft composition |
| POST | /admin/builder/publish | `admin_builder::publish` | JSON API: promote draft to live |
| GET | /admin/builder/load/{id} | `admin_builder::load` | JSON API: load draft composition |
| GET | /admin/builder/{project_id} | `admin_builder::project_pages` | Page list within a project |
| POST | /admin/builder/{project_id}/rename | `admin_builder::rename_project` | Rename/redescribe project |
| POST | /admin/builder/{project_id}/activate | `admin_builder::activate_project` | Set project live (requires ≥1 published page) |
| POST | /admin/builder/{project_id}/delete | `admin_builder::delete_project` | Delete project (blocked while active) |
| GET/POST | /admin/builder/{project_id}/pages/new | `admin_builder::new_page_form` / `create_page` | New page form / create |
| GET | /admin/builder/{project_id}/pages/{page_id} | `admin_builder::edit_page` | Editor (normal chrome) |
| POST | /admin/builder/{project_id}/pages/{page_id}/set-homepage | `admin_builder::set_homepage` | Mark page as project homepage |
| POST | /admin/builder/{project_id}/pages/{page_id}/duplicate | `admin_builder::duplicate_page` | Duplicate page |
| POST | /admin/builder/{project_id}/pages/{page_id}/delete | `admin_builder::delete_page` | Delete page |

## Security Notes

- Every admin builder route requires `AdminUser` plus `admin.caps.can_manage_themes`; the JSON save/load/publish endpoints return `403 Forbidden` (not a redirect) on failure since they''re called via `fetch()`.
- Project/page ownership is re-verified against `site_id` on every mutating call (`builder_project::get_by_id(db, id, site_id)`), preventing cross-site access even with a guessed UUID.
- A project can''t be deleted while `is_active` (must deactivate first), and can''t be activated with zero published pages — both enforced server-side, not just in the UI.
- Composition JSON from the editor is stored as JSONB and re-rendered through Tera templates as `block_config` context variables (not as template source strings), consistent with the project''s structural-sandbox rule against template injection.

## Known Limitations / TODOs

- `Sidebar.jsx` is present in the block source tree but unregistered in `App.jsx` and has no Tera counterpart — a planned block not yet built. Part of a larger backlog of planned Puck builder work, not scheduled soon.
- Special page types (`homepage`, `post_template`, `archive_template`) are capped at one-per-project by application logic in the handlers, not by a DB constraint.', '2026-07-22 20:07:31.967981-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (1, 'admin', 'Admin Panel', '# Admin Panel

> Last updated: 2026-08-06 | Updated by: claude

## Overview

The admin panel (`/admin/*`) is the management interface for content, media, users, sites,
settings, menus, themes, and the visual page builder. Handlers live in
`core/src/handlers/admin/` (one file per area); shared page shell, capability struct, and
render helpers live in the `admin` crate (`admin/src/lib.rs`, `admin/src/pages/`). All routes
require an authenticated admin session via the `AdminUser` extractor
(`core/src/middleware/admin_auth.rs` — see the `middleware` doc for how it resolves the current
site and capabilities).

## How It Works

### Layout and capabilities

`admin::admin_page()` in `admin/src/lib.rs` wraps every admin page in a shared sidebar/header
shell built from `admin::PageContext` (constructed per-request by `page_ctx`/`page_ctx_full` in
`core/src/handlers/admin/mod.rs` from `AdminUser.caps`). The sidebar shows/hides nav items based
on capability flags (`can_manage_users`, `can_manage_sites`, `can_manage_plugins`,
`can_manage_settings`, `can_manage_content`, `can_manage_themes`, `can_manage_taxonomies`,
`can_manage_forms`, `can_manage_pages`) and renders live sidebar badges: unread form submissions
count, pending-review post count, and pending-review page count (all computed in
`page_ctx_full`, which also carries a "visiting" badge and site-switcher link when a super_admin
is impersonating a site). The **Plugins** nav item and all `/admin/plugins/*` routes remain
disabled pre-launch — `core/src/handlers/admin/plugins.rs` implements a full
install/upload/activate/deactivate/delete flow mirroring `themes.rs`, but it is not wired
into `router.rs` at all, so it is currently unreachable in production.

`admin::html_escape`, `admin::live_search_script` (a small vanilla-JS 300ms-debounce fetch
helper used by the Posts and Users list pages'' live search), and the shared media-picker
modal/iframe (`admin::media_picker_modal_html`, `admin::picker_page`) are also defined in
`admin/src/lib.rs`.

### Sidebar logo (added 2026-08-06)

The sidebar brand area (top-left corner, where `app_name` normally shows as text) can be replaced
with a custom logo image. This is **convention-based, not a DB setting or admin UI upload** — by
design, since it''s a rarely-changed branding asset (set once, maybe changed if the company
rebrands), not user content that needs a Media Library entry.

- Drop a file at `admin/static/branding/logo.svg` (or `.png`/`.jpg`/`.webp`, checked in that
  priority order via `app_state::detect_admin_logo`) and restart the app
  (`./app.sh restart`). Deleting the file and restarting reverts to the `app_name` text.
- Detected once at startup, cached as `AppState.logo_url` (`Option<String>`, the public
  `/admin/static/branding/logo.{ext}` URL or `None`). Threaded into `admin::PageContext.logo_url`
  by `page_ctx` (`core/src/handlers/admin/mod.rs`) and rendered by `admin_page()`
  (`admin/src/lib.rs`) as `<img class="brand-logo">` in place of the text when set. No hot-reload —
  changing the file requires a restart, same tradeoff as `PORT` and other `.env`-era startup-only
  config, which is an acceptable tradeoff given how infrequently this changes.
- **Sizing**: the available brand box is fixed at **180px wide** (sidebar is 220px via
  `--sidebar-w`, minus 20px horizontal padding on each side from `.brand`) and the `.brand-logo`
  CSS rule uses a **fixed (not max-) height of 38px** — `admin/style/admin.css`. Using a fixed
  height rather than `max-height` matters: without it, the browser doesn''t know the image''s
  rendered size until it decodes, so the box collapses to 0 height and then snaps down once the
  image loads — a visible layout shift on every admin page navigation (each admin page is a fresh
  server-rendered document, not a SPA, so this repeats on every click). A fixed height reserves
  the space immediately regardless of load timing.
- For a logo to **fill the width without overflowing**, design it close to the box''s aspect ratio,
  **180:38 ≈ 4.7:1** (a wide wordmark shape, not a square icon) — width is capped via
  `max-width: 100%` and scales down proportionally if the image is wider than that ratio, but a
  narrower/more-square image will hit the 38px height cap first and leave empty space on the
  sides rather than stretching to fill. SVG is preferred since it''s resolution-independent at any
  of these sizes; if using a raster format, export close to 2x the target box (e.g. ~360×76) for
  sharpness on high-DPI displays.
- `.brand`''s own vertical padding was reduced from `1rem` to `.6rem` top/bottom (horizontal
  padding unchanged at `1.25rem`, which is what defines the 180px width budget above) to keep the
  total header height close to what it was with text-only, since the 38px image is taller than a
  single line of `app_name` text at the sidebar''s default font size.

### Dashboard (`dashboard.rs`)

Renders role-scoped stats: editors/admins/super_admins see site-wide published/draft/pending
post counts, page count, user/subscriber counts (site-scoped for site staff, cross-site for a
true super_admin, owner-scoped when impersonating), and site counts. Authors additionally see
their own published/draft/pending counts plus two charts built from raw SQL aggregation queries
— posts-published-over-time and post-views-over-time (from the `post_views` table) — bucketed
by week/month/year via a `DashboardQuery` (`range`/`views_range`/`year`/`views_year`) query
param, each zero-filled for the selected bucket.

### Post/Page Editor (`posts.rs`)

- Quill rich-text editor feeding a hidden `content` input; publishing is blocked
  (`content_is_empty`) when the stripped HTML is blank.
- List views (`list`/`list_pages`) support pagination, a status filter, and live search
  (stop-word-stripped `ILIKE` on title) — a `partial` query param returns just the table-fragment
  HTML for the JS live-search to swap in, mirroring the Users page pattern.
- Author role is restricted: can only edit their own draft/pending posts, can only save as
  draft/pending (never publish directly), sees "pending review" badges scoped to their own posts.
- Pages support parent/child hierarchy (`parent_id`, excluding self from the parent dropdown via
  `fetch_parent_options`) and a template override dropdown populated by `scan_templates()`, which
  walks the active theme''s `templates/` directory (site-specific copy preferred over global) and
  excludes reserved templates (`base`, `page`, `index`, `single`, `archive`, `search`, `404`) and
  anything under `partials/`.
- Optional post/page password protection (Argon2 hash, never round-tripped in plaintext) and a
  per-post `comments_enabled` toggle.
- Post list link targets are bare `/{slug}` (no `/blog/` prefix) per the router''s URL
  unification.

### Categories & Tags (`taxonomy.rs`)

Shared `list_terms` helper renders both categories and tags (differentiated by `taxonomy` param);
create validates the slug format and surfaces duplicate-name/slug DB errors as a friendly flash.
Delete enforces site ownership for non-global-admins.

### Media Library (`media.rs`, `upload.rs`)

- Paginated grid (10/page) with mime-type filter (image/video/audio/document), folder filter,
  and per-type counts; authors see only their own uploads.
- Folders (`media_folders` table) support create/delete (delete offers "unassign" vs. "delete
  files too").
- `/admin/media` doubles as an iframe-embeddable browser/picker (`?browser=1` / `?picker=1`)
  used by the shared media-picker modal and the sidebar''s "Media" nav link.
- JSON API (`api_list`, `api_update_meta`, `api_update_folder`) backs the inline media UI; all
  three verify site ownership before returning/mutating data, and folder assignment additionally
  verifies the target folder belongs to the same site.
- Upload (`upload.rs`) slugifies the original filename, appends an 8-char UUID suffix, stores
  files under a per-site `uploads/{site_uuid}/` subdirectory, and reads image dimensions directly
  from the uploaded bytes via `imagesize` (no disk round-trip). Alt text/title/caption are
  sanitized via the shared `sanitize_media_text` (strips tags and `&"\``, caps at 35 chars).

### Nav Menus (`menus.rs`)

CRUD for `nav_menus`/`nav_menu_items` (see the `database` doc for schema). Item URLs are
validated (`clean_url`) to only allow relative paths, `http(s)://`, and `mailto:` — rejecting
`javascript:`/`data:` schemes. All menu/item mutations re-verify the menu belongs to the caller''s
site unless global admin.

### Comments (`comments.rs`)

A single `delete` handler — editors/admins/above may delete any comment; authors may not
(gated by `can_manage_content`, which is true for all content roles including author, so this is
enforced only at the route''s capability check, not per-author-ownership).

### Forms (`forms.rs`)

Lists distinct submitted form names with unread/submission counts, per-form submission view
(marks all as read on view), CSV export (RFC 4180 escaped, columns ordered
name/email/subject/message first then alphabetical), single/bulk delete, and a block/unblock
toggle backed by the `form_blocks` table.

### Users (`users.rs`) — largest handler in this group

- `/admin/users` list is split into two tabs — **Site Users** (staff, i.e. any non-subscriber
  role) and **Subscribers** — each independently paginated (`USERS_PER_PAGE = 20`) and
  live-searched (display name/username/email, case-insensitive substring; `partial=1` returns a
  `tbody` fragment fetched via `live_search_script`). A super_admin gets a site filter dropdown;
  when impersonating, the dropdown and default filter are scoped to the visited site''s owner
  rather than exposing the super_admin''s own sites.
- Create/edit forms enforce username format (lowercase alphanumeric + hyphen), password policy
  (`user::validate_password`), and hostname format when the "create a new site for this user"
  option is chosen. New-site creation seeds `sites/{uuid}/themes/`, `uploads/{uuid}/`, a hostname
  symlink under `uploads/`, and copies the global default theme — done in a `spawn_blocking` task
  so it doesn''t block the request.
- Role editing on `/admin/users/{id}/edit` is deliberately **read-only for role** — role changes
  now go exclusively through the dedicated Site Access UI (see below) "so it''s explicit about
  which site is affected."
- Delete has four guards, in order: no self-delete, cannot delete a protected account, only a
  global admin may delete another global admin, and the last global admin account can never be
  deleted (checked via `count_global_admins`).
- **Site Access** (`site_access_page`, `add_site_access`, `remove_site_access`): assigns/removes
  a user''s role on a specific site. Assigning `site_admin` when the target site already has a
  different non-super_admin owner triggers a `displaced_action` decision
  (`remove` / `demote_author` / `add_additional`) that the UI must resolve first (posting without
  one returns `?error=site_admin_exists`); demoting a site''s `admin` role away also clears
  `sites.owner_user_id` if the demoted user was the owner, keeping the owner column and
  `site_users.role` from silently disagreeing.

### Sites (`sites.rs`)

- List scoping: a true super_admin sees every site (with a "primary domain" badge derived from
  each owner''s `default_site_id`); an impersonating super_admin sees only sites owned by the
  visited site''s owner; other staff see only sites they hold a role on.
- New-site creation (`create`) validates hostname format, optionally creates a new Site Admin
  user or assigns an existing one, and (matching the users.rs flow) seeds the new site''s
  directories, hostname upload symlink, and default theme copy in a background
  `spawn_blocking` task.
- `switch`/`go_home` manage the `current_site_id` session key (site_admin can only switch to
  sites they have a role on); `go_home` restricts its `?next=` redirect target to paths starting
  with `/admin` to prevent open redirects.
- `delete` removes the DB row plus the site''s data directory
  (`sites/{uuid}/`), upload directory and hostname symlink, and plugin directory
  (`plugins/sites/{uuid}/` — DB rows there cascade automatically).
- `site_settings`/`save_site_config` edit per-site display settings (name, description,
  language, date format, posts-per-page) stored via `set_site_setting` into the generic
  `site_settings` table. **Maintenance mode and IP allow/deny-list settings are not exposed here
  or anywhere in the admin UI** — they are set directly in `site_settings` (keys documented in
  the `middleware` doc) via `synap site maintenance|allow-ip|block-ip`, not through any HTTP
  handler.
- `provision_ssl` appends a Caddy reverse-proxy block for the site''s hostname to the Caddyfile
  (idempotent — checks `caddy_block_exists` first) and shells out to `caddy reload` against the
  local Caddy admin API (no `sudo` needed/allowed, since `NoNewPrivileges` blocks it).

### Page Builder (`builder.rs`)

Full CRUD for builder projects (`builder_projects`) and their pages
(`page_compositions`) — project list/create/rename/activate/deactivate/delete, per-project page
list/create/duplicate/set-homepage/delete, and a JSON save/load/publish API that reads/writes
`draft_composition` (editor) vs. `composition` (live, only updated on Publish). A project can
only be activated once it has at least one *published* page. Two parallel page editor UIs are
served — `edit_page` (v1) and `edit_page2` (v2, newer) — both rendering the same
`admin::pages::builder::render_editor` shell with a `use_v2` flag. This is a large subsystem;
see the dedicated `builder` documentation slug for the composition JSON schema and zone
internals — this doc only tracks its presence in the admin routing surface.

### Themes (`themes.rs`, 1700+ lines)

Not one of this pass''s four target slugs, but shares the admin handler directory and the
`copy_dir_all` helper reused by `users.rs`, `sites.rs`, and `plugins.rs` for seeding
new-site/new-user theme copies. Themes resolve from three tiers — `themes/global/`,
`themes/private/` (super_admin only), and `sites/{uuid}/themes/` — with path-traversal guards
(canonicalize + `starts_with`) on every filesystem operation, and a global/private theme is
lazily copied into the site''s own folder the first time it''s activated so it appears in "My
Themes". Includes a file-in-theme editor with per-file `.bak` restore, and an uploaded-zip
installer with a required-file check (`REQUIRED_TEMPLATES`).

### Documentation Viewer (`documentation.rs`)

`/admin/documentation` — super_admin only — reads every row of the `documentation` table
(ordered `system` group before `feature` group, then title) and renders it via
`admin::pages::documentation::render_list`. This is the very table these four docs are stored
in.

### Settings & Profile (`settings.rs`, `profile.rs`)

`settings.rs` edits `app_settings` (installation-wide `app_name`/`timezone`) — gated by
`can_manage_settings`, which per `admin_auth.rs` is only true for a super_admin viewing their own
default/home site. `profile.rs` lets any admin user edit their own email/display
name/bio and change their own password (current-password re-verification + policy check).

## Routes / Endpoints

See the `routing` doc for the full `/admin/*` route table (all admin routes are defined in
`core/src/router.rs`).

## Security Notes

- All admin routes require an authenticated admin session (`AdminUser` extractor); unauthenticated
  requests redirect to `/admin/login`.
- `Cache-Control: no-store` applied to all `/admin/*` responses (`router.rs`).
- Every mutating handler that isn''t global-admin-only re-verifies the target row''s `site_id`
  matches `admin.site_id` (or ownership, for sites) before allowing the action — site isolation
  is enforced per-handler, not centrally.
- Filesystem operations (theme/plugin install, activate, file editor) canonicalize paths and
  check `starts_with` against the allowed root before touching disk, guarding against `..`
  traversal from user-supplied theme/plugin/file names.
- `/admin/plugins/*` handlers exist in source but are not registered in `router.rs` — the
  feature is code-complete but inert pre-launch.
', '2026-08-06 18:22:41.138161-04', 'claude', 'system');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (59, 'permalinks', 'Permalinks', '
# Permalinks

> Last updated: 2026-08-18 | Updated by: claude

## Overview

Per-site, WordPress-style configurable URL structure for **posts**. Lets a
site migrating off WordPress keep its old permalink structure (e.g.
`/%year%/%monthnum%/%postname%/`) so already-published and already-indexed
URLs keep resolving after the switch — the whole point of the feature.
**Pages are unaffected** — they always use their existing flat or
hierarchical slug path, regardless of this setting, matching WordPress''s own
behavior (Permalinks only ever governs post URLs there too).

## How It Works

### Configuration

Each site has a `permalink_structure` setting (default `/%postname%`,
identical to this app''s original bare-slug behavior — no site''s URLs change
until an admin opts in). Editable from **Site Settings → General → Permalinks**
(`admin/src/pages/sites.rs`), which offers WordPress''s familiar presets
(Post name, Month and name, Day and name, Category, Custom Structure) that
populate a single underlying text field, plus a live JS preview.

Supported tokens: `%postname%` (required, must be the final token — see
below), `%year%`, `%monthnum%`, `%day%`, `%hour%`, `%minute%`, `%second%`,
`%post_id%`, `%category%` (falls back to `uncategorized` if the post has no
category — WordPress''s own fallback).

Saving validates that the structure ends with `%postname%` or `%postname%/`
(`core/src/handlers/admin/sites.rs::save_site_config`) — rejected otherwise,
since request resolution depends on the postname always being the final
path segment.

### URL generation

`core/src/models/post::build_permalink(structure, post, category_slug)` does
the token substitution, using `published_at` (falling back to `created_at`
for an unpublished post, e.g. a preview link) for date tokens. This is the
single choke point `PostContext::build()` calls for non-page posts, so every
public-facing URL — templates, RSS feeds, archive listings, `sitemap.xml`,
and the theme API''s `url_for()`/`posts()` Tera functions — picks up the
site''s configured structure automatically. The admin posts list and post
editor''s "View" links (`core/src/handlers/admin/posts.rs`) were updated to
use the same function, so what an editor sees in `/admin/posts` matches what
a visitor actually gets.

### Request resolution (decorative segments)

`core/src/handlers/page.rs::single_page` is the fallback for any URL Axum
can''t otherwise match. When a multi-segment path doesn''t resolve as a page
hierarchy, `try_post_permalink()` retries it by its **last path segment**
as a post slug — deliberately without validating the earlier (date/category)
segments against the site''s configured structure. This is safe because
slugs are already unique per site (`posts_site_slug_unique`), so there''s
nothing to disambiguate.

- If the request matches the post''s current canonical URL exactly, it
  renders directly (full parity with `/{slug}` — password gate, unique-view
  tracking, comments — via the shared `post::render_single_post_response`).
- If it doesn''t match (a stale date, a renamed category, or the structure
  changed since the link was published), it issues a **301 redirect** to
  the true canonical URL — self-correcting instead of serving the same post
  at infinitely many path variations, which is bad for SEO.
- The original bare `/{slug}` route (`post_handler::single_post`) keeps
  working **unconditionally**, regardless of the configured structure. This
  is deliberate: an already-published/indexed link can never break just
  because a site later switches to a date- or category-prefixed structure.

### Known collision (edge case)

Fixed routes (`/category/{slug}`, `/tag/{slug}`, `/author/{username}`) are
registered ahead of the fallback. If a site uses a `%category%`-prefixed
structure and a real category happens to be named `category`, `tag`, or
`author`, a post permalink under that category would be shadowed by the
matching archive route instead of reaching `try_post_permalink`. Not solved
today — a real edge case, not a design flaw in the common case.

## Database Schema

No new table — stored as a row in the existing `site_settings` key/value
table: `(site_id, key = ''permalink_structure'', value = ''<structure string>'')`.
Loaded into `SiteSettings.permalink_structure`
(`core/src/app_state.rs`), same caching/reload pattern as every other
per-site setting.

## Security Notes

The 301 redirect target is always built server-side from the resolved
post''s own real data (`build_permalink`) — never echoes back attacker-
controlled path segments, so there''s no open-redirect surface here.

## Known Limitations / TODOs

- Secondary URL builders that predate this feature — breadcrumbs, the
  saved-posts account page, comment-submit and post-unlock redirects —
  still build flat `/{slug}` URLs. They still resolve correctly (via the
  bare-slug fallback), just don''t display in the site''s configured
  structure. Not yet updated for full display consistency.
- No admin UI or DB flag distinguishes "this site''s permalinks were changed
  recently" — no bulk redirect map or rewrite history; each old URL is
  resolved individually, on request, via the last-segment lookup above.
', '2026-08-18 22:50:57.74059-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (19, 'themes', 'Themes', '
# Themes

> Last updated: 2026-08-22 | Updated by: claude

## Overview

The theme system manages themes: creation, activation, editing, upload, download/copy, publish, and deletion. A theme is a directory of Tera templates (`templates/`) plus static assets (`static/`) and a `theme.toml` manifest. Themes exist in three tiers — **global** (shared library, visible to all sites), **private** (super_admin-only staging area, not visible to site admins), and **site-scoped** (`sites/{site_id}/themes/{name}/`, each site''s own editable copy). This handler is one of the largest in the codebase (~1,770 lines) and underpins both the classic Tera theme editor and, indirectly, the Puck visual page builder (which reads/writes into the same site theme directories via `themes/builder/blocks/`).

## Directory Layout (verified against the current repo, corrects prior assumptions)

- `themes/global/{name}/` — shared theme library available to every site.
- `themes/private/{name}/` — super_admin-only staging themes, not visible or activatable by site admins.
- `themes/global-backup/` — not referenced by any handler code; a manual, human-maintained backup location for custom themes the team has built (as opposed to the built-in defaults), since those are one-of-a-kind work product with no other copy anywhere — losing the working directory without this would mean rebuilding a custom theme from scratch. Intentionally manual, not automated; no code changes planned here.
- `themes/builder/blocks/*.html` — Puck visual builder block templates (Hero, Columns, Posts, Form, Menu, etc.), separate from the classic theme system.
- `sites/{site_id}/themes/{name}/` — **per-site copies live at repo-root `sites/`, not under `themes/sites/`.** Older documentation and the feature-map assumed `themes/sites/{site_id}/{theme_name}/`; the actual path is `sites/{site_id}/themes/{theme_name}/` (`state.config.sites_dir` joined with the site UUID, then `themes/{name}`). This is a real path used throughout `themes.rs` (activate, delete, get_theme, create_theme, editor, etc.) — any tooling or docs referencing the old path are wrong.

Every theme directory must contain the required templates listed in `REQUIRED_TEMPLATES`: `base.html`, `index.html`, `single.html`, `page.html`, `archive.html`, `search.html`, `404.html` (checked under `templates/`) — enforced only at zip-upload time via `extract_and_install_theme`.

## How It Works

### Activation flow (`activate`)

1. Rejects theme names containing `..`, `/`, or `\`.
2. Resolves which directory the theme lives in, in priority order: global → private (super_admin only) → the caller''s site-scoped directory.
3. Canonicalizes the resolved path and verifies it actually starts with one of the allowed parent directories (path traversal guard) before touching anything else.
4. **Global→site copy mechanism**: if the theme being activated comes from `global/` or `private/` and no site-scoped copy exists yet, `copy_dir_all` (recursive file copy, run on a blocking task) duplicates the entire theme directory into `sites/{site_id}/themes/{name}/`. This is what makes the theme show up under "My Themes" and become independently editable per site without touching the shared global copy. If a site copy already exists, the copy step is skipped (existing customizations are preserved).
5. Writes `active_theme = {name}` into `site_settings` via `set_site_setting`.
6. Calls `state.templates.switch_theme(&name)` to reload the Tera engine and updates the in-memory `state.active_theme` and the site cache (`update_site_theme_in_cache`) so static assets are immediately served from the new theme.

### Get Theme / Publish Theme

- `get_theme` — copies a theme from `global/` (or `private/`, super_admin only) into the caller''s site directory **without activating it**, for previewing/editing before switching. No-ops if a site copy already exists.
- `publish_theme` — super_admin only; copies a `private/{name}/` theme into `global/{name}/`, overwriting any existing global copy of the same name. This is how a staged private theme becomes available to all sites.

### Delete

Requires an explicit `source` field (`"site"`, `"global"`, or `"private"`) from the calling form rather than re-discovering the theme by filesystem search — the comment in the code explicitly calls out that falling back to auto-discovery would risk deleting the wrong directory if a site theme and a global theme share a name. Guards: only super_admin can delete global/private themes; the currently-active theme for the site cannot be deleted; a global theme cannot be deleted while any site has it set as `active_theme` (checked via `COUNT(*)` over `site_settings`); and the resolved theme path must be a direct child of the expected parent directory (traversal guard).

### Create Theme

`create_theme` seeds a brand-new theme by copying `themes/global/default/` as a starting point (guaranteeing all required templates exist), then overwrites `theme.toml` with the user-supplied name/description/author. Super admins choose `visibility` (`"public"` → `themes/global/`, otherwise `themes/private/`); site admins always land in their own `sites/{site_id}/themes/{name}/` regardless of the visibility field. Redirects straight into the file editor for the new theme.

### Upload (zip)

`upload_theme` accepts a multipart zip (size-capped by `state.config.max_upload_mb`, minimum 25MB floor), extracted on a blocking thread via `extract_and_install_theme`: detects a common top-level folder prefix inside the zip (`find_theme_prefix`, preferring a root-level `theme.toml` over a nested one), rejects any entry path containing `..` or a leading slash, extracts to a temp dir, validates `theme.toml` parses and has a safe `name`, checks all `REQUIRED_TEMPLATES` are present, then moves the temp dir into its final location (replacing any existing theme of the same name). Super admins upload into `themes/global/`; site admins upload into their own `sites/{site_id}/themes/`. After a successful upload, the currently active theme is reloaded in Tera (`switch_theme`) so the new files are recognized.

### Theme Editor (file management)

- `walk_theme_files` / `walk_dir_inner` recursively list files under a theme directory, allowlisting only `.html`, `.css`, `.js`, `.xml` extensions and skipping dotfiles/dot-directories (hides `.bak` backups, `theme.toml`, `screenshot.png`, `Zone.Identifier`, in-progress upload temp dirs, etc.).
- `resolve_theme_dir_by_source` is the single source of truth every editor handler (`edit_file`, `save_file`, `restore_file`, `delete_file`, `new_file`) uses to find the right copy of a theme, keyed by an explicit `source` query/form param (`"site"`, `"global"`, `"private"`) rather than a generic search — this avoids accidentally editing the wrong tier''s copy.
- `new_file` maps the chosen extension to a subdirectory and boilerplate content: `.html` → `templates/`, empty comment; `.css`/`.js` → `static/`, empty comment; `.xml` → `templates/`, XML declaration. Global/private themes are read-only for non-super_admins ("Global themes cannot be modified. Copy this theme to your site first.").
- `save_file` validates HTML files as real Tera syntax before writing: it builds a scratch `Tera` instance from the theme''s full `templates/**/*.html` glob (so `{% extends %}`/`{% include %}` resolve) and registers the *new* content under a throwaway name to catch parse errors before anything touches disk; on failure it redirects back with the (ANSI-stripped) Tera error message. If the file content is unchanged, no write/backup occurs. On the first real edit to a file, a `.bak` sibling is created if one doesn''t already exist. After saving an `.html` file, `state.templates.invalidate_theme(&theme, admin.site_id)` forces Tera to reload it from disk on next request.
- `restore_file` reverts a file from its `.bak` copy (mirrors the read-only/traversal guards of `save_file`).
- `delete_file` removes a theme file (not shown in full above, but follows the same source-resolution and read-only guard pattern as the other editor handlers).
- `resolve_file_in_theme` and `bak_path_for` provide traversal-safe path resolution and consistent `.bak` naming used across all editor operations.

### Theme discovery / listing

`scan_theme_dir` walks a themes parent directory, parses each subdirectory''s `theme.toml`, and builds a `ThemeInfo` (display name, version, description, author, whether a screenshot exists, source tier, active flag). `render_theme_list` aggregates global + private (if super_admin) + site-scoped scans, computes `can_delete`/`in_use_by`/`has_site_copy`/`has_global_copy` flags, filters by the `?filter=` query (`my` / `global` / `private`), and sorts active themes first, then alphabetically. Unit tests in the same file (`#[cfg(test)] mod tests`) cover global/site discovery, per-site isolation, and dot-directory exclusion.

### Screenshot serving

`GET /admin/theme-screenshot/{theme_name}` searches global → private (super_admin only) → site directory for a `screenshot.png`, applying the same canonicalize-and-check-`starts_with` traversal guard as everywhere else, and serves it with a 1-hour cache header.

## Theme Customizer

A theme opts into the customizer by setting `[customizer] enabled = true` in its `theme.toml`. When enabled, the theme editor landing page (`GET /admin/themes/editor/{theme}`) renders `render_customizer_landing` (`admin/src/pages/themes.rs`) instead of the raw file picker. Everything about it is manifest-driven — no field is hardcoded in Rust; it''s all declared under `[customizer.*]` in the theme''s `theme.toml`.

### Option types

| Type | TOML declaration | Stored | Rendered as | Read in templates as |
|------|-------------------|--------|-------------|-----------------------|
| Color | `[customizer.colors.{key}]` | Rewritten directly into the theme''s `static/css/style.css` `--{key}` variable — never the database | Color swatch input | Plain CSS variable; no Tera context needed |
| `bool` | `[customizer.options.{key}]` with `type = "bool"` | `theme_options` table (per site + theme) | Toggle-switch checkbox | `theme_options.{key}` — `{% if theme_options.some_key %}` |
| `order` | `type = "order"`, plus a nested `[customizer.options.{key}.items]` table of `item_key = "Label"` | `theme_options` table; value is a comma-joined key list | Drag-and-drop reorderable list | `theme_option_lists.{key}` — `{% for item in theme_option_lists.some_key %}` |
| `choice` | `type = "choice"`, plus a nested `[customizer.options.{key}.choices]` table | `theme_options` table | Radio button group | `theme_option_choices.{key}` — `{{ theme_option_choices.some_key }}` |
| `text` | `type = "text"` | `theme_options` table | Free-form text input (200 char max) | `theme_option_texts.{key}` — `{{ theme_option_texts.some_key }}` |
| `image` | `type = "image"`, plus `default_preview` for the theme''s built-in default image | `theme_options` table; value is a media library URL, empty string means "use the theme''s own default" | Image picker (opens the shared media library in `customizer_image` mode) | `theme_option_images.{key}` — `{{ theme_option_images.some_key }}` |

Every declared option also accepts:

- `label` — the human-readable field label shown in the admin UI.
- `default` — the resolved value used until a per-site override is stored.
- `group` — which customizer card (one `.card-boxed` per distinct group) the field renders inside; defaults to `"Layout Options"` if omitted. Cards are assembled by grouping every declared color/option by this string.
- `placement` — which column the field''s card renders in: `"main"` (default) or `"sidebar"`. All fields sharing a `group` should agree on `placement`; if they don''t, the first-seen value wins.

Card *position* within a column is currently just "first `group` name encountered" while parsing colors → bool options → order options → choices → texts → images in file order — there is no explicit numeric ordering key yet (deferred).

### Save / restore flow

- `POST /admin/themes/editor/{theme}/customizer-save` (`save_customizer`) handles every card through one route and one Save button. Colors are rewritten straight into the theme''s CSS file (a `.bak` backup is created on first edit, same convention as the classic file editor). Bool/order/choice/text/image values are upserted into the `theme_options` table, scoped to the current site + theme (`site_id, theme_name, option_key` — see `models::theme_options::save_option`/`save_order`/`save_choice`/`save_text`/`save_image`). A hidden `bool_option_keys` field lists which bool keys belong to *this* card''s submission — checkboxes only POST when checked, so without that list, saving one card would silently zero out every other card''s bool options too.
- `POST /admin/themes/editor/{theme}/customizer-reset` (`reset_options`) is each card''s "Restore original": deletes that card''s stored override rows from `theme_options` so the `resolve_*` helpers fall back to each option''s manifest `default`. Colors restore through the same `.bak`-based `restore_file` route the file editor uses, targeting `static/css/style.css`.
- Options are per-site: they only take effect once a site has activated its own copy of the theme. Editing a global/private theme directly (no `site_id` to store overrides against) still renders the customizer fields, but changes to non-color options don''t persist.

### Reading option values in front-end templates

`insert_theme_options` (`core/src/handlers/mod.rs`) runs on every front-end request and injects five context maps for the site''s active theme, resolving each site''s stored override over the manifest default:

- `theme_options` — `{key: bool}`
- `theme_option_lists` — `{key: [item_key, ...]}`
- `theme_option_choices` — `{key: string}`
- `theme_option_texts` — `{key: string}`
- `theme_option_images` — `{key: url}`

A theme that isn''t customizer-enabled (or declares none of a given type) just gets empty maps, never an error.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/themes | themes::list | Theme list (`?filter=my\|global\|private`) |
| POST | /admin/themes/activate | themes::activate | Activate a theme, copying global/private → site if needed |
| POST | /admin/themes/get-theme | themes::get_theme | Copy a global/private theme to the site without activating |
| POST | /admin/themes/publish-theme | themes::publish_theme | Publish a private theme to the global library (super_admin) |
| POST | /admin/themes/delete | themes::delete | Delete a theme (with active/in-use guards) |
| POST | /admin/themes/upload | themes::upload_theme | Upload a theme zip |
| GET | /admin/theme-screenshot/{theme_name} | themes::screenshot | Serve a theme''s screenshot.png |
| GET/POST | /admin/themes/create | themes::create_form / create_theme | New-theme form and creation |
| GET | /admin/themes/editor/{theme} | themes::edit_file | File editor / file picker |
| POST | /admin/themes/editor/{theme}/save | themes::save_file | Save file (validates Tera syntax, creates `.bak`) |
| POST | /admin/themes/editor/{theme}/restore | themes::restore_file | Restore file from `.bak` |
| POST | /admin/themes/editor/{theme}/new-file | themes::new_file | Create a new theme file |
| POST | /admin/themes/editor/{theme}/delete-file | themes::delete_file | Delete a theme file |
| POST | /admin/themes/editor/{theme}/customizer-save | themes_editor::save_customizer | Save one customizer card''s colors/options |
| POST | /admin/themes/editor/{theme}/customizer-reset | themes_editor::reset_options | Restore one customizer card''s options to their manifest defaults |

## Database Schema

- Active theme: no dedicated table — stored as a key/value row in the generic `site_settings` table (`key = ''active_theme''`, `value = {theme name}`), read/written via `set_site_setting` / direct `sqlx::query_scalar` lookups against `site_settings`.
- Customizer options: `theme_options` table, keyed by `(site_id, theme_name, option_key)` with a text `value` column and `updated_at`. Holds bool/order/choice/text/image overrides only — colors are never stored here (they live in the theme''s own CSS file).

## Static Asset Serving & Caching (`theme_static::serve`)

`GET /theme/static/{*path}` (separate from the admin management handler above — this is the front-end route every theme''s `base.html` links its CSS/JS/images through) resolves the requester''s active theme from the `Host` header via `state.resolve_site`, then serves the file from that theme''s `static/` directory. It responds with `Cache-Control: public, max-age=300, must-revalidate` (added 2026-08-21 for PageSpeed''s caching-policy score).

**Gotcha:** the URL path never encodes which theme is being served (`/theme/static/css/style.css` is identical regardless of active theme), so it is a single cache key shared by every theme. Switching a site''s active theme does not change the URL, meaning a browser that cached the previous theme''s CSS under that URL will keep serving it — stale, mismatched with the new theme''s HTML — for up to 5 minutes. Every theme''s `base.html` now appends `?theme={{ site.theme }}` to the stylesheet `<link>` specifically to bust this cache on a theme switch (`site.theme` is already available on `SiteContext`, see `core/src/templates/context.rs`). **Any new theme''s `base.html`, and any other static asset referenced by a fixed path across themes, must do the same** — otherwise re-adding a plain `href="/theme/static/..."` silently reintroduces the stale-cache bug. This applies independently to each of the three tiers a theme can live in (`themes/global/{name}/templates/base.html`, `themes/private/{name}/...`, and every site''s own copy at `sites/{site_id}/themes/{name}/templates/base.html`) — a site-scoped copy is a separate file and does not inherit a fix made only to the global template.

## Configuration

- `state.config.themes_dir` — base directory containing `global/`, `private/`, and `global-backup/`.
- `state.config.sites_dir` — base directory containing each site''s `{site_id}/themes/{name}/` copies.
- `state.config.max_upload_mb` — bounds theme zip upload size (25MB floor enforced regardless of configured value).

## Security Notes

- Theme/file names are rejected outright if they contain `..`, `/`, or `\` before any filesystem access.
- Every filesystem operation that resolves a theme or file path canonicalizes it and checks it `starts_with`/has-parent the expected directory, closing path traversal via symlinks or crafted names.
- Zip extraction rejects any entry whose relative path contains `..` or starts with `/`/`\`, and only finalizes installation after validating `theme.toml` and required templates.
- Global and private themes are read-only to non-super_admins in the editor; only super_admin may delete, publish, or fetch private themes.
- Deleting a theme is blocked if it is the active theme for the current site, or (for global themes) active on *any* site.
- Saved `.html` files are pre-validated as real Tera templates (using the full theme''s template set for `{% extends %}`/`{% include %}` resolution) before being written to disk, preventing a bad save from breaking live template rendering — the Tera invalidation cache is cleared only after a successful write.


', '2026-08-22 14:58:48.404555-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (60, 'wp-import', 'WordPress Import', '# WordPress Import

> Last updated: 2026-08-22 | Updated by: claude

## Overview

SynapCMS can import content from a WordPress WXR export (`Tools → Export → All content` in WP, an XML file). The importer lives on the **Import Content** tab of a site''s Settings page (`/admin/sites/{id}/settings?tab=import`), not the Media Library — it used to be a link inside the Media Library''s picker toolbar, but that trapped the compact media-browser iframe on a full-chrome page when clicked from inside it, so it was moved out to its own settings tab. Implementation: `core/src/handlers/admin/wp_import.rs`.

A single upload runs media import and content import together, in one pass:

1. **Attachments** are imported into the Media Library first (so featured images and in-content `<img>`s can be rewritten to the new URLs before posts are created).
2. **Authors** are matched or created (see below).
3. **Posts and Pages** are imported, with categories/tags, featured images, custom fields, and parent/child page relationships.

Every run writes a summary flash message, and a more detailed trace (including exactly which items were skipped and why) to the server log — see `./app.sh logs`.

**Re-imports update in place, not duplicate (added 2026-08-22):** `wp_import_post_map` records which Synap post each WXR item (`<wp:post_id>`) became (parallel to `wp_import_media_map` for attachments). `import_post` checks this table first: if the item was imported before for this site, the existing post is updated instead of a new one being created. This is what makes it safe to re-upload the *same* export a second time — the main use case being adding a media zip you didn''t have on the first pass, so images that failed to download over HTTP the first time get filled in. Specifics:
- The slug is never touched on update (it''s a public URL, not something a re-import should shift).
- `featured_image_id` is only ever *filled in*, never cleared or replaced — if a prior run already resolved it, a run where it doesn''t resolve (e.g. still no matching zip entry) leaves it alone rather than nulling it out.
- Title, content (re-rewritten against whatever media/URL map exists *this* run), excerpt, status, and published date are otherwise overwritten to match the export every time — so a manual edit made in the admin after import will be lost on a re-import. The Import Content tab''s UI copy calls this out.
- Categories/tags (`attach_to_post`) and custom fields (`set_meta`) were already idempotent (`ON CONFLICT DO NOTHING` / upsert), so re-running was always safe for those.

**Search indexing (added 2026-08-22):** imported posts/pages are created directly via the DB (`create_post_unique_slug`), bypassing the normal admin post handlers'' per-post `search::indexer::index_post` call on publish — so without this, imported content stayed unsearchable until the next restart or a manual reindex. `run_import` now calls `search::indexer::rebuild_index` once, at the end of Pass 3 (after parent_id linking, before the final progress write), the same full-index rebuild the admin UI''s "Rebuild Search Index" button and `synap search reindex` trigger — one batch commit covering every published post across every site, not scoped to just the imported ones (see the **Search** doc''s On-Demand Reindex section). A rebuild failure only logs a warning; it doesn''t fail the import or change the flash message.

## What''s Supported

- **Posts and Pages** — WP''s two built-in post types. Title, content, excerpt, slug (de-duplicated with a `-2`, `-3`, ... suffix on collision), status, publish date, comments-open/closed.
- **Status mapping**: `publish` → Published, `draft` → Draft, `pending` → Pending, `future` → Scheduled, `private` → Draft (Synap has no private-visibility gate, so this is the safer default rather than publishing something meant to be private). `trash`, `auto-draft`, and `inherit` (attachments'' own status) are skipped as not real content.
- **Media / attachments** — downloaded and added to the Media Library, organized into one `media_folder` per year/month (matching the upload date), same as WP''s own `/wp-content/uploads/YYYY/MM/` layout. Re-running an import (e.g. a newer export from the same site, or the same export with a media zip added) reuses already-imported media instead of re-downloading it, tracked in the `wp_import_media_map` table, and updates the existing posts instead of duplicating them, tracked in `wp_import_post_map` (see below).
- **Content URL rewriting** — `<img src>`/`<a href>` references to the old site''s media URLs inside imported post content are rewritten to the new Synap media URLs, including a fuzzy match that strips WP''s auto-generated size suffixes (`-300x200`) when the exact resized file wasn''t itself an attachment in the export.
- **Featured images** — resolved via the WP `_thumbnail_id` postmeta pointing at an imported attachment.
- **Categories and Tags** — WP''s two built-in taxonomies (`category`, `post_tag`); matched or created by slug. Any other custom `<category domain="...">` a plugin registered is skipped.
- **Custom fields** — every other postmeta key/value pair is copied verbatim onto the post''s custom fields, *except* WP''s own internal housekeeping keys (`_edit_lock`, `_edit_last`, `_wp_old_slug`, `_wp_old_date`, `_wp_desired_post_slug`, `_thumbnail_id`), which are dropped since they''re meaningless in Synap.
- **Page hierarchy** — WP `post_parent` is resolved to the new Synap page''s `parent_id` in a second pass, once every item in the export has been imported (so parent/child order in the file doesn''t matter).
- **Authors** — each WP author (`<wp:author>`) is matched to an existing Synap user by email. If no match exists, a new Synap account is created automatically: role `author`, granted `author` access on this site, `can_self_publish` off by default, with a randomly generated password. The generated username/password pairs are shown in the post-import flash message so the admin doing the import can hand them out — there''s no self-service password recovery for staff accounts yet, so that message is currently the only place to get them (see the **Users & Roles** doc). Authors with no email on file in the export (some minimal WXR files omit it) can''t be matched or created; their posts are assigned to whoever ran the import instead. If a matched author has no existing access to *this* site (e.g. they were matched by email but only ever had an account/role on a different Synap site — see "WP Multisite" below), they''re granted `author` access here too, but only if they hold no role on this site yet, so re-running an import never resets an existing `can_self_publish` grant back to off.

## What''s Not Supported

- **User passwords** — WXR never includes WP password hashes (a different hashing scheme anyway), so this can''t be imported under any circumstances. New accounts get a random Synap password instead (see above).
- **Custom post types** — anything other than `post`/`page` (e.g. WooCommerce products, a plugin-registered CPT) is skipped and counted in the flash message as "skipped (unsupported type)".
- **WP-internal block/site-editor data** — `nav_menu_item`, `wp_navigation`, `wp_global_styles`, `wp_template`, `wp_template_part`, `wp_block`, `wp_font_family`, `wp_font_face`, `custom_css`, `customize_changeset`, `oembed_cache`, `user_request`. These are WP''s own editor plumbing, never real content — they''re silently skipped without cluttering the flash message (still visible in the server log if you want the detail).
- **Comments** — individual `<wp:comment>` entries (the actual comment text/authors on a post) are not parsed or imported at all right now, only the post-level open/closed toggle.
- **SEO plugin metadata** — Yoast/RankMath/etc. fields land as raw custom-field text (via the generic postmeta copy above) but aren''t understood or surfaced by Synap''s own SEO features.
- **Shortcodes and Gutenberg block comments** — left as literal text inside imported content; Synap has no shortcode runtime, so `[gallery ids="1,2,3"]` or `<!-- wp:paragraph -->` markers render as-is rather than being interpreted.
- **Widgets** — not imported; WP''s sidebar/widget-area export isn''t part of WXR''s content items and has no Synap equivalent (theme layout, not content).
- **Forms** — plugin-based forms (Gravity Forms, CF7, etc.) aren''t part of WXR content and aren''t imported; use Synap''s own Form Designer to rebuild them.
- **Private-post visibility** — WP''s `private` status has no direct Synap equivalent (no password/visibility gate is set automatically); imported as Draft instead so nothing meant to be private goes live unreviewed.

See `docs/wordpress-migration-pain-points.md` in the repo for the fuller migration-planning writeup these gaps come from, including suggested working order for closing them.

## WP Multisite

WordPress multisite has no network-wide export — each subsite exports its own WXR file (`Tools -> Export` run from within that subsite). Migrating a multisite network means running this importer once per subsite, each into its corresponding Synap site; there''s no bulk/network-level import.

If the same person authors on more than one subsite, they''ll typically share one email address across those subsites'' exports. The importer takes advantage of that: an author matched by email is reused as the same Synap user across every site you import into, rather than creating a duplicate account per site (see above).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /admin/sites/{id}/import-wp | `wp_import::import` | Upload a WXR file and run the import (Import Content tab) |

## Security Notes

- Requires site-manager permission on the target site (`require_site_manager` — the same gate as other site-settings actions).
- The uploaded file is parsed in-memory as XML (`quick_xml`); it is never rendered as a Tera template, so it can''t reach the template-injection surface the plugin sandbox guards against.
- Attachment downloads are fetched server-side with a dedicated `SynapCMS-WP-Importer/1.0` user agent; a failed download just counts toward "media item(s) failed to import" rather than failing the whole run.



', '2026-08-22 16:29:27.181866-04', 'claude', 'feature');
INSERT INTO public.documentation (id, slug, title, content, last_updated, updated_by, grp) VALUES (61, 'deployment', 'Deployment & Build Toolchain', '# Deployment & Build Toolchain

> Last updated: 2026-08-26 | Updated by: claude

## Overview

VPS deploy binaries (`synapcms`, `synap`) are built against `x86_64-unknown-linux-musl` instead of the host''s native glibc target, producing a fully static binary with zero dynamic dependencies — see `scripts/install-vps.sh`''s `do_build()`. This avoids a binary built on a newer-glibc dev machine being unrunnable on an older-glibc VPS (glibc only guarantees forward compatibility). The tradeoff is that the local dev machine needs a musl C toolchain (`musl-gcc`) in addition to the `rustup` musl target, and that toolchain''s quirks can produce binaries that build cleanly but crash at runtime.

## Issue: segfault in `_start_c` on first run (AlmaLinux dev machine)

AlmaLinux 9 (and RHEL-family distros generally) have no `musl-tools`/`musl-gcc` package via dnf or EPEL — that package name is Debian/Ubuntu-only. To get `musl-gcc` there, musl libc 1.2.5 was built from source (`./configure --prefix=/usr/local/musl && make && make install`) and symlinked onto `PATH`.

Binaries linked with this from-source `musl-gcc` built and ran the version-print path locally in some quick checks, but segfaulted immediately (before printing anything) both locally and on the actual VPS (178.156.176.60) once actually invoked in the real install flow (`sudo -u www-data ./synap install ...` failed with `Segmentation fault`, exit 139).

**Root cause:** rustc''s musl target defaults to a PIE relocation model, which links in the self-relocating `rcrt1.o` crt startup object (from rustc''s own bundled musl sysroot at `~/.rustup/toolchains/.../lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/`). This object expects to run as a real PIE (`ET_DYN`) binary and self-relocate at load time using an auxv-provided base address. But the from-source `musl-gcc`''s generated specs file doesn''t pass `-pie`/`-static-pie` to the linker, so the actual output was a plain non-PIE `ET_EXEC` binary (`file` reported "statically linked", not "shared object"). The crt''s self-relocation code then ran against a binary layout it didn''t match, segfaulting in `_start_c` on the very first instructions before `main` is ever reached.

**Fix:** force rustc to use the plain (non-PIE) `crt1.o` instead, so the crt object and the actual link output agree. Added to `.cargo/config.toml` (checked into the repo root, applies to any machine building this project):

```toml
[target.x86_64-unknown-linux-musl]
linker = "musl-gcc"
rustflags = ["-C", "relocation-model=static"]
```

This is checked into the repo (not a per-machine `~/.cargo/config.toml`) specifically so both dev machines (this AlmaLinux box and a separate Arch Linux machine) build identically without needing to remember which one needed which workaround. `relocation-model=static` is the standard, most-compatible static-linking mode — safe on any musl toolchain, including a correctly-configured one (e.g. Arch''s `musl` package, which wasn''t actually hitting this bug) — so bringing it to a machine that didn''t need it causes no regression.

## Troubleshooting: musl-target binary crashes/segfaults immediately

1. Check the binary''s link type: `file target/x86_64-unknown-linux-musl/release/<bin>` should say "statically linked" (`ET_EXEC`), not "shared object".
2. Try running it directly: `./target/x86_64-unknown-linux-musl/release/<bin> --version`. A crash before any output at all (not a normal panic/error) points at crt/startup, not application code.
3. Confirm with a backtrace: `gdb -q --batch -ex run -ex bt --args ./<bin> --version`. A crash frame `#0` inside `_start_c` (rather than anywhere in Rust code) confirms the PIE/`ET_EXEC` crt mismatch described above.
4. Verify `.cargo/config.toml`''s `[target.x86_64-unknown-linux-musl]` `rustflags` is actually in effect — a closer `.cargo/config.toml` further down a directory tree, or a stray per-machine `~/.cargo/config.toml`, can override it (arrays like `rustflags` don''t merge across Cargo config files; the closest one to the build directory wins outright for that key).
5. Verbose-link if still unclear: `RUSTFLAGS="-C link-arg=-v" cargo build --release --target x86_64-unknown-linux-musl --bin <bin>` and inspect the `collect2`/`ld` invocation for which crt object (`crt1.o` vs `rcrt1.o`) actually got linked, and whether `-static`/`-pie`/`-static-pie` appear.

## Deployment target compatibility

Because the shipped binary is a fully static musl build, it has no libc/glibc-version dependency at all and will run on essentially any Linux distro on the same CPU architecture — Ubuntu, Debian, RHEL/AlmaLinux/Rocky, Fedora, Arch, even musl-native distros like Alpine.

Two things are **not** covered by that static-binary compatibility and would currently block a deploy regardless of distro flavor:

- **Architecture**: only `x86_64-unknown-linux-musl` is built. An ARM-based VPS (AWS Graviton, Oracle Cloud''s ARM tier, etc.) cannot run this binary — would need an `aarch64-unknown-linux-musl` cross-build added to the toolchain and `install-vps.sh`.
- **No systemd**: `scripts/install-vps.sh`''s requirements gate hard-requires `systemctl` on the remote host (checks `command -v systemctl`, sets up a `.service` unit). Non-systemd distros (Alpine/OpenRC, Void Linux) would fail that check before any build/install step runs, even though the binary itself would run fine there. The script also requires Caddy already installed and reachable, and PostgreSQL 13+ with passwordless sudo for the `postgres` and app-service users — provisioning steps, not distro-compatibility issues, but worth having ready before targeting a new host.
', '2026-08-26 18:36:34.533026-04', 'claude', 'system');


--
-- Name: documentation_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.documentation_id_seq', 61, true);


--
-- PostgreSQL database dump complete
--

