---
title: Subscriptions
group: feature
updated_by: claude
last_updated: 2026-09-07
---

# Subscriptions

> Last updated: 2026-09-07 | Updated by: claude

## Overview

The subscriptions feature allows visitors to create a `subscriber` role account via `/subscribe`. New subscribers are assigned to the current site via a `site_users` row. As of 2026-09-07, an anonymous submission that matches an *existing* identity no longer attaches that account to the new site at all (previously it silently linked them without proving control of the account) — see the fix note under How It Works. The form includes bot protection via a honeypot field and a human-check checkbox, plus a required Terms of Service checkbox. A live requirements checklist (display name/email/password/human-check, same `.form-note`/`.pw-dot` pattern as the admin New User form) updates as the visitor types. The page cross-links to `/login` ("Already a member? Sign in") and, since 2026-08-05, `/login` itself cross-links back to `/subscribe` ("Join today!") and to the new `/recover` password-recovery flow (documented below).

## How It Works

### Handler (`core/src/handlers/subscribe.rs`)

Two handlers are defined:
- `subscribe_form` (`GET /subscribe`) — renders the signup form (`admin::pages::subscribe::render`) or a success page (`render_success`) if `?subscribed=1` is present. Site resolution comes from the Host header via the `CurrentSite` extractor, so posting to a given site's host automatically scopes the new subscriber to that site — no extra query params or hidden fields required.
- `subscribe_post` (`POST /subscribe`) — validates and processes the signup form.

Validation and processing flow:
1. Honeypot check: `website` field must be empty; non-empty silently redirects to `?subscribed=1` (bots are not told they were caught).
2. `human_check` must be `"on"`.
3. `terms` (ToS agreement) must be `"on"`.
4. `display_name` must be non-empty and satisfy `validate_display_name` (≤60 chars, added 2026-08-05 — previously unbounded, so an arbitrarily long display name could reach the DB and break layout in admin lists/author URLs/email templates). Also enforced client-side via `maxlength="60"` on the field.
5. `email` must be non-empty and contain `@` (lowercased before use).
6. `password` must equal `confirm_password`.
7. `validate_password` enforces the (2026-09-07) password policy — 12–128 Unicode characters, no composition rules — identical to admin user creation.
8. **Fixed 2026-09-07** (see `AUTH_SECURITY_REVIEW.md` / `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`): if the email already exists, anonymous registration no longer touches that identity or its site memberships. Previously, an absent `site_users` row for the current site was silently added — letting a visitor self-enroll someone else's existing account (including a staff identity) into a registration-enabled site without proving control of the email. The response is now identical to a successful new registration regardless of whether the email was new or already registered, preventing account enumeration too. An existing user must sign in first; a verified invitation/"join site" flow is a possible future addition, not yet built.
9. If the email is new: generates a username via `generate_username`, creates the user with `UserRole::Subscriber`, and adds a `site_users` row (skipped only for the nil-UUID fallback used in single-site mode).

### Password Recovery (`/recover`, added 2026-08-05)

A separate handler, `core/src/handlers/recover.rs`, lets a subscriber reset a forgotten password:
- `GET /recover` / `POST /recover` — request form. On POST, looks up the email; if found **and the account's role is `subscriber`**, generates a single-use token (`core/src/models/password_reset.rs`, stored as a SHA-256 hash with a 1-hour expiry, never the raw token), emails a `/recover/{token}` link via `crate::mail::send_for_site` (the site's own Mailgun account, falling back to the install-wide one), and returns the same "check your email" message either way — including for staff accounts and unregistered addresses — so the form can't be used to enumerate which emails exist or which belong to staff. Staff password resets remain CLI-only (`synap user reset-password`) by design.
- `GET /recover/{token}` / `POST /recover/{token}` — shows a "set a new password" form if the token is still valid (unexpired, unused); POST validates the new password with the same `validate_password` rule as everywhere else, consumes the token (marks it used so it can't be replayed), updates the password hash, and redirects to `/login` with a success flash message.

`generate_username` derives a base username by slugifying the display name (e.g. "Steve Miller" → "steve-miller") and now (fixed 2026-08-05) always produces a result that satisfies `validate_username` (8–15 chars, lowercase/digits/hyphens, no leading/trailing hyphen) — previously it had no length enforcement at all, so a short display name like "Bo" produced an invalid 2-char username. Names under 8 chars are padded with hex from a fresh UUID; names over 15 chars (after slugifying) are truncated, with any resulting trailing hyphen stripped. If the base is taken, it tries sequential numeric suffixes (`steve-miller2`, `steve-miller3`, … up to 9999, trimming the base as needed to stay within 15 chars), then falls back to a guaranteed-valid, guaranteed-unique `user{11 hex chars}` (15 chars total). Every candidate is re-validated against `validate_username` before being accepted. Uniqueness is checked via `username_taken`, a simple `SELECT EXISTS` query. Since the username is never shown to or confirmed by the visitor, it must be generated valid rather than relying on them to fix it.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /subscribe | `subscribe::subscribe_form` | Subscription signup form |
| POST | /subscribe | `subscribe::subscribe_post` | Process signup |
| GET | /recover | `recover::request_form` | Password recovery request form |
| POST | /recover | `recover::request_post` | Send recovery email (subscriber accounts only) |
| GET | /recover/{token} | `recover::reset_form` | Set-new-password form, if token valid |
| POST | /recover/{token} | `recover::reset_post` | Consume token, update password |

## Security Notes

Honeypot: bots that auto-fill the hidden `website` field are silently accepted and redirected without storing any data (both `/subscribe` and `/recover` use the same pattern). The human-check checkbox and ToS agreement checkbox are both server-side validated. Password policy is enforced identically to admin user creation (see the Users & Roles doc). As of 2026-09-07, duplicate-email signups no longer attach the existing identity to a new site — see the Critical fix note above; the response is the same success page as a new registration either way, and a `site_users` row is added only when the identity is genuinely new. `/subscribe` and `/recover` also gained rate limiting the same day (`auth_security::allow` — 60 requests/15min per IP, 30/15min per normalized email) — see the Middleware & Auth doc for the full policy.

`/recover` deliberately excludes staff accounts (`super_admin`/`site_admin`/`editor`/`author`) — it silently no-ops for them exactly as it does for an unregistered email, rather than returning a distinct error, so the form can't be used to fingerprint which addresses belong to staff. Relatedly, the public `/login` page's own staff-account branch (tried to sign in as staff via the subscriber-facing form) was changed from a distinct "Staff accounts sign in at /admin/login." message to the same generic "Invalid email or password." used for any other failure — that branch only runs after a *correct* password match, so the distinct message was telling anyone testing valid credentials whether a given login belonged to a staff account.


