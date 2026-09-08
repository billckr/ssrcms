---
title: CLI Tool (synaptic-cli)
group: system
updated_by: claude
last_updated: 2026-09-07
---


# CLI Tool (synap)

> Last updated: 2026-09-07 | Updated by: claude

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

Supports `--non-interactive` mode for scripted deployments; reads values from flags or env vars (`SYNAPTIC_DOMAIN`, `ADMIN_EMAIL`, `ADMIN_PASSWORD`, `DATABASE_URL`, `PORT`, `INSTALL_DIR`, `APP_NAME`, `APP_USER`, `NOTIFICATION_EMAIL`, `ADMIN_USERNAME`, `ADMIN_DISPLAY_NAME`, `SITE_URL`). If `ADMIN_PASSWORD` is omitted in non-interactive mode, a 16-character password is generated and printed once. **Bug found and fixed same day (2026-09-07):** `install`'s own local `generate_password()` initially still produced a 10-character password after the password-policy change (missed when the equivalent `core::models::user::generate_password()` was widened to 16 chars in the same security pass) and its output wasn't re-checked against `validate_password()` at the call site — a non-interactive install without `ADMIN_PASSWORD` set could briefly mint a super_admin whose password was shorter than the app's own stated 12-character minimum. Fixed by widening the generator to 16 characters (matching `core::models::user::generate_password()`) and adding an explicit `validate_password()` check on the generated value before use; a regression test (`password_tests::generated_admin_password_always_satisfies_current_policy`) generates 100 passwords and asserts each is 16 characters and policy-compliant.

`site_url` (used to build permalinks) defaults to deriving from `domain`+`port` (`http://domain` for 80, `https://domain` for 443, else `http://domain:port`) — this is wrong whenever a reverse proxy (Caddy) fronts the app on a different public port than Axum's internal listen port, since it bakes the internal port into every link. Pass `--site-url`/`SITE_URL` explicitly (e.g. `https://example.com`) to override; interactive mode now also prompts for it directly (with the same derived value as the default) rather than only being settable via the non-interactive flag. Like all `site_settings`, this is `ON CONFLICT DO NOTHING` — fixing it via flag/prompt only affects new installs; an already-wrong value needs a manual `UPDATE site_settings SET value=... WHERE key='site_url'` plus a service restart (settings are cached in memory at startup).

Password policy (updated 2026-09-07): 12–128 Unicode characters, no mandatory composition rules — previously 8–12 characters requiring an uppercase letter, a digit, and a symbol from `!@#$%&*-_+`.

**Restart required after every `install` run.** A running server loads its site cache once at startup and does not watch the database — `install`'s DB writes (new site, new/updated settings) have no effect on an already-running process until it's restarted (`systemctl restart synaptic-signals`, or `./app.sh restart` in dev). `install` now prints this reminder unconditionally after the summary, in both interactive and `--non-interactive` mode, since skipping it silently produces a confusing "No site found for hostname" on the homepage with no error anywhere.

**Interactive mode's "Next Steps" is now aware of an already-deployed service.** It used to always print a generic 6-step "copy the binary/systemd unit/Caddyfile, run caddy setup, enable the service" checklist, even on a re-run against a VPS that's already fully deployed and running — which read as required steps and was actively misleading (re-copying the freshly generated systemd unit in that case would have downgraded the service to run as whoever invoked `install`, often `root`, instead of the correct app user). It now checks whether `/etc/systemd/system/synaptic-signals.service` already exists: if so, it diffs the freshly generated Caddyfile/service unit against what's live and prints only what actually changed (or a one-line "nothing to copy" if they match); the full checklist only appears for a genuinely fresh install with no unit installed yet.

**Always run `install` (and `dev reset`) as the actual service user**, not bare `root` — e.g. `sudo -u www-data bash -c 'cd /var/www/bckr.dev && ./synap install'` (use the relative/full path; `sudo`'s `secure_path` on RHEL/AlmaLinux strips `/usr/local/bin` so the bare `synap` symlink isn't found under `sudo -u`). Running as root re-creates `search-index/`, `uploads/`, and `sites/` as root-owned; the live systemd service still runs as the app user and loses write access to the search index, causing it to crash-loop with `Error: Index already exists` until you `chown -R <app-user>:<app-user>` the install dir back.

**Re-running `install` without pinning `ADMIN_PASSWORD`/typing a new password silently rotates the admin password** — the user insert is `ON CONFLICT (email) DO UPDATE SET password_hash = ...`, so every re-run invalidates the previous password and prints a new `GENERATED_ADMIN_PASSWORD` (non-interactive) or prompts for a fresh one (interactive). Easy to lock yourself out by re-running `install` for an unrelated reason (e.g. just to test something) without noticing.

