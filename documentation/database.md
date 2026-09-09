---
title: Database Schema
group: system
updated_by: claude
last_updated: 2026-09-08
---

# Database Schema

> Last updated: 2026-09-08 | Updated by: claude

## Overview

The database is PostgreSQL. All schema changes are applied via numbered SQL migration files in
`migrations/`. Migrations are embedded into the binary at compile time via `sqlx::migrate!()`
and run automatically on startup or via `synap migrate`. There are 71 migrations in the
current codebase (up from 62).

## How It Works

Each migration is a plain SQL file named `NNNN_description.sql`. SQLx tracks applied migrations
in `_sqlx_migrations`. Migrations are strictly additive — earlier files are never modified. The
`synap install` and `synap migrate` commands both call
`sqlx::migrate!("../migrations")`. Because migrations are embedded at compile time, adding a new
migration file requires rebuilding the binary.

Per-site feature toggles introduced recently (maintenance mode, IP allow/deny lists) do **not**
have their own tables — they're stored as rows in the existing generic `site_settings`
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
  `location TEXT` (NULL = name-only, `'primary'`/`'footer'` = auto-loaded into template
  context), `created_at TIMESTAMPTZ`, `updated_at TIMESTAMPTZ`
- `UNIQUE (site_id, name)` — menu names unique per site
- Location uniqueness (one menu per location per site) enforced at the application layer, not DB

**nav_menu_items** (0040)
- `id UUID PK`, `menu_id UUID NOT NULL REFERENCES nav_menus ON DELETE CASCADE`,
  `parent_id UUID REFERENCES nav_menu_items ON DELETE CASCADE` (self-referencing, dropdown
  nesting), `sort_order INT DEFAULT 0`, `label TEXT NOT NULL`, `url TEXT` (ignored when
  `page_id` set), `page_id UUID REFERENCES posts ON DELETE SET NULL`, `target TEXT DEFAULT
  '_self'`, `created_at TIMESTAMPTZ`
- Indexed on `menu_id` and `parent_id`

**page_compositions** (0041, extended by 0042, 0044, 0045, 0046→0047 reverted) — backs the Puck
visual page builder's per-page content:
- `id UUID PK`, `site_id UUID` (FK `sites`, cascade), `name VARCHAR(255)`,
  `composition JSONB DEFAULT '{}'` (live/published content), `is_homepage BOOL`,
  `created_by UUID`, `created_at`/`updated_at TIMESTAMPTZ`
- `project_id UUID REFERENCES builder_projects ON DELETE CASCADE` (0042) — links a composition
  to a project
- `slug VARCHAR(100)`, `page_type VARCHAR(20) DEFAULT 'page'` (0044) — homepage slug is always
  `/`; unique index `(project_id, slug)` where `slug IS NOT NULL`
- `draft_composition JSONB DEFAULT '{}'` (0045) — separate work-in-progress column so in-flight
  edits don't clobber the published `composition` until Publish; seeded from `composition` on
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
predate this doc's last full pass; see the **Form Designer**, **Forms**, and **Email Providers**
docs for those in full instead of this file.)

**form_submissions.form_id** (0060) — `UUID NULL REFERENCES forms(id) ON DELETE SET NULL`,
backfilled for existing rows. Gives submissions an exact FK to the form that collected them,
alongside the pre-existing `form_name` (slug) text match, which stays the authoritative lookup
key everywhere (it's immutable and still resolves even for orphaned/pre-FK rows where `form_id`
is NULL). See the **Forms** doc.

**forms.total_submissions** (0061) — `BIGINT NOT NULL DEFAULT 0`, backfilled from existing
`form_submissions` counts via the new `form_id` FK. A lifetime counter incremented once per
submission and never decremented, kept deliberately separate from the live submission count used
for the Submissions tab's pagination — see the **Form Designer** doc.

### site_users PK widened for multi-role (0062, 2026-08-18)

`site_users`' `PRIMARY KEY (site_id, user_id)` was replaced with a surrogate `id UUID` PK plus
`UNIQUE(site_id, user_id, role)`, so a user can now hold more than one role on the same site
(previously exactly one, enforced by the old composite PK). The `role` CHECK constraint itself
(`admin | editor | author | subscriber`) was deliberately left unchanged — it never permitted
`super_admin`/`site_admin` and still doesn't; a matching guarantee was added independently at
the Rust type level via a `SiteRole` enum with no such variant. See the **Users & Roles** doc's
"Multiple roles per user per site" section for the full session/login-picker flow this enabled.

