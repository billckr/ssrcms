# TOTP MFA for Staff Logins — Implementation Results

Date: 2026-09-11

Related: `AUTH_SECURITY_REVIEW.md` (Phase 3: optional advanced local features — "TOTP
authenticator-app MFA for staff, plus hashed recovery codes"), `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`,
`AUTH_EXPLOIT_REVIEW_RESULTS.md`.

## Outcome

Staff logins (`/admin/login`) can now be protected by TOTP two-factor authentication, self-service
opt-in via `/admin/profile`. Implemented entirely with local Rust code and the existing PostgreSQL
database — no third-party identity provider, no SMS, no external QR-generation service. RFC
6238/4226 TOTP is pure local computation; the enrollment QR code is rendered locally to inline SVG.

Scope: staff only (`super_admin`/`site_admin`/`editor`/`author`). Subscribers, org-wide mandatory
enforcement, and WebAuthn/passkeys remain future follow-ups (see AUTH_SECURITY_REVIEW.md's
"Phase 3" and the current `TODO.md`).

## Implemented changes

### 1. TOTP algorithm (`core/src/utils/totp.rs`)

Hand-rolled against RFC 6238 (TOTP) / RFC 4226 (HOTP dynamic truncation) rather than a heavier
all-in-one crate — small enough to audit directly and verify against the RFC's own published test
vectors. New dependencies: `hmac`, `sha1` (HMAC-SHA1 is the algorithm every real authenticator app
implements — Google Authenticator, Authy, 1Password, Bitwarden), `base32` (RFC 4648 encoding for
the secret), `qrcode` (pure-Rust QR matrix, rendered to SVG via its `svg` feature).

- `generate_secret()` — 160-bit secret via `rand`'s OS CSPRNG.
- `generate_code()` / `verify_code()` — 30-second steps, 6 digits, ±1 step (±30s) clock-drift
  tolerance, constant-time code comparison.
- `provisioning_uri()` / `qr_svg()` — the `otpauth://` enrollment URI and its inline-SVG QR
  rendering.

### 2. Data model

- `core/src/models/user_totp.rs` + migration `0006_totp_mfa.sql`: one row per enrolled user.
  Secret is AES-256-GCM encrypted at rest via the existing `core::crypto` module (same mechanism
  as site email-provider API keys) — recoverable, not one-way hashed, since codes must be
  computed from it. `enabled_at IS NULL` = enrollment started but never confirmed.
  `last_used_step` blocks replaying an already-used code within its own validity window,
  enforced with `SELECT ... FOR UPDATE` so two concurrent requests can't both pass first.
- `core/src/models/mfa_recovery_code.rs` + the same migration: 10 single-use recovery codes per
  user, hashed (SHA-256) exactly like `password_resets.token_hash`.

### 3. Login gate (`core/src/handlers/auth.rs`)

`login_post` is unchanged for any account without MFA enabled. For one that does, after the
existing password/role/site-access checks succeed, a short-lived pending state
(`SESSION_MFA_PENDING_*` keys, 5-minute window) replaces the full session write, and the browser
is redirected to `GET /admin/login/mfa` instead of `/admin`. `auth.login_succeeded` is not
audit-logged until the second factor actually passes.

New routes `GET`/`POST /admin/login/mfa` accept either a 6-digit TOTP code or a recovery code,
rate-limited the same way `/admin/login` itself is (`middleware::auth_security`, new flow
`"admin-mfa"`, keyed by user id — zero changes needed to that module, it was already
flow-parameterized). On success they call the same `finish_admin_login` helper the non-MFA path
uses, so both write identical session keys and audit-log entries.

### 4. Enrollment / management UI (`/admin/profile`)

New "Two-factor authentication" card (`admin/src/pages/profile.rs`,
`admin/src/pages/profile_mfa.rs`). Setup, disable, and recovery-code regeneration each require
re-confirming the current password (same step-up pattern as changing a password), and each sends
a notification email to the account's own address — so a hijacked-session attacker silently
toggling MFA as a persistence mechanism gets caught by the real owner. Email is resolved through
`admin.site_id.or(admin.user.default_site_id)` (a pure global super_admin can have neither pinned),
skipping the send with a warning rather than blocking the action if no site is available — the
same "opt-in, not required" posture `mail::send_for_site` already has.

### 5. Lockout recovery

`POST /admin/users/{id}/disable-mfa` (`core/src/handlers/admin/users.rs`, super_admin only)
force-clears a staff member's TOTP secret and recovery codes if they lose both their authenticator
device and their codes — audit-logged, and emails the affected user a notice. Surfaced as a
"Two-Factor Authentication" reset button on the Edit User page (`admin/src/pages/users.rs`),
visible only to a super_admin and only when the target actually has TOTP enabled. Added
route-first, then wired into the Edit User page once the UI gap was pointed out — `UserEdit`'s new
`mfa_enabled` field required updating all 15 of its construction sites in
`core/src/handlers/admin/users.rs`; the compiler enumerated every one (`error[E0063]: missing
field`), so the update was mechanical rather than risky. 8 sites (every `save_new`/new-user path,
`id: None`) are always `false`; the `edit_user` GET handler and `save_edit`'s 6 re-render sites
fetch/reuse the real `user_totp::is_enabled` value. The icon reads as status (green shield =
enabled, matching the existing Account Status toggle's convention) rather than action — hovering
swaps it to the plain struck-through shield to preview the disable click.

## Verification results

Automated:

```text
cargo test -p synaptic-core --lib
cargo test -p synap-cli
cargo test -p synaptic-core --test routes -- --include-ignored --test-threads=1
cargo check --workspace
git diff --check
```

- Core library: 208 tests passed, 0 failed (28 new: 19 in `utils::totp` including all four RFC
  6238 Appendix B known-answer vectors, 5 in `models::mfa_recovery_code`, 3 in
  `middleware::admin_auth`'s pending-window boundary logic, plus 1 pre-existing `models::user`
  suite unaffected).
- CLI: 6 tests passed, 0 failed.
- HTTP integration suite (`core/tests/routes.rs`, live Postgres): 24 tests passed, 0 failed — 12
  new (enrollment happy path, unenrolled-account regression guard, wrong-code rejection,
  replay-protection, recovery-code login + single-use, disable, wrong-current-password rejection
  on setup/disable/regenerate, rate limiting, CSRF rejection, oversized-body rejection,
  subscriber-login non-interference, admin-initiated lockout recovery), 12 pre-existing —
  including the full login/session/CSRF/rate-limit suite from the original auth hardening pass,
  confirming zero regressions from the `login_post` refactor.
- Entire workspace compiles; no whitespace errors.

Manual, against the live dev server (not automatable — the one thing that proves interop with a
real authenticator app rather than just this code's generator matching its own verifier):
scanned/decoded the actual rendered QR's `otpauth://` URI, computed a code from it, and drove the
complete enrollment → logout → login-with-code → login-with-recovery-code →
recovery-code-single-use-rejected flow end-to-end via the running server. All behaved as expected.

### A correctness fix found while writing the tests

`confirm_enrollment` originally recorded the confirmation code's time-step into the same
`last_used_step` field the login-time replay guard reads. Since enrollment confirmation never
establishes a login session, there was nothing to protect by blocking replay of *that* action —
but doing so anyway meant the very next real login would be spuriously rejected as a "replay" if
it landed in the same 30-second window as setup (a realistic case: confirming setup and then
immediately logging in). Fixed by leaving `last_used_step` untouched during enrollment
confirmation; the login-time replay guard is unaffected.

### 6. Recovery-code download (added 2026-09-11)

The one-time recovery-codes page (`admin/src/pages/profile_mfa.rs::render_recovery_codes`) also
offers a "Download codes" button alongside "Done". Entirely client-side — builds a `Blob` from the
already-rendered codes and triggers a `synapcms-recovery-codes.txt` download via
`URL.createObjectURL` — no server round-trip, no query string or extra request carrying the codes.
Verified live: the embedded JS array matches the on-screen list exactly, and the download fires
correctly.

## Known gaps / deliberately deferred

- Subscriber MFA, org-wide "require MFA for super_admin" enforcement, WebAuthn/passkeys, and the
  broader security-notification-email system (beyond MFA's own three notices) are all unchanged
  from the review's original Phase 3 list — none of them are prerequisites for what shipped here.

## Third-party cost impact

Current added cost: none. No new external account, API, recurring service, or hosted
authentication dependency.
