---
title: Middleware & Auth
group: system
updated_by: claude
last_updated: 2026-09-08
---

# Middleware & Auth

> Last updated: 2026-09-08 | Updated by: claude

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
old doc's `RequireAdmin` name no longer exists in source). It:

1. Reads `admin_user_id` (`SESSION_USER_ID_KEY`) from the session; redirects to `/admin/login`
   (`AdminAuthError::NotAuthenticated`) if absent or the user lookup fails.
2. Rejects with `403 Forbidden` unless the user's global `role` is one of
   `super_admin | site_admin | editor | author`.
3. Resolves the **current site** (`site_id`): prefers `current_site_id` from the session
   (re-validated against the DB, clearing the key if the site was deleted); otherwise, for a
   global admin, resolves via the request's `Host` header (reloading the stale site cache once
   on a miss) or falls back to the first site in the DB; for a non-admin site user, picks their
   first accessible site from `site_user::list_for_user`. The resolved id is written back into
   the session. This user/site resolution is shared (`resolve_user_and_site`) with the lighter
   `PickRoleUser` extractor used by `/admin/pick-role` (added 2026-08-18, see below).
4. Resolves `site_role: Option<SiteRole>` for that site (added 2026-08-18, was a plain `String`
   before). Global admins are always `Some(SiteRole::Admin)`, resolved without ever consulting
   `site_users`. Otherwise, `site_user::list_roles_for_user_and_site` is queried live (no
   caching in the session cookie): 0 roles falls back to the user's global role parsed as a
   `SiteRole` (yields `None` for `site_admin`/`super_admin`, since those aren't valid site
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
   `is_on_default_site` (system settings are restricted to a super_admin's own default/home
   site). `is_impersonating` is true when a super_admin is viewing a site other than their
   `default_site_id` (drives the "visiting" badge in the admin UI).

**Capability fix (2026-09-07):** `can_manage_content` previously defaulted to `true`
regardless of site role, so a subscriber-only site membership could still receive
content-management capability. It is now `is_editor_or_above || site_role ==
Some(SiteRole::Author)` — a subscriber, or a user with no resolvable site role, gets no
admin capabilities at all.

**Session rotation, credential invalidation, and absolute lifetime (2026-09-07):**
`login_post` calls `session.cycle_id()` before writing `admin_user_id`, replacing the
pre-auth session id with a fresh one (session-fixation defense). It also writes
`SESSION_CREDENTIAL_VERSION_KEY` (`admin_credential_version`) — `User::credential_version()`,
a SHA-256 digest of the current `password_hash` — and `SESSION_LOGIN_AT_KEY`
(`admin_login_at`, a Unix timestamp). `AdminUser` extraction compares the session's
credential version against the user's *current* one on every request and rejects the
session if they differ, so changing or recovering a password immediately invalidates every
other active session (the session used to make the change is rewritten with the new marker
so it keeps working). `admin_login_at` also enforces `ADMIN_ABSOLUTE_SESSION_SECONDS` (24h)
on top of the 2h inactivity timeout below — an actively-used session can no longer stay valid
indefinitely. `AccountUser` follows the identical pattern with its own keys
(`SESSION_ACCOUNT_CREDENTIAL_VERSION_KEY`/`account_credential_version`,
`SESSION_ACCOUNT_LOGIN_AT_KEY`/`account_login_at`) and a 7-day absolute lifetime
(`ACCOUNT_ABSOLUTE_SESSION_SECONDS`).

**`credential_version()` now covers email too (2026-09-08):** the digest is
`SHA-256(password_hash || "|" || email)`, not just the password hash. This closes the same
gap for email as already existed for passwords — completing the new verified email-change
flow (`POST /account/email/confirm/{token}`, see the **Account Area** doc) changes the user's
`email` column, which now bumps this marker exactly like a password change does, forcing
every other active session for the account to re-authenticate. The session that performed the
confirmation has its own stored marker refreshed in the same request so it isn't logged out by
the very change it just made — the identical pattern the password-change handlers already use.

**Manual invalidation via `session_nonce` (2026-09-08):** the digest is now
`SHA-256(password_hash || "|" || email || "|" || session_nonce)` — a third `users` column
(migration 0002, post-baseline) with no meaning of its own beyond feeding this hash. Every prior
trigger for invalidating other sessions was a side effect of changing something the user was
already changing for another reason (a new password, a new email). "Sign out other devices"
(`POST /account/profile/sign-out-other-devices`, `POST /admin/profile/sign-out-other-devices`)
is the first *direct* trigger: `user::regenerate_session_nonce` assigns a fresh random UUID to
this column and nothing else, so it can invalidate every other session on demand without also
needing a reason tied to the password or email fields. Same re-verification posture as the rest
of this list: the clicking session's own credential-version marker is refreshed in the same
request so the click doesn't log out the device that made it.

**Pending TOTP MFA state (2026-09-11):** a staff account with TOTP enabled doesn't get
`SESSION_USER_ID_KEY` written on a correct password alone. `handlers::auth::login_post` instead
writes three short-lived keys — `SESSION_MFA_PENDING_USER_ID_KEY`, `SESSION_MFA_PENDING_SITE_ID_KEY`,
`SESSION_MFA_PENDING_AT_KEY` (a 5-minute completion window, enforced by the pure function
`admin_auth::mfa_pending_expired`) — and redirects to `GET /admin/login/mfa` instead of `/admin`.
None of these three keys alone satisfy `AdminUser` (only `SESSION_USER_ID_KEY` does), so a stolen
pending-MFA cookie only lets an attacker *attempt* the second factor, still subject to the same
`auth_security` rate limiting as everything else here (flow `"admin-mfa"`, keyed by user id). The
session id is rotated once when the pending state is created and again when the second factor
succeeds (`finish_admin_login`, shared by both the MFA and non-MFA login paths so they write the
exact same final session keys) — rotating on every privilege escalation, not just once. See the
**Admin Area** doc's Two-Factor Authentication section for the full flow.

### Account Auth (`account_auth.rs`)

`AccountUser` extractor for any authenticated non-admin user (subscriber and above), keyed on
its own session key `account_user_id` (`SESSION_ACCOUNT_USER_ID_KEY`) — entirely separate from
the admin session key, so a browser can be logged into `/admin` and `/account` as two different
users simultaneously. Also resolves `site_id`, `site_name`, and `site_base_url` from the `Host`
header via `state.resolve_site()` for "back to site" links. Rejects to `/login`.

### CSRF Protection (`csrf.rs`, added 2026-09-07)

`same_origin()` is a global `middleware::from_fn` layer (see Layer Order below) that rejects
state-changing requests (anything but GET/HEAD/OPTIONS) to `/admin*`, `/account*`, `/login`,
`/subscribe`, and `/recover*` unless the `Origin` header exactly matches the `Host` header
(accounting for Caddy's `X-Forwarded-Proto` when deciding whether the expected origin is
`http://` or `https://`). This blocks cross-site POSTs even from a same-site sibling
subdomain, which `SameSite=Lax` cookies alone don't cover. Public, unauthenticated endpoints
such as `/form/{name}` are deliberately excluded — they may need to accept cross-origin
embeds.

### Authentication Rate Limiting (`auth_security.rs`, added 2026-09-07)

`allow(endpoint, headers, identity)` is called directly inside the admin-login,
subscriber-login, `/subscribe`, `/recover`, and (added 2026-09-08) `/account/email/change`
handlers — it is not a router-level layer.
It checks two in-process token buckets against a 15-minute rolling window: the caller's IP
(from `X-Forwarded-For`) capped at 60 requests, and the normalized account identifier
(email) capped at 30. Buckets live in a `Lazy<Mutex<HashMap<...>>>`; once the map reaches
10,000 entries it prunes stale buckets before accepting new keys, bounding memory under a
distributed identifier-flood. This is single-process only — before running more than one
app instance, move the counters to PostgreSQL or a shared cache so limits apply across
instances.

**Escalating delay on repeated login failures (2026-09-08):** `allow()` above is a flat
ceiling — every attempt costs the same right up until the window's limit, then a hard
block. A second, separate mechanism in the same file (`login_delay_remaining`,
`record_login_failure`, `record_login_success`) adds exponential backoff specifically for
wrong-password guessing against `/admin/login`: the first 3 (`FREE_ATTEMPTS`) failures for a
given identity cost nothing, then each subsequent one roughly doubles the wait before the
next attempt is even evaluated (2s, 4s, 8s, ... capped at 5 minutes), tracked in its own
`Lazy<Mutex<HashMap<String, FailureState>>>` keyed by `flow:identity` (no IP component — a
botnet spreading guesses across many IPs at one account is exactly the case a purely
IP-based scheme misses). A correct password clears the identity's failure history even if a
later authorization step (role/site mismatch) still rejects the request, since that's a
wrong-form mistake, not a credential-guessing signal. Deliberately scoped to `/admin/login`
only for now, not `/login` (subscriber) — mirrors the existing `log_staff_login` precedent
in `handlers/auth.rs` treating subscriber logins as high-volume/low-stakes relative to staff
access; extending it to the subscriber flow (or to `/recover`, `/subscribe`) is a small,
separate follow-up if warranted later.

### Session Timeouts & Logout

Admin and account logins use two entirely separate `tower_sessions` cookies/layers
(`core/src/main.rs`), sized to risk level per OWASP session-management guidance rather than
sharing one timeout: higher-privilege accounts get a shorter leash.

- **`admin_session`** — 2h inactivity timeout. Used by `/admin/*`.
- **`session`** — 24h inactivity timeout. Used by everything else (public content, `/login`,
  `/account/*`).

As of 2026-09-07 both layers also enforce an absolute session lifetime independent of
inactivity (24h for admin, 7 days for account) via a login-time timestamp compared on every
request — see the session-rotation note under Admin Auth above.

Both are `Expiry::OnInactivity` **and** `with_always_save(true)`. The `always_save` flag is
required for "inactivity" to mean what it says: `tower_sessions` only recomputes a session's
expiry when a request *writes* to it, and most page views (`AdminUser`/`AccountUser` extractors)
only *read* the session to check who's logged in. Without `always_save`, the timeout would
silently behave like a fixed timer from login instead of a rolling window — an actively-working
admin would still get booted at a fixed point regardless of activity.

Because a single shared session's expiry can't vary by login type (the layer's config, not the
DB record, determines expiry on every save), two distinct `SessionManagerLayer`s were required.
`router.rs::build()` reflects this: routes are split into `public_router` (content routes,
`/login`, `/account/*`, static/plugin routes, and the `page::single_page` fallback) and
`admin_router` (`/admin/*`), each wrapped with its own session layer *before* being merged —
any handler extracting `Session` must live in the group whose layer actually inserts that
extension, or extraction fails at runtime. (This bit a first pass: the fallback route was
originally registered on the merged router, outside both layers, and 500'd on any unmatched
`/{slug}` since `page::single_page` extracts `Session`.)

`logout` (`auth::logout`, `auth::account_logout`) calls `session.flush()`, not
`session.remove(key)`. Removing just the auth key looked like a fix but wasn't: it leaves the
session record (and cookie) alive, and `tower_sessions`' `is_empty()` check only triggers cookie
removal when a request arrives with *no* session id at all — a request carrying an existing
cookie never qualifies, even with an empty data map. The old code was quietly **renewing** a
full-length "logged out" cookie on every logout instead of invalidating it. `flush()` deletes the
store row, clears the session id, and does trigger `is_empty()` — the browser gets a real
`Set-Cookie: ...; Max-Age=0` and the DB row is gone, not just emptied.

### Maintenance Mode (`maintenance.rs`)

`gate()` is a `middleware::from_fn_with_state` layer applied globally in `router.rs`. For every
request it checks (live, no cache — a single indexed `site_settings` query) whether the
resolved site has `maintenance_mode = 'true'`; if so it renders a branded 503 page (custom
`maintenance_message` setting, default message, `Retry-After: 3600`) instead of continuing.
`/admin*`, `/theme/static*`, `/uploads*`, and `/metrics` are always exempt (`is_exempt()`) so an
operator can still log in to disable it and so the maintenance page's own assets still load.
Requests with no resolvable `Host` or site pass through unaffected. Toggled via
`synap site maintenance on/off` — takes effect immediately, no restart.

### IP Allowlist (`ip_allowlist.rs`) / IP Denylist (`ip_denylist.rs`)

Two symmetric per-site gates, also applied globally and checked live (no cache) on every
request:
- **Allowlist**: if `ip_allowlist_enabled = 'true'` for the resolved site, the caller's IP must
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
`csrf::same_origin` → `no_store_for_protected` → `maintenance_layer` → `ip_allowlist_layer` →
`ip_denylist_layer` → `track_http_metrics` → `TraceLayer`. `csrf::same_origin` was added
2026-09-07 as the innermost layer (closest to the router), so CSRF rejection happens after
the maintenance/IP gates but before any handler runs.

## Security Notes

- Stale site-cache entries (e.g. after `dev reset` without restart) are caught by DB validation
  in both `CurrentSite` and `AdminUser` and return 404 / re-resolve rather than serving or
  authenticating against a ghost site.
- `Cache-Control: no-store` is applied to all `/admin/*` and `/account/*` responses
  (`no_store_for_protected` in `router.rs`) to prevent back-button cache leakage after logout.
- Maintenance mode always exempts `/admin/*` so it can't be used to lock out the operator;
  IP allowlist/denylist intentionally make no such exception.
- `real_ip()` trusts `X-Real-Ip`/`X-Forwarded-For` only because Caddy is the sole thing able to
  reach the Axum process — this assumption breaks if the app is ever exposed directly.
- Renaming the session cookies (`id` → `admin_session`/`session`) orphaned every pre-existing
  session on deploy — a one-time, expected side effect, not a bug. Old rows linger in
  `tower_sessions.session` (schema `tower_sessions`) until their original expiry passes; they're
  inert since no layer reads a cookie named `id` anymore.
- CSRF (`csrf::same_origin`) and per-endpoint rate limiting (`auth_security::allow`) were
  added 2026-09-07 — see the dedicated sections above.
- A credential-version marker invalidates every other session immediately after a password
  change/reset **or** (added 2026-09-08) a verified email change; see the Admin Auth
  session-rotation note above.
- Cookie `Secure` is now tied to `dev_mode` in `synaptic.toml` rather than hardcoded `false`
  (`core/src/main.rs`) — production (`dev_mode = false`) marks both session cookies `Secure`;
  only explicit local dev disables it.




### Public Account-Action Hardening (2026-09-07)

Account identity resolution is now shared across protected account routes, public-page navigation, comments, saved-post actions, and site membership checks. Expired sessions, password-revoked sessions, inactive users, and users removed from the current site are rejected consistently. Authenticated public mutations at /{slug}/comment, /{slug}/save, and /{slug}/unsave now receive same-origin CSRF enforcement. Public draft preview also uses full admin-session expiry and credential validation and requires a content-capable site role. Small authentication/account forms have a 16 KiB request-body limit. See AUTH_EXPLOIT_REVIEW_RESULTS.md.
