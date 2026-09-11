---
title: System Settings
group: feature
updated_by: codex
last_updated: 2026-09-10
---
# System Settings

> Last updated: 2026-09-10 | Updated by: codex

## Overview

System settings control global application behavior and are split into two stores: `app_settings` (global, non-site-scoped — app name, timezone, max upload size) and `site_settings` (per-site key/value pairs — site name, description, URL, language, active theme, posts-per-page, date format). The `/admin/settings` admin page has three tabs: General (app name + timezone, under one tab bundling both `general` and `localisation` sub-forms), Security (placeholder, no fields yet), and Advanced (max upload size, an on-demand Search Index rebuild button, plus Seed Users/Seed Posts/Clear Test Data dev-data tools). SMTP and most `AppConfig` fields are still not exposed in this UI (file/env-var + restart only). Access requires `can_manage_settings`.

## How It Works

### AppConfig (`core/src/config.rs`)

`AppConfig` is deserialized at startup via the `config` crate, loaded through `AppConfig::load()`: layer order is (1) serde field defaults, (2) `synaptic.toml` in the working directory (or the path in the `CONFIG_FILE` env var, file is optional), (3) environment variables (`config::Environment` with `__` separator, `.env` loaded via `dotenvy`) — later layers win.

Fields: `host` (default `0.0.0.0`), `port` (default `3000`), `database_url` (required), `secret_key` (default is an insecure placeholder — must be overridden in production), `themes_dir` (default `themes`), `plugins_dir` (default `plugins`), `uploads_dir` (default `uploads`), `sites_dir` (default `sites` — the base for each site's `{uuid}/themes/` and `{uuid}/uploads/` subdirectories), `dev_mode` (bool, default false), `log_level` (default `info`), `log_format` (default `text`), `ai_log_path` (default `logs/ai-translation.jsonl`), `search_index_path` (default `search-index`), `pid_file` (default `synaptic.pid`, used by `synap` for live reload), `caddyfile_path` (default `/etc/caddy/Caddyfile`, used for SSL provisioning from the admin panel), `metrics_token` (optional bearer token for `/metrics`), `max_upload_mb` (default `25` — since 2026-08-05 this is only a first-boot seed value for the DB-backed `app_settings.max_upload_mb`; once an admin saves a value on the Advanced tab, the DB value is authoritative and this field is no longer consulted for enforcement, only as a fallback), `admin_email` (optional, reply-to/notification address), and a full SMTP block (`smtp_host`, `smtp_port` default `587`, `smtp_username`, `smtp_password`, `smtp_from_name`, `smtp_from_email`, `smtp_encryption` default `starttls`) — outbound mail is disabled entirely if `smtp_host` is unset, and password-reset/form-notification code paths log a warning instead of sending. `bind_addr()` composes `host:port`. See the **Logging** document for operational commands and retention guidance.

### AppState (`core/src/app_state.rs`)

`AppState` is `Clone` (internally `Arc`-wrapped) and passed to every handler via the `State` extractor. Fields: `db: PgPool`, `templates: TemplateEngine`, `settings: Arc<SiteSettings>` (default/fallback), `config: Arc<AppConfig>`, `cookie_key` (HMAC signing key for post-unlock session cookies), `plugin_routes: Arc<HashMap<String, RouteRegistration>>`, `search_index: Arc<SearchIndex>`, `loaded_plugins: Arc<Vec<LoadedPlugin>>`, `active_theme: Arc<RwLock<String>>` (live-updated on theme switch), `site_cache: Arc<RwLock<HashMap<String, (Site, SiteSettings)>>>` (hostname-keyed), `metrics_handle: PrometheusHandle`, `metrics_token: Option<String>`, `app_settings: Arc<RwLock<AppSettings>>` (hot-reloadable), and `view_buffer: mpsc::UnboundedSender<(Uuid, String, NaiveDate)>` (a lock-free channel feeding a background task that batches post-view writes into `post_views` every 60s — deliberately not an `Arc<Mutex<HashSet>>` because a blocking std Mutex in async code starves the whole Tokio thread pool under load).

`SiteSettings` (per site): `site_name`, `site_description`, `base_url` (from key `site_url`), `language` (from `site_language`), `active_theme`, `posts_per_page` (i64, parsed with fallback), `date_format`. Loaded via `SiteSettings::load(pool, site_id)` (filters `site_settings WHERE site_id = $1`) or `SiteSettings::load_global(pool)` (filters `WHERE site_id IS NULL`, used at startup before any site is configured / for legacy pre-migration rows).

`AppSettings` (global): `app_name`, `timezone`, `max_upload_mb` — loaded from the un-scoped `app_settings` table.

Helper functions `set_app_setting(pool, key, value)` and `set_site_setting(pool, site_id, key, value)` upsert into their respective tables. `set_site_setting` uses `ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL` targeting a partial unique index, reflecting that `site_settings.site_id` can be `NULL` for legacy/global rows.

`AppState` methods: `resolve_site(hostname)`, `active_theme_for_site(site_id)` (falls back to the global in-memory `active_theme` if the site isn't cached), `site_hostname(site_id)`, `get_site_by_id(site_id)` (linear scan of the cache), `update_site_theme_in_cache(site_id, theme)` (called after a theme activation so static asset serving picks up the change without a restart), `reload_app_settings()` and `reload_site_cache()` (re-read from the DB into the in-memory caches).

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
- The actual upload size cap is enforced by a dynamic `core/src/middleware/upload_limit.rs` layer that re-checks the live `app_settings.max_upload_mb` value against each request's `Content-Length`, paired with a fixed 1GB `DefaultBodyLimit` as an absolute safety net against unbounded/chunked bodies. See the middleware doc for the full layering.

## Known Limitations / TODOs

- `save_settings` implements `general`, `localisation`, and `uploads`; any other `tab` value is accepted by the form but silently produces no change (falls through to the "re-render unchanged" branch). The Security tab has no fields yet — it's a placeholder for session timeout/login lockout/password policy config.
- Per-site maintenance mode and IP allow/block-list configuration (added in recent middleware work) are not part of this handler — they live in `core/src/middleware/maintenance.rs`, `core/src/middleware/ip_allowlist.rs`, and `core/src/middleware/ip_denylist.rs`, which are documented separately (middleware), not under this System Settings doc.
