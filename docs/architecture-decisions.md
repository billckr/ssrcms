# Architecture Decisions

Decisions made during development, along with the reasoning behind them.
Update this file when a significant design choice is made or revised.

---

## Multi-Tenancy Model

**Decision:** Every site — including the agency's own site — is a plain row in the `sites`
table. There is no structurally special "primary" site.

**Rationale:** Keeps the data model uniform. The agency may run their own public site, run
only client sites, or both. The system makes no assumption about which use case applies.

**Role distinction:** The `super_admin` role is what gives the agency operator elevated
access (cross-site visibility, system settings, user management). That privilege lives on
the user, not on a special site row.

---

## Outbound Mail: Config File, Not Database

**Decision:** All SMTP configuration lives in `.env` / `synaptic.toml`, not in the database.
There is no Email settings tab in the admin UI.

**Fields:** `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM_NAME`,
`SMTP_FROM_EMAIL`, `SMTP_ENCRYPTION` (starttls / tls / none). All optional — if `SMTP_HOST`
is not set, outbound mail is disabled and operations that require email log a warning.

**Rationale:** Follows the same model as WordPress — WP ships no mail UI and expects you to
use the server's mail agent or a third-party SMTP plugin/service. SMTP credentials in the
database create unnecessary risk (SQL injection, backup leakage, query logs). Credentials in
environment variables are outside the app's data layer entirely and are gitignored by
default. Most agencies have a preferred provider (Mailgun, Postmark, SendGrid, SES) with
their own dashboards — the CMS has no value to add there.

**Admin email:** `ADMIN_EMAIL` in `.env` / `synaptic.toml` — used as the reply-to /
notification address for system emails. Same rationale as SMTP: rarely changes, no DB
needed. The `admin_email` key that was seeded in `site_settings` by migration 0006 is
dead weight — never populated, never read — and can be ignored.

**Install wizard:** The installer prompts for two separate email addresses:
- **Admin login email** — stored in `users.email`; used to log in to the admin panel
- **System notification email** — written as `ADMIN_EMAIL` in `.env`; used as the reply-to
  address for outbound system emails. Defaults to the login email but can be any address
  (e.g. `info@acme.com`). Changing it later requires editing `.env` and restarting (no rebuild).

**Status:** Implemented. Migration 0022 live. SMTP fields and `admin_email` added to `AppConfig`.
Email tab removed from `/admin/settings`. Installer writes `ADMIN_EMAIL` to `.env`.

---

## Settings: App-Level vs. Per-Site — Decided

**Decision:** Three-layer separation:

| Layer | Storage | Examples |
|-------|---------|---------|
| Infrastructure | `.env` / `synaptic.toml` | DATABASE_URL, SMTP, SECRET_KEY, ports |
| App-wide runtime | `app_settings` table (new) | app_name, maintenance_mode, default theme for new sites |
| Per-site | `site_settings` table (existing) | site_name, active_theme, posts_per_page, date_format |

**The `app_settings` table** is a simple key-value store with no `site_id`. It holds
settings that affect the whole installation — not any one site. Only `super_admin` can
edit these, via `/admin/settings`.

**Why not reuse `site_settings` with `site_id IS NULL`?**
That was the legacy approach and it created ambiguity — you couldn't tell whether a NULL
`site_id` row was an intentional app-level setting or a site setting that lost its reference.
A dedicated table is unambiguous and isolates app data from site data structurally.

**The admin app name (`app_name`)** is the first concrete use case. The top-left "Synaptic"
label in the admin UI should reflect what the agency calls their CMS installation — not a
site's `site_name` (which is public-facing content). These are distinct concepts:
- `app_settings.app_name` → "Acme CMS" (admin chrome, agency brand)
- `site_settings.site_name` → "Beth's Bakery" (public site, theme templates)

**Hot-reload:** `app_name` and other app settings are cached in `AppState` behind an
`Arc<RwLock<>>`. Saving via the UI invalidates the cache without a restart — same pattern
as `active_theme`.

**Status:** Implemented. Migration 0022 (`app_settings` table) applied. `AppSettings` struct
cached in `AppState` behind `Arc<RwLock<>>`. Hot-reload confirmed working — saving via UI
updates the sidebar brand label without a restart. `page_ctx()` reads `app_name` from state
on every request.