`install` is idempotent against a re-run on an existing site: the `sites` insert uses `ON CONFLICT (hostname) DO NOTHING`, then looks up the row's real `id` by hostname before seeding `site_settings` — this lookup was added to fix a foreign-key violation that occurred on re-install when the freshly-generated (but never-inserted) UUID was used instead of the existing site's actual id.

### `migrate`
Runs pending migrations against the database. Accepts `--database-url` flag or reads `DATABASE_URL` from the environment. Used when upgrading without re-running the full installer.

### `dev reset`
**Destructive — development only.** Wipes all data rows from every table except `_sqlx_migrations` and `documentation` (both are intentionally preserved). Verifies the `super_admin` password before proceeding. Shows a summary of what will be wiped (sites, user count, post count, media count). Also removes `themes/sites/`, `themes/private/`, and `uploads/` subdirectories if `INSTALL_DIR` is set. Supports `--force` to skip the confirmation prompt.

### `user create`
**Requires the super-admin password (added 2026-08-05)** — before any prompts, verifies a password (via `--password <PASSWORD>` for scripting, or an interactive prompt otherwise) against the current `is_protected = TRUE` user's Argon2 hash. Bails with a clear error if no super_admin exists yet (run `synap install` first). Having server/DB access to run the CLI is no longer sufficient on its own to mint accounts — reuses a new shared `verify_super_admin_password` helper in `cli/src/commands/mod.rs`, the same pattern `dev reset` already used.

