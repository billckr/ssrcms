---
title: Routing
group: system
updated_by: codex
last_updated: 2026-09-10
---
# Routing

> Last updated: 2026-09-10 | Updated by: codex

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
fallback — nested password-protected pages aren't supported in MVP). The page fallback
(`page::single_page`) catches all unmatched paths, including nested page URLs like `/a/b/c`.

### Post and Page URL Unification

Posts and pages both live at `/{slug}` — no `/blog/` prefix. `post_handler::single_post`
resolves the slug and, if the record's `post_type` is `page`, delegates to the page fallback
handling. The fallback also directly handles nested page paths that never match `/{slug}`.

### Locale-Prefixed URLs (AI Translation, 2026-09-09)

`/{locale}/{slug}` (and nested paths) is not a separately registered route — it's always a 2+
segment path, so it can never match the direct `/{slug}` route and always lands in the
`page::single_page` fallback described above. There, before any of the existing segment-count
logic runs, a leading segment matching one of the site's enabled locales (`models::site_locale`)
is peeled off and threaded through separately, purely to select which translation to overlay
once the underlying post/page resolves — it never changes *which* post/page resolves. See the
**AI Post Translation** doc for the full routing/SEO details, including why a single content
segment under a locale prefix needs the same dual post-or-page handling `single_post` uses
(`render_page` alone only understands pages) and the accepted reserved-namespace trade-off of
enabling a locale whose code collides with a real top-level slug.

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
| POST | /account/email/change | `account_email::request_change` | Request a verified email change |
| GET/POST | /account/email/confirm/{token} | `account_email::confirm_form` / `confirm_post` | Confirm email change (unauthenticated) |
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
| POST | /admin/posts/{id}/translate | `posts::translate_post_action` | Translate missing/stale post or page prose |
| POST | /admin/posts/{id}/translate-embeds | `posts::translate_embeds_action` | Translate only missing/stale embedded forms/polls |
| POST | /admin/posts/{id}/translations/{locale}/delete | `posts::delete_translation` | Delete one post/page translation |
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
| GET | /admin/users/{id}/site-access | `users::site_access_page` | Manage a user's site access |
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
| GET | /admin/builder/{project_id} | `admin_builder::project_pages` | Project's pages list |
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
| GET/POST | /admin/form-designer | `admin_form_designer::list / create` | List or create reusable forms |
| GET | /admin/form-designer/new | `admin_form_designer::new_form` | New form editor |
| GET/POST | /admin/form-designer/{id} | `admin_form_designer::edit_form / update` | Edit a reusable form |
| POST | /admin/form-designer/{id}/translate | `admin_form_designer::translate` | Translate/refresh one form locale |
| POST | /admin/form-designer/{id}/translations/{locale}/delete | `admin_form_designer::delete_translation` | Delete one form translation |
| GET/POST | /admin/designer/polls | `admin_poll_designer::list / create` | List or create polls |
| GET/POST | /admin/designer/polls/{id} | `admin_poll_designer::edit_poll / update` | Edit a poll |
| POST | /admin/designer/polls/{id}/translate | `admin_poll_designer::translate` | Translate/refresh one poll locale |
| POST | /admin/designer/polls/{id}/translations/{locale}/delete | `admin_poll_designer::delete_translation` | Delete one poll translation |
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
- `/admin/*` and `/account/*` responses always get `Cache-Control: no-store`.