### General Settings Tab — Field Breakdown

| Field | Tab/Page | Storage | Notes |
|-------|----------|---------|-------|
| App Name | `/admin/settings` General | `app_settings` | Admin chrome brand label — not public-facing |
| Admin Email | `/admin/settings` General (read-only) | `AppConfig` (env) | Set via `ADMIN_EMAIL` in `.env` |
| Timezone | `/admin/settings` General | `app_settings` | App-wide — one timezone per installation |
| Date Format | `/admin/sites/{id}/settings` | `site_settings` | Per-site — already live and working |
| Posts Per Page | `/admin/sites/{id}/settings` | `site_settings` | Per-site — already live and working |

**Timezone** is app-level, not per-site. It is used for admin activity timestamps, scheduled
publishing, and form submission records. Running different timezones per site on the same
server is not supported — one installation, one timezone, set by the agency.

**Date Format and Posts Per Page** have no meaning at the app level. They are purely
per-site content settings and belong only in the per-site settings page. Removed from
the General tab in `/admin/settings`.

### Security Tab — Deferred

Password complexity rules are currently hardcoded in the auth handler (min/max length,
mixed case, numbers, symbols). Moving these to `app_settings` is useful for agencies that
want to adjust policy for their clients but is not urgent. Session timeout and login lockout
do not exist yet. The Security tab will be wired up when auth gets a dedicated pass.

### Advanced Tab — Upload Size

| Field | Storage | Default | Notes |
|-------|---------|---------|-------|
| Max Upload Size | `AppConfig` (env) | 25 MB | Read-only in UI — requires restart to change |

**Revised decision:** Upload size lives in `.env` / `synaptic.toml` as `MAX_UPLOAD_MB`, not
in `app_settings`. Rationale: the original case for `app_settings` (no restart) was valid,
but the operational reality is that agencies almost never need to change the upload limit
mid-session, and keeping it in config alongside other infrastructure values (ports, DB URL)
is simpler and more consistent. The Advanced tab shows the current value as a read-only
field with a note to edit `.env` and restart.

The Axum `DefaultBodyLimit` layer is applied per-route at startup, and the in-handler
size check in `appearance.rs` also reads from `AppConfig`. Both must agree — both use
`state.config.max_upload_mb`. Default of 25 MB written to `.env` by the install wizard.

All other Advanced settings (maintenance mode, debug logging, template cache TTL) are
deferred until the underlying features exist.

---

## Plugin System: Tera Templates, Not Compiled Code

**Decision:** Plugins and themes are Tera templates loaded at runtime from a watched
directory. No compilation step for plugin authors.

**Rationale:** Mirrors WordPress's "drop files in a folder, it works" model. Low barrier —
any Jinja2/Twig/Django/Liquid developer can write plugins. WASM plugin layer is the
long-term goal but deferred post-MVP.

**Security constraint (cardinal rule):** User-supplied content must always enter templates
as context variables, never as template source strings. Rendering user content as a Tera
template string is a template injection vulnerability.

---

## Performance-Critical Hooks Live in Rust, Not Templates

**Decision:** Hooks that fire on every request (middleware-style, request filtering, response
modification) must be compiled Rust, not Tera templates.

**Rationale:** Template rendering is for the presentation layer only. Putting hot-path logic
in interpreted templates would add latency on every request with no escape hatch.

---

## Search Index: Single Commit for Bulk Rebuilds

**Decision:** On startup, the Tantivy search index is rebuilt in a single batch commit
rather than one commit per document.

**Rationale:** Discovered in production testing — with 1,000+ posts, committing after every
`upsert()` caused 1,000+ sequential disk flushes, delaying the server becoming responsive
by ~3 minutes. The fix (`rebuild_all()`) loads all documents into the writer buffer and
commits once. Single-document writes from admin handlers still commit immediately, which is
correct for keeping the index fresh after edits.

---

## Admin UI: SSR-Only for Now

**Decision:** The admin UI is server-side rendered Rust (no WASM, no JS framework).
Migration to Leptos/WASM is a planned future step.

**Rationale:** Gets a working admin UI shipped faster. The full Leptos/WASM admin is the
long-term goal (single language across backend, frontend, and plugins) but is deferred until
the content model and API surface are stable.