Interactively creates a new user. Prompts for:
- **Username** (8–15 chars, lowercase/digits/hyphens) — validated with a retry loop, mirroring `validate_username` from the web forms (duplicated locally in `cli/src/commands/user.rs` since the CLI doesn't depend on the core crate).
- **Email**.
- **Display name** (≤60 chars, defaults to the username) — validated with a retry loop, mirroring `validate_display_name`.
- **Password** (12–128 Unicode characters, no composition rules, updated 2026-09-07) — validated the same way as web signup and the admin profile password change.
- **Role** (super_admin / editor / author / subscriber).
- **Site assignment (added 2026-08-05)** — for any role except `super_admin` (which has global access, not site-scoped), if any sites exist, prompts to assign the new user to one (or leave "(Unassigned)", the previous default/only behavior). Writes a `site_users` row with `invited_by = NULL` (CLI-seeded, no attributable inviter) using the same role string. Previously there was no way to do this at creation time — new CLI users always started unassigned, requiring a separate step via the admin UI's Site Access page.

Hashes password with Argon2. Choosing `super_admin` also sets `is_protected=TRUE` on the row (fixed 2026-07-22) — previously only `install`'s admin-creation step set this flag, so a super_admin created via `user create` instead of `install` silently ended up unprotected: invisible to `dev reset`'s super_admin lookup (`WHERE is_protected = TRUE`, which would then falsely report "database already reset"), exempt from the delete-protection checks in the admin Users page, and unable to be picked up by the owner-auto-assignment logic in `site create`/site rename (all of which key off `is_protected`, not `role`).

### `user list`
Lists all users (id, username, email, role, created_at) ordered by creation time.

### `user reset-password`
Looks up a user by email and sets a new Argon2-hashed password interactively.

### `site create`
Adds a new empty site by hostname. Auto-assigns the protected super_admin as owner. Optionally copies `themes/global/default/` into the new site's theme folder (`--themes-dir`).

**Removed command: `site init`** (2026-07-22). It existed to backfill a pre-existing single-site database's content with a `site_id` after multi-site migrations (0008–0011) ran, and to create that first site row — but `install` already creates the first site directly (`INSERT INTO sites ... ON CONFLICT (hostname) DO NOTHING`), making `site init` redundant for any install using the current `install` flow, fresh or otherwise: there has never been a pre-multisite install of this app (only the local dev machine and the shared test VPS, both routinely wiped/reinstalled), so the backfill case it existed for never occurred in practice. It was also a latent footgun — if ever run before `install` on a hostname `install` would later reuse, the site it created would end up with `owner_user_id = NULL` (no admin existed yet to claim ownership at that point), and `install`'s `ON CONFLICT (hostname) DO NOTHING` would then silently preserve that ownerless row instead of fixing it, permanently breaking the owner-gated `can_manage`/`is_owner` checks in `core/src/handlers/admin/sites.rs` for that site. New installs now provision their first site exclusively via `install`; additional sites via `site create` or the admin UI's `/admin/sites/new`.

### `site list`
Lists all sites (id, hostname, post count) ordered by creation time.

### `site delete`
Deletes a site and all its content via `CASCADE`. Prompts for confirmation.

### `site maintenance on|off|status`
WordPress-style maintenance mode, toggled per site. `on [--hostname] [--message]` and `off [--hostname]` write `maintenance_mode` (`true`/`false`) and `maintenance_message` into `site_settings` via a plain upsert (`ON CONFLICT ... DO UPDATE`, unlike the install-time seed rows). `--hostname` is required only if more than one site exists — with a single site it's auto-selected. `status` prints the current mode and stored message. If `--message` is omitted on `on`, the previous message is reused, falling back to a default WP-style sentence.

Scoped to the target site's `site_id` only — other sites on the same multi-site install keep serving normally while one is in maintenance mode. Verified: with one site's `maintenance_mode` set, a request for a different site's hostname on the same server still returned 200.

Enforced by `core/src/middleware/maintenance.rs`, a global Axum middleware layered via `middleware::from_fn_with_state` in `router.rs`. It runs a live, uncached query (`SELECT value FROM site_settings WHERE site_id=$1 AND key='maintenance_mode'`) on every request, resolving the site from the `Host` header via `state.resolve_site()` — deliberately **not** cached like `active_theme`, so the CLI toggle takes effect immediately with no restart and no reload signal. Exempts `/admin*`, `/theme/static*`, `/uploads*`, and `/metrics` so an operator can still log in to turn it back off and static assets keep loading. Renders a hand-written HTML page (not a Tera theme template) with a `503 Service Unavailable` status and `Retry-After: 3600` header.

### `site allow-ip on|off|add|remove|status`
Per-site IP allowlist — like an `.htaccess` Allow/Deny list. `on --ip <cidr>` (repeatable for multiple entries) blocks **all** traffic to the site except from the given IPs/CIDRs (IPv4 or IPv6, e.g. `203.0.113.9` or `10.0.0.0/8`); `off` restores open access; `status` prints on/off and the stored list. `add --ip <cidr>` appends a single entry without touching the rest of the list (and turns the allowlist on if it wasn't already); `remove --ip <cidr>` deletes a single entry, leaving the rest in place. Values are written to `site_settings` as `ip_allowlist_enabled` (`true`/`false`) and `ip_allowlist` (comma-separated), via the same live-upsert pattern as `site maintenance`. `--hostname` is required only if more than one site exists.

`remove` **refuses to delete the last remaining IP** while the allowlist is still enabled — that would leave the allowlist on with nobody, including you, able to reach `/admin`. Run `allow-ip off` instead if the intent is to fully reopen the site.

Unlike maintenance mode, **`/admin` is not exempt** — if enabled and your own IP isn't on the list, you lock yourself out of the admin too, with no remote escape hatch; recovery requires shell/SSH access to the server to run `allow-ip off` directly. This is intentional: the use case is hard isolation of a test/staging deploy (e.g. a VPS site you don't want anyone else reaching yet), not a "the operator can still log in" gate like maintenance mode.

Enforced by `core/src/middleware/ip_allowlist.rs`, layered in `router.rs` so it runs *before* the maintenance-mode check (an IP block takes priority even if maintenance mode is also off). Determines the real client IP by checking `X-Real-IP`, then the first hop of `X-Forwarded-For`, finally the raw socket address — trustworthy here because Axum only binds to a private interface behind Caddy, so an outside caller can never reach Axum directly to forge those headers themselves. CIDR matching is a small hand-written IPv4/u32 and IPv6/u128 bitmask comparison (no external crate), shared with `block-ip` below. Verified locally and live on the VPS (bckr.dev): blocks by default, matching single IPs and CIDR ranges pass, a spoofed out-of-range `X-Real-IP`/`X-Forwarded-For` is rejected, another site on the same server is unaffected, `add`/`remove` adjust the list without disturbing other entries, and `off` restores access — all live, no restart.

**Examples:**
```
# Lock a VPS test deploy down to just your own IP (run as the app's service user)
sudo -u www-data bash -c 'cd /var/www/bckr.dev && ./synap site allow-ip on --hostname bckr.dev --ip 203.0.113.9'

# Allow a CIDR range instead (e.g. an office network) — quotes matter, / is a shell no-op but some shells still complain without them
sudo -u www-data bash -c 'cd /var/www/bckr.dev && ./synap site allow-ip on --hostname bckr.dev --ip "203.0.113.0/24"'

# Allow more than one IP/CIDR at once (repeat --ip)
./synap site allow-ip on --hostname bckr.dev --ip 203.0.113.9 --ip 198.51.100.0/24

# Trust a second teammate's IP without retyping the whole list
./synap site allow-ip add --hostname bckr.dev --ip 198.51.100.42

# That teammate leaves — remove just their IP, yours stays allowed
./synap site allow-ip remove --hostname bckr.dev --ip 198.51.100.42

# Check what's currently allowed
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

Unlike `allow-ip remove`, `block-ip remove` **auto-disables** the denylist when the last entry is removed — an empty denylist safely means "block nobody," so there's no lockout risk to guard against. Use `allow-ip` when you want to restrict a site to a small trusted set (e.g. isolating a VPS test deploy); use `block-ip` when the site should stay public but a specific IP (e.g. an abusive scraper) needs to be kept out.

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

# Check what's currently blocked
./synap site block-ip status --hostname bckr.dev
#   Site: bckr.dev
#   IP denylist: ON
#   Blocked: 198.51.100.13,203.0.113.66

# Remove the last blocked IP — denylist auto-turns itself OFF (no explicit `off` needed)
./synap site block-ip remove --hostname bckr.dev --ip 198.51.100.13
./synap site block-ip remove --hostname bckr.dev --ip 203.0.113.66
#   Removed '203.0.113.66' from the denylist for 'bckr.dev'.
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

**Only works while the app is stopped.** Tantivy allows exactly one `IndexWriter` on the index directory at a time, and a running server holds it open for its entire process lifetime — so `open_or_create` fails with a `LockBusy` error if the server is up, and the command prints a message pointing at the admin UI's "Rebuild Search Index" button (Settings → Advanced) as the live-app alternative. Intended for offline/scripted use, e.g. right after a bulk import or DB restore done with the app down:
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
- `--defaults`: skip the menu, use built-in/env-var defaults, no prompts. Also automatic whenever stdin isn't a TTY, so still CI-safe.
- `--interactive`: skip the menu, go straight to the field-by-field wizard.
- `--update` (renamed from `--no-install`): push a code update to an already-running install — rebuild, re-ship, apply pending migrations, restart. Does **not** create a site/admin or touch existing data. This is the intended production shape: get the app running first, then configure it via `synap install` as a separate deliberate step.
- `--clean`: force-drops the DB (`WITH (FORCE)`, PG13+) and wipes `INSTALL_DIR` entirely.

### Destructive-migration heads-up
Before applying migrations against an already-populated DB (skipped on `--clean`/fresh installs, since there's nothing to lose), a static keyword scan (`DROP TABLE`/`DROP COLUMN`/`TRUNCATE`/`DELETE FROM`/`RENAME`) of not-yet-applied `.sql` files prompts to confirm in a TTY (declining aborts before the DB is touched), or just warns loudly in `--defaults`/non-TTY runs (surfaced in the final summary). It's a heuristic scan, not a certified safety check.

### Known gotcha: running `synap install` by hand
Must run as the service user (`synap` checks it owns `$INSTALL_DIR`) and must be invoked via a relative/full path (`./synap install`), not the bare command — `sudo`'s `secure_path` on RHEL/AlmaLinux strips `/usr/local/bin`, so `sudo -u www-data synap install` fails with "command not found" even though the symlink exists.

### Fixed bug: `/theme/*` Caddy bypass 404'd every theme
`deployment/Caddyfile.template` (and its compiled-in fallback `cli/deployment_templates/Caddyfile.template`) used to have `handle /theme/* { file_server }`, bypassing Axum for performance. But every theme's `base.html` links the identical `/theme/static/css/style.css` — there's no theme name in the URL. Which theme's files actually get served is resolved dynamically per-request in `core/src/handlers/theme_static.rs` (Host header → site → active theme), which a flat file_server can never do. Fixed by removing that block so `/theme/*` falls through to `reverse_proxy` → Axum. `/uploads/*` correctly stays on the Caddy bypass — the app maintains an `uploads/{hostname}/ → uploads/{site-uuid}/` symlink specifically so a flat file_server works there.

## Security Notes

- `dev reset` requires the current `super_admin` password (Argon2-verified) before wiping data.
- `caddy setup` / `caddy teardown` must run as root.
- Password generation excludes `$` and `!` to avoid shell variable expansion issues in env files and URL strings.
- Install-time admin users are inserted with `is_protected=TRUE` to prevent accidental deletion.

## Known Limitations / TODOs

- Migrations are embedded at compile time. Every new migration file requires `cargo install --path cli --force` before `synap migrate` or `install` will see it (or, for VPS deploys, an unconditional rebuild — see `scripts/install-vps.sh` above).
- `theme activate` updates `site_settings` using the old single-column conflict key (`ON CONFLICT (key)`) which may not work correctly in multi-site installs where `site_id` scoping is required.




