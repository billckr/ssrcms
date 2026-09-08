---
title: Plugins
group: feature
updated_by: claude
last_updated: 2026-07-23
---
# Plugins

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Plugins are Tera template collections loaded from the filesystem at startup. They extend the CMS by registering template partials against named hook points (e.g. `head_end`, `after_content`). Plugins are installed per-site and tracked in the `site_plugins` table, and can register custom meta fields and HTTP routes via their manifest. The plugin system is designed for zero-compilation: plugin authors write Jinja2-style Tera templates, not compiled Rust.

**Paused indefinitely (confirmed 2026-07-22, not a pre-launch/post-launch timeline).** The admin plugin-management UI routes are commented out of the router — the handler code in `core/src/handlers/admin/plugins.rs` is fully implemented but not reachable via HTTP in the current build. This isn't a launch-sequencing decision; it's a genuine architectural problem the app hasn't solved: because the core app is a compiled Rust binary, letting third parties add their own plugins is a real challenge (there's no safe, dynamic extension mechanism the way an interpreted-language CMS has), and even the Tera-template plugins that do exist have very limited access to core app functionality precisely because of that compiled boundary. Revisiting this is possible someday but is a long way off and isn't on any current roadmap.

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
- `upload` — accepts a multipart zip (size-limited by `state.config.max_upload_mb`, minimum enforced floor of 25MB), extracts to a temp directory under the site's plugin folder, validates `plugin.toml` exists and is well-formed TOML with a safe `name`, validates `plugin_type` (`"tera"` always OK, `"wasm"` requires an included `.wasm` file, anything else rejected), moves the temp dir to its final location (replacing any existing plugin of the same name), registers the plugin's templates into the shared Tera engine (`register_plugin_templates` — globs `*.html`/`*.xml`, since the `glob` crate doesn't support brace expansion), and records the install in the DB.
- `activate` / `deactivate` — flip the `active` flag in `site_plugins`.
- `delete` — refuses to delete an active plugin ("Deactivate it first"), guards against path traversal by checking the canonicalized plugin path's parent equals the site's plugin directory, removes the directory recursively and deletes the DB record.

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

Unrelated to admin management, `core/src/handlers/plugin_route.rs` dispatches plugin-registered public routes (declared under a manifest's `[routes]` section) at runtime — `state.plugin_routes` is built from all loaded plugins' route registrations and each path is registered individually in the router (`plugin_route::dispatch`), plus a dedicated `/sitemap.xml` route (`plugin_route::sitemap`). These public dispatch routes are unaffected by the admin-UI being disabled.

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
- WASM plugin support (`plugin_type = "wasm"`) is validated for presence of a `.wasm` file at upload time but the loader (`core/src/plugins/loader.rs`) only loads Tera templates — there is no WASM execution path. Same status as the rest of the plugin system: paused, no timeline.
