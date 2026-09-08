---
title: Themes
group: feature
updated_by: claude
last_updated: 2026-08-22
---

# Themes

> Last updated: 2026-08-22 | Updated by: claude

## Overview

The theme system manages themes: creation, activation, editing, upload, download/copy, publish, and deletion. A theme is a directory of Tera templates (`templates/`) plus static assets (`static/`) and a `theme.toml` manifest. Themes exist in three tiers — **global** (shared library, visible to all sites), **private** (super_admin-only staging area, not visible to site admins), and **site-scoped** (`sites/{site_id}/themes/{name}/`, each site's own editable copy). This handler is one of the largest in the codebase (~1,770 lines) and underpins both the classic Tera theme editor and, indirectly, the Puck visual page builder (which reads/writes into the same site theme directories via `themes/builder/blocks/`).

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
2. Resolves which directory the theme lives in, in priority order: global → private (super_admin only) → the caller's site-scoped directory.
3. Canonicalizes the resolved path and verifies it actually starts with one of the allowed parent directories (path traversal guard) before touching anything else.
4. **Global→site copy mechanism**: if the theme being activated comes from `global/` or `private/` and no site-scoped copy exists yet, `copy_dir_all` (recursive file copy, run on a blocking task) duplicates the entire theme directory into `sites/{site_id}/themes/{name}/`. This is what makes the theme show up under "My Themes" and become independently editable per site without touching the shared global copy. If a site copy already exists, the copy step is skipped (existing customizations are preserved).
5. Writes `active_theme = {name}` into `site_settings` via `set_site_setting`.
6. Calls `state.templates.switch_theme(&name)` to reload the Tera engine and updates the in-memory `state.active_theme` and the site cache (`update_site_theme_in_cache`) so static assets are immediately served from the new theme.

### Get Theme / Publish Theme

- `get_theme` — copies a theme from `global/` (or `private/`, super_admin only) into the caller's site directory **without activating it**, for previewing/editing before switching. No-ops if a site copy already exists.
- `publish_theme` — super_admin only; copies a `private/{name}/` theme into `global/{name}/`, overwriting any existing global copy of the same name. This is how a staged private theme becomes available to all sites.

### Delete

Requires an explicit `source` field (`"site"`, `"global"`, or `"private"`) from the calling form rather than re-discovering the theme by filesystem search — the comment in the code explicitly calls out that falling back to auto-discovery would risk deleting the wrong directory if a site theme and a global theme share a name. Guards: only super_admin can delete global/private themes; the currently-active theme for the site cannot be deleted; a global theme cannot be deleted while any site has it set as `active_theme` (checked via `COUNT(*)` over `site_settings`); and the resolved theme path must be a direct child of the expected parent directory (traversal guard).

### Create Theme

`create_theme` seeds a brand-new theme by copying `themes/global/default/` as a starting point (guaranteeing all required templates exist), then overwrites `theme.toml` with the user-supplied name/description/author. Super admins choose `visibility` (`"public"` → `themes/global/`, otherwise `themes/private/`); site admins always land in their own `sites/{site_id}/themes/{name}/` regardless of the visibility field. Redirects straight into the file editor for the new theme.

### Upload (zip)

`upload_theme` accepts a multipart zip (size-capped by `state.config.max_upload_mb`, minimum 25MB floor), extracted on a blocking thread via `extract_and_install_theme`: detects a common top-level folder prefix inside the zip (`find_theme_prefix`, preferring a root-level `theme.toml` over a nested one), rejects any entry path containing `..` or a leading slash, extracts to a temp dir, validates `theme.toml` parses and has a safe `name`, checks all `REQUIRED_TEMPLATES` are present, then moves the temp dir into its final location (replacing any existing theme of the same name). Super admins upload into `themes/global/`; site admins upload into their own `sites/{site_id}/themes/`. After a successful upload, the currently active theme is reloaded in Tera (`switch_theme`) so the new files are recognized.

### Theme Editor (file management)

- `walk_theme_files` / `walk_dir_inner` recursively list files under a theme directory, allowlisting only `.html`, `.css`, `.js`, `.xml` extensions and skipping dotfiles/dot-directories (hides `.bak` backups, `theme.toml`, `screenshot.png`, `Zone.Identifier`, in-progress upload temp dirs, etc.).
- `resolve_theme_dir_by_source` is the single source of truth every editor handler (`edit_file`, `save_file`, `restore_file`, `delete_file`, `new_file`) uses to find the right copy of a theme, keyed by an explicit `source` query/form param (`"site"`, `"global"`, `"private"`) rather than a generic search — this avoids accidentally editing the wrong tier's copy.
- `new_file` maps the chosen extension to a subdirectory and boilerplate content: `.html` → `templates/`, empty comment; `.css`/`.js` → `static/`, empty comment; `.xml` → `templates/`, XML declaration. Global/private themes are read-only for non-super_admins ("Global themes cannot be modified. Copy this theme to your site first.").
- `save_file` validates HTML files as real Tera syntax before writing: it builds a scratch `Tera` instance from the theme's full `templates/**/*.html` glob (so `{% extends %}`/`{% include %}` resolve) and registers the *new* content under a throwaway name to catch parse errors before anything touches disk; on failure it redirects back with the (ANSI-stripped) Tera error message. If the file content is unchanged, no write/backup occurs. On the first real edit to a file, a `.bak` sibling is created if one doesn't already exist. After saving an `.html` file, `state.templates.invalidate_theme(&theme, admin.site_id)` forces Tera to reload it from disk on next request.
- `restore_file` reverts a file from its `.bak` copy (mirrors the read-only/traversal guards of `save_file`).
- `delete_file` removes a theme file (not shown in full above, but follows the same source-resolution and read-only guard pattern as the other editor handlers).
- `resolve_file_in_theme` and `bak_path_for` provide traversal-safe path resolution and consistent `.bak` naming used across all editor operations.

### Theme discovery / listing

`scan_theme_dir` walks a themes parent directory, parses each subdirectory's `theme.toml`, and builds a `ThemeInfo` (display name, version, description, author, whether a screenshot exists, source tier, active flag). `render_theme_list` aggregates global + private (if super_admin) + site-scoped scans, computes `can_delete`/`in_use_by`/`has_site_copy`/`has_global_copy` flags, filters by the `?filter=` query (`my` / `global` / `private`), and sorts active themes first, then alphabetically. Unit tests in the same file (`#[cfg(test)] mod tests`) cover global/site discovery, per-site isolation, and dot-directory exclusion.

### Screenshot serving

`GET /admin/theme-screenshot/{theme_name}` searches global → private (super_admin only) → site directory for a `screenshot.png`, applying the same canonicalize-and-check-`starts_with` traversal guard as everywhere else, and serves it with a 1-hour cache header.

## Theme Customizer

A theme opts into the customizer by setting `[customizer] enabled = true` in its `theme.toml`. When enabled, the theme editor landing page (`GET /admin/themes/editor/{theme}`) renders `render_customizer_landing` (`admin/src/pages/themes.rs`) instead of the raw file picker. Everything about it is manifest-driven — no field is hardcoded in Rust; it's all declared under `[customizer.*]` in the theme's `theme.toml`.

### Option types

| Type | TOML declaration | Stored | Rendered as | Read in templates as |
|------|-------------------|--------|-------------|-----------------------|
| Color | `[customizer.colors.{key}]` | Rewritten directly into the theme's `static/css/style.css` `--{key}` variable — never the database | Color swatch input | Plain CSS variable; no Tera context needed |
| `bool` | `[customizer.options.{key}]` with `type = "bool"` | `theme_options` table (per site + theme) | Toggle-switch checkbox | `theme_options.{key}` — `{% if theme_options.some_key %}` |
| `order` | `type = "order"`, plus a nested `[customizer.options.{key}.items]` table of `item_key = "Label"` | `theme_options` table; value is a comma-joined key list | Drag-and-drop reorderable list | `theme_option_lists.{key}` — `{% for item in theme_option_lists.some_key %}` |
| `choice` | `type = "choice"`, plus a nested `[customizer.options.{key}.choices]` table | `theme_options` table | Radio button group | `theme_option_choices.{key}` — `{{ theme_option_choices.some_key }}` |
| `text` | `type = "text"` | `theme_options` table | Free-form text input (200 char max) | `theme_option_texts.{key}` — `{{ theme_option_texts.some_key }}` |
| `image` | `type = "image"`, plus `default_preview` for the theme's built-in default image | `theme_options` table; value is a media library URL, empty string means "use the theme's own default" | Image picker (opens the shared media library in `customizer_image` mode) | `theme_option_images.{key}` — `{{ theme_option_images.some_key }}` |

Every declared option also accepts:

- `label` — the human-readable field label shown in the admin UI.
- `default` — the resolved value used until a per-site override is stored.
- `group` — which customizer card (one `.card-boxed` per distinct group) the field renders inside; defaults to `"Layout Options"` if omitted. Cards are assembled by grouping every declared color/option by this string.
- `placement` — which column the field's card renders in: `"main"` (default) or `"sidebar"`. All fields sharing a `group` should agree on `placement`; if they don't, the first-seen value wins.

Card *position* within a column is currently just "first `group` name encountered" while parsing colors → bool options → order options → choices → texts → images in file order — there is no explicit numeric ordering key yet (deferred).

### Save / restore flow

- `POST /admin/themes/editor/{theme}/customizer-save` (`save_customizer`) handles every card through one route and one Save button. Colors are rewritten straight into the theme's CSS file (a `.bak` backup is created on first edit, same convention as the classic file editor). Bool/order/choice/text/image values are upserted into the `theme_options` table, scoped to the current site + theme (`site_id, theme_name, option_key` — see `models::theme_options::save_option`/`save_order`/`save_choice`/`save_text`/`save_image`). A hidden `bool_option_keys` field lists which bool keys belong to *this* card's submission — checkboxes only POST when checked, so without that list, saving one card would silently zero out every other card's bool options too.
- `POST /admin/themes/editor/{theme}/customizer-reset` (`reset_options`) is each card's "Restore original": deletes that card's stored override rows from `theme_options` so the `resolve_*` helpers fall back to each option's manifest `default`. Colors restore through the same `.bak`-based `restore_file` route the file editor uses, targeting `static/css/style.css`.
- Options are per-site: they only take effect once a site has activated its own copy of the theme. Editing a global/private theme directly (no `site_id` to store overrides against) still renders the customizer fields, but changes to non-color options don't persist.

### Reading option values in front-end templates

`insert_theme_options` (`core/src/handlers/mod.rs`) runs on every front-end request and injects five context maps for the site's active theme, resolving each site's stored override over the manifest default:

- `theme_options` — `{key: bool}`
- `theme_option_lists` — `{key: [item_key, ...]}`
- `theme_option_choices` — `{key: string}`
- `theme_option_texts` — `{key: string}`
- `theme_option_images` — `{key: url}`

A theme that isn't customizer-enabled (or declares none of a given type) just gets empty maps, never an error.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/themes | themes::list | Theme list (`?filter=my\|global\|private`) |
| POST | /admin/themes/activate | themes::activate | Activate a theme, copying global/private → site if needed |
| POST | /admin/themes/get-theme | themes::get_theme | Copy a global/private theme to the site without activating |
| POST | /admin/themes/publish-theme | themes::publish_theme | Publish a private theme to the global library (super_admin) |
| POST | /admin/themes/delete | themes::delete | Delete a theme (with active/in-use guards) |
| POST | /admin/themes/upload | themes::upload_theme | Upload a theme zip |
| GET | /admin/theme-screenshot/{theme_name} | themes::screenshot | Serve a theme's screenshot.png |
| GET/POST | /admin/themes/create | themes::create_form / create_theme | New-theme form and creation |
| GET | /admin/themes/editor/{theme} | themes::edit_file | File editor / file picker |
| POST | /admin/themes/editor/{theme}/save | themes::save_file | Save file (validates Tera syntax, creates `.bak`) |
| POST | /admin/themes/editor/{theme}/restore | themes::restore_file | Restore file from `.bak` |
| POST | /admin/themes/editor/{theme}/new-file | themes::new_file | Create a new theme file |
| POST | /admin/themes/editor/{theme}/delete-file | themes::delete_file | Delete a theme file |
| POST | /admin/themes/editor/{theme}/customizer-save | themes_editor::save_customizer | Save one customizer card's colors/options |
| POST | /admin/themes/editor/{theme}/customizer-reset | themes_editor::reset_options | Restore one customizer card's options to their manifest defaults |

## Database Schema

- Active theme: no dedicated table — stored as a key/value row in the generic `site_settings` table (`key = 'active_theme'`, `value = {theme name}`), read/written via `set_site_setting` / direct `sqlx::query_scalar` lookups against `site_settings`.
- Customizer options: `theme_options` table, keyed by `(site_id, theme_name, option_key)` with a text `value` column and `updated_at`. Holds bool/order/choice/text/image overrides only — colors are never stored here (they live in the theme's own CSS file).

## Static Asset Serving & Caching (`theme_static::serve`)

`GET /theme/static/{*path}` (separate from the admin management handler above — this is the front-end route every theme's `base.html` links its CSS/JS/images through) resolves the requester's active theme from the `Host` header via `state.resolve_site`, then serves the file from that theme's `static/` directory. It responds with `Cache-Control: public, max-age=300, must-revalidate` (added 2026-08-21 for PageSpeed's caching-policy score).

**Gotcha:** the URL path never encodes which theme is being served (`/theme/static/css/style.css` is identical regardless of active theme), so it is a single cache key shared by every theme. Switching a site's active theme does not change the URL, meaning a browser that cached the previous theme's CSS under that URL will keep serving it — stale, mismatched with the new theme's HTML — for up to 5 minutes. Every theme's `base.html` now appends `?theme={{ site.theme }}` to the stylesheet `<link>` specifically to bust this cache on a theme switch (`site.theme` is already available on `SiteContext`, see `core/src/templates/context.rs`). **Any new theme's `base.html`, and any other static asset referenced by a fixed path across themes, must do the same** — otherwise re-adding a plain `href="/theme/static/..."` silently reintroduces the stale-cache bug. This applies independently to each of the three tiers a theme can live in (`themes/global/{name}/templates/base.html`, `themes/private/{name}/...`, and every site's own copy at `sites/{site_id}/themes/{name}/templates/base.html`) — a site-scoped copy is a separate file and does not inherit a fix made only to the global template.

## Configuration

- `state.config.themes_dir` — base directory containing `global/`, `private/`, and `global-backup/`.
- `state.config.sites_dir` — base directory containing each site's `{site_id}/themes/{name}/` copies.
- `state.config.max_upload_mb` — bounds theme zip upload size (25MB floor enforced regardless of configured value).

## Security Notes

- Theme/file names are rejected outright if they contain `..`, `/`, or `\` before any filesystem access.
- Every filesystem operation that resolves a theme or file path canonicalizes it and checks it `starts_with`/has-parent the expected directory, closing path traversal via symlinks or crafted names.
- Zip extraction rejects any entry whose relative path contains `..` or starts with `/`/`\`, and only finalizes installation after validating `theme.toml` and required templates.
- Global and private themes are read-only to non-super_admins in the editor; only super_admin may delete, publish, or fetch private themes.
- Deleting a theme is blocked if it is the active theme for the current site, or (for global themes) active on *any* site.
- Saved `.html` files are pre-validated as real Tera templates (using the full theme's template set for `{% extends %}`/`{% include %}` resolution) before being written to disk, preventing a bad save from breaking live template rendering — the Tera invalidation cache is cleared only after a successful write.



