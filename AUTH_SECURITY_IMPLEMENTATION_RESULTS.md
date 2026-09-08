# Authentication Security Implementation Results

Date: 2026-09-07

Related review: `AUTH_SECURITY_REVIEW.md`

## Outcome

The highest-priority authentication hardening was implemented using only local Rust application code, PostgreSQL, and the existing server-side session store. No hosted identity provider, paid CAPTCHA, SMS service, Redis service, Clerk account, or Google authentication integration was added.

The implementation covers staff authentication at `/admin/login`, subscriber authentication at `/login`, public registration, password recovery, account/admin sessions, password policy, logout, tenant role isolation, and email identity normalization.

## Implemented changes

### 1. Closed anonymous existing-account linking

Changed `core/src/handlers/subscribe.rs`.

Previously, submitting an existing email to `/subscribe` could immediately add that existing identity to the current site without proving control of the account.

Now:

- Anonymous registration never modifies an existing identity or its site memberships.
- The response remains the same as the successful-registration response, avoiding account enumeration.
- New identities can still register normally.

Product effect: an existing user cannot self-enroll in an additional site until a verified invitation or authenticated “join site” flow is implemented. This is an intentional safe default.

### 2. Closed the site-subscriber admin capability gap

Changed `core/src/middleware/admin_auth.rs`.

Now:

- `can_manage_content` is granted only to author-or-higher site roles.
- A site-level subscriber receives no admin capabilities.
- A non-global user whose current site role is missing or `Subscriber` is denied access to the admin area.
- Added role/capability unit tests for subscriber and author boundaries.

### 3. Added same-origin CSRF enforcement

Added `core/src/middleware/csrf.rs` and connected it in `core/src/router.rs`.

State-changing requests under these paths now require an exact `Origin` and `Host` match:

- `/admin...`
- `/account...`
- `/login`
- `/subscribe`
- `/recover...`

The middleware also honors Caddy's `X-Forwarded-Proto` when deciding whether the expected external origin is HTTP or HTTPS. Exact origin matching blocks ordinary cross-site attacks and attacks from sibling subdomains that may still be considered “same-site” by cookie rules.

Public content forms such as `/form/{name}` were intentionally not included, because they are unauthenticated submission endpoints and may have separate embedding requirements.

Operational effect: non-browser scripts posting to protected/authentication routes must send a correct `Origin` header. Normal modern browser form submissions and same-origin JavaScript requests already do this.

### 4. Added session fixation protection

Changed `core/src/handlers/auth.rs`.

Both admin and subscriber login now call the session library's `cycle_id()` before establishing authentication. The old server-side session ID is deleted and a new random ID is issued.

### 5. Added local authentication abuse limiting

Added `core/src/middleware/auth_security.rs` and checks in login, registration, and recovery handlers.

Protected flows:

- Admin login
- Subscriber login
- Registration
- Password recovery requests

Policy:

- Fifteen-minute rolling window.
- Up to 60 requests per source-IP bucket.
- Up to 30 requests per normalized-identity bucket.
- A hard 10,000-bucket memory bound with stale-bucket cleanup.

This implementation is in-process and costs nothing. It is appropriate for the current single-application-instance architecture. Before horizontally scaling to multiple app instances, move the counters to PostgreSQL or an already-available shared cache so limits apply across instances.

The limits are deliberately moderate to reduce denial-of-service risk from intentional account lockout attempts. They can be tuned after observing production traffic.

### 6. Added password-change session revocation

Changed:

- `core/src/models/user.rs`
- `core/src/middleware/admin_auth.rs`
- `core/src/middleware/account_auth.rs`
- `core/src/handlers/auth.rs`
- `core/src/handlers/account.rs`
- `core/src/handlers/admin/profile.rs`

At login, the session stores an opaque SHA-256 marker derived from the current Argon2 password hash. Every authenticated request compares its session marker with the current user record.

Results:

- Changing or recovering a password changes the Argon2 hash and invalidates every older session.
- The session used to perform an authenticated password change is updated to the new marker, so the current user can continue normally.
- Other browsers/devices are signed out automatically.
- The password hash itself is not copied into session data.
- Suspension and deletion continue to take effect immediately because the user is reloaded on every authenticated request.

### 7. Added absolute session lifetimes

Changed both authentication middleware modules and login handlers.

Limits now combine rolling inactivity expiration with a fixed maximum lifetime:

- Admin: two-hour inactivity timeout and 24-hour absolute lifetime.
- Subscriber account: 24-hour inactivity timeout and seven-day absolute lifetime.

This prevents an actively used session from remaining valid forever.

### 8. Fixed the post-login open redirect

Changed `core/src/handlers/auth.rs`.

Login redirects now require an internal path that:

- Begins with exactly one `/`.
- Does not begin with `//`.
- Contains no backslashes.
- Contains no control characters.

Added unit tests covering valid internal paths, absolute external URLs, scheme-relative URLs, backslash tricks, and header-injection characters.

### 9. Reduced account-enumeration timing differences

Changed `core/src/models/user.rs` and both login handlers.

When an email does not exist, the server now performs a dummy Argon2 verification before returning the generic invalid-credentials response. Oversized credentials are also bounded before normal processing.

### 10. Modernized the password policy

Changed the core model, admin UI, subscriber UI, bundled themes, CLI user management, and installer.

New policy:

- Minimum 12 Unicode characters.
- Maximum 128 Unicode characters.
- Spaces and passphrases are supported.
- No mandatory uppercase, number, or symbol composition rules.
- Argon2 remains the password hashing algorithm.
- Generated local passwords are now 16 characters instead of eight.

