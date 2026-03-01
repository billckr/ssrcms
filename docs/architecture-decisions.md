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

**Status:** Decided. SMTP fields added to `AppConfig`. Email tab removed from
`/admin/settings`.

---

## Settings: Agency-Level vs. Per-Site (OPEN — under discussion)

**Context:** The `site_settings` table is a key-value store scoped by `site_id`. Currently
every site, including the agency's own domain, stores its settings there on equal footing.

**The concern:** Infrastructure-level settings (SMTP credentials, session policy, upload
limits, maintenance mode) affect the whole application, not just one site. Storing them
alongside per-site content settings in the same table creates a single point of failure and
blurs the line between "app config" and "site config".

**Options being considered:**

1. **Separate `app_settings` table** — Agency/system-wide settings get their own table with
   no `site_id`. Per-site settings stay in `site_settings`. Clean separation: things that
   affect the whole app vs. things that are per-site.

2. **Config file for infrastructure settings** — SMTP credentials, session timeouts, max
   upload size live in `app.toml` / `.env` alongside `DATABASE_URL`. These require a restart
   to change anyway, so the DB adds no real value for them. DB is reserved for things that
   need to be editable at runtime via the UI.

3. **Combination of 1 + 2** — Infrastructure config (SMTP, limits, timeouts) goes in the
   config file. Runtime-editable system settings (maintenance mode, default theme for new
   sites, registration policy) go in `app_settings`. Per-site content settings stay in
   `site_settings`.

**Guiding question:** Which settings need to be changeable at runtime without a restart?
That split cleanly separates DB from config file.

**Status:** Pending decision. Do not wire up `/admin/settings` fully until resolved.

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
