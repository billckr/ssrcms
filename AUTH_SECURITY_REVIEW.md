# Authentication Security Review

Date: 2026-09-07

Scope: local authentication for staff at `/admin/login`, subscribers at `/login`, public registration, password recovery, sessions, and site authorization.

This document records review findings for discussion. It does not mean that a paid identity provider is required. All urgent and near-term recommendations can be implemented locally in the Rust application with the existing PostgreSQL-backed authentication system.

## Executive summary

The existing system has a solid base: Argon2 password hashing, random salts, server-side sessions, separate admin and subscriber session cookies, active-account checks, site-scoped roles, hashed reset tokens, and audit logging.

Before production, the highest priorities are:

1. Prevent public signup from attaching an existing identity to a site without verification.
2. Correct the capability calculation that grants content management to a site-level subscriber.
3. Add CSRF protection to authenticated mutations.
4. Rotate session IDs after login.
5. Rate-limit login, registration, and password recovery.
6. Revoke existing sessions after password resets and sensitive account changes.

These are local application changes and do not require a third-party service.

## Confirmed findings

### 1. Critical: unverified cross-site account linking

`POST /subscribe` looks up an existing email and, when that identity is not already a member of the current site, immediately adds a subscriber membership. It does not verify the submitted password or prove control of the email address.

Location: `core/src/handlers/subscribe.rs`, around lines 112–136.

Risks:

- A visitor can enroll another person's identity in a site.
- Existing staff identities can be attached to registration-enabled tenants.
- Membership and consent records become unreliable.
- This combines dangerously with the capability issue below.

Recommended local fix:

- Never attach an existing user during anonymous registration.
- Ask the user to sign in first, or send a short-lived email confirmation link.
- Return the same public response whether the email is new or existing to avoid account enumeration.
- Perform user creation and site membership creation in one database transaction.

### 2. Critical: subscriber site role receives content-management capability

`AdminCaps::from_roles` currently assigns `can_manage_content: true` without checking the site role.

Location: `core/src/middleware/admin_auth.rs`, around line 107.

An existing staff identity attached to another site as a subscriber may consequently receive content-management capability there.

Recommended local fix:

- Explicitly map every site role to capabilities.
- A subscriber must receive no admin capabilities.
- Deny admin access when the resolved site role is `Subscriber` or absent.
- Add table-driven tests for every global-role/site-role combination.

### 3. High: no explicit CSRF protection

No CSRF token or Origin-checking middleware was found. Admin and account areas expose many cookie-authenticated POST routes, including destructive operations.

Location: route declarations in `core/src/router.rs`.

`SameSite=Lax` is useful defense in depth but is not a complete CSRF boundary, especially where sibling subdomains or compromised tenant origins are possible.

Recommended local fix:

- Generate a random CSRF token for each session.
- Include it in every authenticated HTML form and relevant JavaScript request.
- Verify it on POST, PUT, PATCH, and DELETE requests.
- Validate the `Origin` header as an additional check.
- Keep `SameSite=Lax` unless a stricter setting is compatible with intended flows.

No external service is needed.

### 4. High: session identifier is not rotated after login

Both login handlers store a user ID in the existing session without cycling the session identifier.

Locations:

- `core/src/handlers/auth.rs`, around line 204 for admin login.
- `core/src/handlers/auth.rs`, around line 299 for subscriber login.

Recommended local fix:

- Rotate the server-side session ID immediately before establishing authentication.
- Rotate it again after privilege changes.
- Clear stale authentication and role keys before writing the new identity.

### 5. High: no rate limiting for authentication flows

No throttling was found for:

- `/admin/login`
- `/login`
- `/subscribe`
- `/recover`

The registration and recovery honeypots only address simple bots. They do not stop credential stuffing, brute force, or email flooding.

Recommended local fix:

- Add in-process or PostgreSQL-backed token buckets keyed by IP and normalized account identifier.
- Apply stricter limits to admin login.
- Use short escalating delays after repeated failures.
- Avoid permanent account lockouts, which can be abused for denial of service.
- Add CAPTCHA later only when traffic or abuse warrants it.