### Case-insensitive email uniqueness (0070, 2026-09-07)

Part of the authentication security hardening (see `AUTH_SECURITY_REVIEW.md` /
`AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`). `UPDATE users SET email = lower(trim(email))`
normalizes any existing mixed-case/whitespace rows, then
`CREATE UNIQUE INDEX users_email_lower_unique ON users (lower(email))` enforces the
invariant in Postgres, matching the application-level normalization now applied by
`user::normalize_email` on every create/update/lookup. If legacy data has case-only
duplicate emails, this migration intentionally fails so an operator resolves them first —
see the deployment checklist in `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`.

(Migrations 0063–0069 in between — `sites.parent_site_id`, polls, WP-import media/post
maps, `site_users.can_self_publish`, and `users.personal_data_erased_at`/
`welcome_panel_dismissed_at` — are unrelated feature work, not part of this security pass.)

### Verified email-change tokens (0071, 2026-09-08)

**email_changes** — `id UUID PK`, `user_id UUID` (FK `users`, `ON DELETE CASCADE`),
`new_email TEXT`, `token_hash TEXT`, `expires_at TIMESTAMPTZ`, `used_at TIMESTAMPTZ`,
`created_at TIMESTAMPTZ`. Indexed on `token_hash` and `user_id`. Mirrors `password_resets`'
shape and posture exactly: a hashed, single-use, 60-minute token backs a verified
self-service email change at `POST /account/email/change` (subscriber-only for now — see the
**Account Area** doc). The candidate address sits in `new_email` until the token is consumed;
`consume_and_apply` does the token-burn check and the `users.email` UPDATE in one transaction,
so a token can never be spent without the change landing, or vice versa. Requesting a new
change deletes any prior unused row for the same user first (one active pending change per
user, same invariant as `password_resets`). No DB-level uniqueness on `new_email` — the
existing `users_email_lower_unique` index (0070, above) is what actually rejects a collision,
at consume time, not at request time.

Also part of this change: `User::credential_version()` (`core/src/models/user.rs`) now hashes
`email` alongside `password_hash`, so completing a verified email change invalidates every
other active session for the account, the same way a password change already does — see the
session-rotation note in the **Middleware & Auth** doc.

### `users.session_nonce` (0002, post-baseline, 2026-09-08)

All 72 migrations up to and including 0071 above were squashed into `0001_baseline.sql` on
2026-09-08 (see `synapcms_migration_baseline` in the repo's session memory, if reading this from
a later session) — new migrations resume numbering from `0002`. This is the first one:
`ALTER TABLE users ADD COLUMN session_nonce UUID NOT NULL DEFAULT gen_random_uuid()`.

The column has no meaning of its own — it exists purely as a third input to
`User::credential_version()` (alongside `password_hash` and `email`) so that "Sign out other
devices" (`POST /account/profile/sign-out-other-devices`, `POST
/admin/profile/sign-out-other-devices`) has something to change that invalidates every other
session without also touching a recovery-sensitive field. See the manual-invalidation note in
the **Middleware & Auth** doc, and the **Account Area** doc for the user-facing flow.

### AI translation (0003, post-baseline, 2026-09-09)

`ai_providers` (per-site AI provider credentials, JSON-blob-in-one-encrypted-column, same shape
as `email_providers`) and `post_translations` (one row per `(post_id, locale)`: title/excerpt/
content plus `source_updated_at` for staleness detection). A site's enabled locales are *not* a
new table — a single `site_settings` row (key `enabled_locales`), the same list-shaped KV
convention `ip_allowlist` already uses. See the **AI Post Translation** doc for the full feature.

## Known Limitations / TODOs

Because `sqlx::migrate!()` embeds migrations at compile time, adding a new migration file
requires rebuilding the binary. Running an old binary against a DB with newer migrations applied
will fail at startup.

0046/0047 illustrate that a migration is not the place to experiment — `is_post_template` was
added and dropped again within two consecutive migrations, meaning both are effectively dead
weight in the migration history (harmless, but worth knowing when reading migration history for
context on `page_compositions`).