Existing passwords are not invalidated merely because they are shorter. The new policy applies when creating or changing a password.

### 11. Normalized email identity

Changed `core/src/models/user.rs` and added `migrations/0070_normalize_user_email.sql`.

Now:

- Email input is trimmed and lowercased in a central model helper.
- User creation and update store normalized email.
- Email lookup is case-insensitive.
- PostgreSQL normalizes existing rows during migration.
- A unique index on `lower(email)` enforces case-insensitive identity uniqueness.

Migration safety: if historical data contains case-only duplicate addresses, the migration intentionally fails instead of guessing which identity should win. Resolve those duplicate users before rerunning the migration.

### 12. Made password recovery transactional and single-current-token

Changed `core/src/models/password_reset.rs` and `core/src/handlers/recover.rs`.

Now:

- Issuing a reset token removes the user's previous unused reset tokens.
- Consuming the token and updating the password happen in one PostgreSQL transaction.
- A database failure rolls back token consumption instead of burning a valid token without changing the password.
- A successful reset automatically invalidates older sessions through the credential marker.

### 13. Enabled Secure cookies outside development mode

Changed `core/src/main.rs` and `synaptic.toml.example`.

Now:

- `dev_mode = false` marks admin and subscriber session cookies `Secure`.
- `dev_mode = true` permits local plain-HTTP development cookies.

Axum receiving internal HTTP from Caddy does not prevent the external browser cookie from carrying `Secure`; the browser sees the public HTTPS connection.

Operational effect: a direct plain-HTTP deployment must explicitly use development mode, while production remains secure by default.

### 14. Changed logout to protected POST

Changed routes, generated admin/account navigation, and all bundled active themes.

Now:

- `/admin/logout` accepts POST rather than GET.
- `/account/logout` accepts POST rather than GET.
- Logout requests receive same-origin CSRF validation.
- Sessions continue to use full `flush()` behavior.

Custom-theme effect: a custom theme containing `<a href="/account/logout">` must change that link to a small POST form. Bundled themes have already been updated.

### 15. Disabled unsafe self-service email changes

Changed account/admin profile handlers and screens.

Self-service email fields are now read-only and server handlers ignore a submitted replacement address. This avoids allowing an authentication identifier to change without password confirmation and verification of the new address.

Administrative user-management flows remain available. A future self-service implementation should use a pending address, a short-lived verification token, current-password confirmation, and notification to the old address.

## Verification results

Commands run:

```text
cargo test -p synaptic-core --lib
cargo test -p synap-cli
cargo check --workspace
git diff --check
```

Results:

- Synaptic core: 120 tests passed; 0 failed.
- CLI: 4 tests passed; 0 failed.
- Entire Rust workspace compiled successfully.
- No whitespace errors were reported by `git diff --check`.
- Five focused tests were added or updated for role capabilities, redirect validation, rate limiting, CSRF route selection, and the new password policy.
- An additional CLI regression test generates 100 installer passwords and verifies that every result is 16 characters and satisfies the current password policy.

`cargo fmt --all -- --check` was also inspected. It reports extensive pre-existing formatting differences across the repository, including unrelated files. No bulk repository-wide formatting rewrite was performed, to avoid mixing unrelated changes into this security work.

## Deployment and review checklist

Before deploying:

1. Back up PostgreSQL.
2. Check for case-insensitive duplicate emails:

   ```sql
   SELECT lower(trim(email)), count(*)
   FROM users
   GROUP BY lower(trim(email))
   HAVING count(*) > 1;
   ```

3. Resolve any rows returned before applying migration 0070.
4. Confirm production uses `dev_mode = false` and public HTTPS through Caddy.
5. Update any non-bundled/custom theme logout links to POST forms.
6. Expect all existing admin and subscriber sessions to require login again. Existing sessions lack the new credential and login-time markers and are intentionally rejected.
7. If scripts POST to auth/admin/account routes, add the correct external `Origin` header.
8. Manually smoke-test `/admin/login`, `/login`, `/subscribe`, `/recover`, password change, logout, and at least one mutation in each admin role.

## Intentionally deferred items

These are no-cost or optional capabilities, but were not folded into this change because they need additional product/UI decisions or broader infrastructure:

### Verified self-service email changes

The unsafe path is closed by making email read-only. A complete replacement requires a new pending-email table, confirmation email templates, expiration behavior, and decisions about users who lose access to their old address.

### Verified cross-site invitations/joining

Anonymous existing-account attachment is now blocked. A future flow should require either an authenticated user action or a one-time email invitation.

### Database-level multi-instance rate limits

The local limiter protects one running process. Shared counters are required only if the service is later run as multiple concurrent application instances.

### TOTP, recovery codes, and passkeys

These can be implemented locally without a paid identity provider, but account recovery and support policy should be designed first. They are not prerequisites for correcting the vulnerabilities addressed here.

### Common/breached-password screening

A local denylist can be added without an online service. It should be selected and versioned deliberately because the list affects user experience and application package size.

### Full HTTP integration suite

The existing `core/tests/routes.rs` authentication tests remain ignored `todo!()` placeholders requiring a live PostgreSQL test harness and seeded application state. Unit coverage was added, and the workspace compiles, but browser-level and database-backed route tests remain recommended before a production release.

## Third-party cost impact

Current added cost: none.

No new external account, API, recurring service, or hosted authentication dependency is required by this implementation.