This can be implemented without a paid service. A single-instance deployment can start with an in-memory limiter; multiple application instances should use a shared store such as PostgreSQL or an existing Redis deployment.

### 6. High: password and security changes do not revoke sessions

Password changes and password recovery update the password hash but leave existing sessions active.

Locations:

- `core/src/handlers/account.rs`, around line 94.
- `core/src/handlers/admin/profile.rs`, around line 95.
- `core/src/handlers/recover.rs`, around line 122.

Recommended local fix:

- Add `session_version` or `credentials_changed_at` to `users`.
- Copy that value into a session at login and compare it on each authenticated request.
- Increment it after password reset, password change, suspension, sensitive role changes, and account recovery.
- Offer a “sign out all devices” action.

### 7. Medium-high: post-login open redirect

The subscriber login accepts a redirect whenever it starts with `/`. A value beginning with `//` can be interpreted as a scheme-relative external URL.

Location: `core/src/handlers/auth.rs`, around lines 304–308.

Recommended local fix:

- Accept only an application-relative path beginning with exactly one `/`.
- Reject `//`, backslashes, control characters, and malformed paths.
- Prefer named destinations when practical.

### 8. Medium: password policy discourages strong passwords

Passwords must be 8–12 characters and contain prescribed character classes.

Location: `core/src/models/user.rs`, around lines 62–84.

The 12-character maximum prevents strong passphrases and many password-manager-generated credentials.

Recommended local fix:

- Permit at least 64 characters.
- Require a sensible minimum, such as 12 characters for staff and 10–12 for subscribers.
- Permit spaces and Unicode.
- Remove mandatory uppercase/digit/symbol composition rules.
- Optionally check passwords against a locally stored common-password list.
- Continue using Argon2.

### 9. Medium: production cookies lack the `Secure` attribute

Both session layers use `with_secure(false)` because Axum receives HTTP behind Caddy.

Location: `core/src/main.rs`, around lines 87–110.

Cookie security is determined from the browser's connection, not the internal Caddy-to-Axum connection. Production cookies should therefore still carry `Secure`.

Recommended local fix:

- Make secure-cookie behavior configurable.
- Default it to enabled in production.
- Disable it only for explicit local HTTP development.
- Confirm `HttpOnly`, host-only scope, and root path behavior in integration tests.

### 10. Medium: logout is a GET request

Admin and subscriber logout endpoints use GET, allowing forced logout through cross-site navigation.

Locations: `core/src/router.rs`, around lines 123 and 152.

Recommended local fix:

- Change logout to a CSRF-protected POST.
- Continue flushing the complete server-side session.

### 11. Medium: account lookup timing can reveal registered emails

An unknown email avoids Argon2 verification, while a known email performs expensive password verification. Public error messages are generic, but response timing may still reveal whether an account exists.

Recommended local fix:

- Perform a dummy Argon2 verification when an email lookup fails.
- Normalize email before every lookup.

### 12. Medium: recovery controls need strengthening

Positive properties already present:

- Reset tokens are random.
- Only token hashes are stored.
- Tokens expire after one hour.
- Tokens are single-use.
- Recovery responses do not directly reveal account existence.

Remaining issues:

- Unlimited active tokens may be created for one user.
- Recovery requests are not rate-limited.
- The token is consumed before the password update succeeds.
- Successful recovery does not revoke sessions.

Recommended local fix:

- Invalidate previous reset tokens when issuing a new one.
- Rate-limit by IP and normalized email.
- Update the password and consume the token in one database transaction.
- Revoke existing sessions after success.
- Do not place reset tokens in application logs or analytics.

### 13. Medium: email changes lack verification and recent authentication

Authenticated users can update their email without confirming their password or verifying the new address.

Recommended local fix:

- Require the current password for local-auth email changes.
- Store a pending email address and send a short-lived confirmation link.
- Notify the old address after completion.
- Rotate or revoke sessions after the change.

### 14. Medium: email normalization is inconsistent

Signup trims and lowercases emails, while login lookup does not consistently do so. The database's email uniqueness constraint is case-sensitive.

Locations:

- `core/src/handlers/subscribe.rs`, around line 101.
- `core/src/models/user.rs`, around line 384.
- `migrations/0001_create_users.sql`, line 7.

Recommended local fix:

- Normalize email in one model-layer function used by all callers.
- Add a unique database index on `lower(email)` after cleaning existing collisions.
- Keep the original presentation form separately only if it is needed for display.

### 15. Medium-low: admin sessions have no absolute lifetime

Admin sessions expire after two hours of inactivity, and subscriber sessions after 24 hours of inactivity. Because expiry rolls on every request, an actively used session can continue indefinitely.

Recommended local fix:

- Keep the inactivity timeout.
- Also store login time and enforce an absolute lifetime, such as 12–24 hours for staff.
- Require recent authentication for highly sensitive actions.

### 16. Testing gap

The current 115 library unit tests pass, but authentication route tests in `core/tests/routes.rs` are ignored `todo!()` placeholders.

Important missing integration tests include:

- Successful and unsuccessful login.
- Session rotation after login.
- Cookie flags and expiry.
- Tenant isolation and every role/capability combination.
- CSRF rejection and acceptance.
- Safe redirect validation.
- Password-reset replay and concurrency.
- Session invalidation after password changes.
- Existing-email registration behavior.
- Suspended and deleted account behavior.

## Existing strengths to preserve

- Argon2 password hashing with unique random salts.
- PostgreSQL-backed server-side sessions.
- Separate session cookies and timeouts for staff and subscribers.
- Active/deleted status is checked during authenticated requests.
- Site roles are generally re-read and validated.
- Reset tokens are stored hashed rather than in plaintext.
- Login errors are mostly generic.
- Staff login auditing exists.
- Protected pages use `Cache-Control: no-store`.
- Caddy supplies HSTS, MIME-sniffing protection, frame protection, and a referrer policy.

## No-cost implementation roadmap

### Phase 1: production blockers

- Fix unverified existing-account linking.
- Fix subscriber capability mapping and deny subscribers access to admin handlers.
- Add CSRF protection.
- Rotate session IDs after login.
- Fix redirect validation.
- Enable secure cookies in production.
- Add basic login and recovery rate limiting.
- Add integration tests for these boundaries.

### Phase 2: account hardening

- Replace the 12-character password maximum.
- Normalize emails and enforce case-insensitive uniqueness.
- Revoke sessions after password/security changes.
- Make reset issuance and consumption transactional.
- Change logout to POST.
- Add verified email changes and “sign out all devices.”
- Add absolute staff-session lifetime and recent-authentication checks.

### Phase 3: optional advanced local features

- TOTP authenticator-app MFA for staff.
- Recovery codes stored as hashes.
- WebAuthn/passkeys using a Rust WebAuthn library.
- Device/session management.
- Security notification emails.

TOTP and passkeys do not inherently require a paid provider. They do add implementation and recovery complexity, so they should follow the production blockers.

## Preparing for optional Google, Clerk, or another provider

External providers should remain optional. Their job is to establish identity; SynapCMS should remain authoritative for sites, roles, and capabilities.

A future-friendly local data model could add an `auth_identities` table containing:

- Internal user ID.
- Provider (`local`, `google`, `clerk`, `webauthn`, and so on).
- Provider subject identifier.
- Provider email and verification state where applicable.
- Created and last-used timestamps.

The current local password hash can later become one authentication method among several. This avoids coupling tenant authorization to any vendor and permits local authentication to remain available.

## Third-party cost assessment

No paid service is required for the critical fixes, CSRF protection, rate limiting, session invalidation, stronger password handling, email normalization, TOTP, recovery codes, or passkeys.

Features that may involve external cost include:

- Hosted identity platforms such as Clerk beyond their free allowance.
- Transactional email delivery at meaningful volume.
- Managed CAPTCHA or bot-detection products beyond free allowances.
- Managed Redis or another shared rate-limit store if the deployment grows beyond one application instance.
- SMS-based MFA, which is not recommended as the first MFA option anyway.

The recommended near-term direction is to harden the existing local authentication system and design clean extension points for external identity providers later.
