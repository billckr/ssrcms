---
title: Account Area
group: feature
updated_by: claude
last_updated: 2026-09-08
---

# Account Area

> Last updated: 2026-09-08 | Updated by: claude

## Overview

The account area (`/account/*`) is for authenticated subscribers (any logged-in role via the `AccountUser` extractor). It provides a dashboard, profile management, a saved posts reading list, and comment history. It is rendered entirely in Rust (`admin/src/pages/account.rs`), never through a site's Tera theme — because it handles authenticated user data (profile, passwords), a site admin cannot modify these templates.

## How It Works

### Dashboard

`GET /account` — the default landing page. Renders a welcome message with the subscriber's display name.

### Profile Management

`GET /account/profile` shows the profile view. The "Edit Profile" dialog covers **Display Name** and **Bio** only — email is shown there as static text, not an editable field, and `POST /account/profile/update` still ignores any submitted `email` value, building an `UpdateUser` that leaves `username`, `email`, `password_hash`, and `role` untouched. On success it shows the flash message "Profile updated!"; on failure, "Error saving profile. Please try again."

Email has its own **verified self-service change flow** (added 2026-09-08, subscriber-only for now — closes the "email changes require an admin" gap from the 2026-09-07 interim fix), in a separate "Change Email" dialog. See "Verified Email Change" below for the full flow.

`POST /account/profile/change-password` requires `current_password`, `new_password`, `confirm_password`. It verifies the new/confirm passwords match, verifies the current password via `account.user.verify_password`, validates the new password with `validate_password` (12–128 Unicode characters, no composition rules, updated 2026-09-07), and hashes it with `hash_password` before updating. A successful change also writes a fresh credential-version marker into the *current* session (so it keeps working) while every other active session for the account is invalidated on its next request — see the Middleware & Auth doc. Flash: "Password changed successfully!" or an error string.

**Sign Out Other Devices (2026-09-08):** a "Sign out other devices" icon button (next to Change Email) posts to `POST /account/profile/sign-out-other-devices` with no form fields — just a JS `confirm()` before submitting. It calls `user::regenerate_session_nonce`, which assigns the account a fresh random `session_nonce` (new `users` column, migration 0002), then writes the resulting `credential_version()` into the *current* session the same way password/email changes do, so the click doesn't log the clicking device out too. Every other active session — anywhere else this account is signed in — fails its next credential-version check and is redirected to sign in again. Unlike a password or email change, this needs no re-authentication step first: it doesn't touch any recovery-sensitive field, so there's nothing to prove ownership of beyond the session cookie itself. Flash: "Signed out of every other session." See the Middleware & Auth doc for how `credential_version()` folds in `session_nonce`.

### Saved Posts

`GET /account/saved-posts` — paginated (20/page) list of posts the subscriber has saved, with an optional `search` query param and a `partial=1` mode that returns only the inner list fragment (used by the live-search JS). Each row shows a view link to the post URL and an unsave form. `derive_unsave_url()` strips the scheme/host from the stored absolute post URL and appends `/unsave`, pointing at the public `POST /{slug}/unsave` route (`core/src/handlers/post.rs`), which removes the post from the subscriber's reading list.

### My Comments

`GET /account/my-comments` — paginated (20/page) list of comments the subscriber has posted, with the same `search` and `partial=1` live-search pattern. View icon links to `/{slug}#comments`. `POST /account/comments/{id}/delete` soft-deletes the subscriber's own comment, but only if: the comment belongs to them (`is_owner`), it was created within the last 15 minutes (`within_window`), and it isn't already deleted.

Both Saved Posts and My Comments use small inline JS (`crate::live_search_script`) for progressive enhancement: 300ms-debounced `fetch()` calls that swap the list `<div>` without a full page reload. This is an intentional placeholder — when the account pages are ported to Leptos, the JS is meant to be replaced with reactive signals/server functions.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /account | `account::dashboard` | Dashboard |
| GET | /account/profile | `account::profile_view` | View profile |
| POST | /account/profile/update | `account::profile_update` | Update profile |
| POST | /account/profile/change-password | `account::profile_change_password` | Change password |
| POST | /account/profile/sign-out-other-devices | `account::sign_out_other_devices` | Invalidate every other session for this account (added 2026-09-08) |
| POST | /account/email/change | `account_email::request_change` | Request a verified email change (added 2026-09-08) |
| GET/POST | /account/email/confirm/{token} | `account_email::confirm_form` / `confirm_post` | Confirm an email change (added 2026-09-08, unauthenticated — see below) |
| GET | /account/saved-posts | `account::saved_posts` | Saved posts list (supports `?search=`, `?page=`, `?partial=1`) |
| GET | /account/my-comments | `account::my_comments` | Comment history (supports `?search=`, `?page=`, `?partial=1`) |
| POST | /account/comments/{id}/delete | `account::delete_comment` | Delete own comment (15-minute window) |
| POST | /account/logout | `auth::account_logout` | Log out (changed from GET to a CSRF-protected POST 2026-09-07 — GET allowed forced logout via cross-site navigation) |

## Security Notes

- All routes require an active account session via the `AccountUser` extractor.
- Comment deletion is scoped to the authenticated user's own comments and time-limited to 15 minutes after posting.
- Password changes require the current password to be re-verified server-side before a new hash is written.
- Sign Out Other Devices (2026-09-08) requires no password re-entry — it's a same-origin POST from an already-authenticated session, not a change to any recovery-sensitive field.
- All state-changing routes here require an exact `Origin`/`Host` match as of 2026-09-07 (`csrf::same_origin`) — see the Middleware & Auth doc.
- Email changes (added 2026-09-08) also require the current password re-verified server-side, are rate-limited (`auth_security::allow("email-change", ...)`), and only take effect once the emailed confirmation link is clicked — the request step alone never touches `users.email`. `/account/email/confirm/{token}` is necessarily unauthenticated (the link may be opened on a different device), but that POST still passes through the same `csrf::same_origin` check as every other route under `/account*`.


### Sign-in Redirects

As of 2026-09-07, a subscriber who signs in directly at /login is redirected to the site home page (/). When an unauthenticated visitor requests a protected /account path, the account guard sends that internal path through the login form and returns the visitor there after successful authentication. Redirect destinations must be absolute local paths; protocol-relative URLs, external URLs, backslashes, and control characters are rejected. Staff sign-in at /admin/login continues to redirect to /admin.


### Authentication Edge-Case Hardening (2026-09-07)

Commenting and saved-post changes now require the same fully valid, active, current-site account session as the protected account area. Saved-post return paths reject external and scheme-relative redirects. The public login page accepts only fixed named notices rather than arbitrary flash text. Registration performs equivalent password-hashing work for existing addresses, and recovery token/email work runs off the generic response path to reduce account-enumeration timing differences. See AUTH_EXPLOIT_REVIEW_RESULTS.md.


### Verified Email Change (2026-09-08)

Closes the last item from `AUTH_SECURITY_REVIEW.md`'s email-change finding — subscribers can now change their own sign-in email, but only through a verified request/confirm flow, not a direct field edit. New handler module `core/src/handlers/account_email.rs`, new model `core/src/models/email_change.rs`, new table `email_changes` (migration 0071 — see the Database Schema doc). Deliberately **subscriber-only** for now: `/admin/profile` (staff self-service) stays read-only, since sending mail requires a concrete `site_id` and a staff `AdminUser.site_id` can be `None` in single-site fallback mode, and no admin handler currently sends mail at all.

Flow, closely mirroring `/recover`'s password-reset flow:

1. `POST /account/email/change` (requires `AccountUser` + the current password re-entered — email is a recovery identity, same sensitivity bar as changing a password) validates and normalizes the new address, rejects it if unchanged or already in use by another account (an authenticated, rate-limited context, so revealing "already in use" here isn't the account-enumeration concern it would be on anonymous `/subscribe`), then rate-limits via `auth_security::allow("email-change", ...)` and — off the response path via `tokio::spawn`, so mail-provider latency doesn't affect it — creates an `email_changes` row (a hashed, 60-minute, single-use token, same shape as `password_resets`) and emails the raw token link to the **new** address.
2. `GET /account/email/confirm/{token}` renders a "confirm this change?" page without consuming the token. This is deliberate, unlike a plain lookup-and-render: corporate mail scanners (e.g. Outlook Safe Links) auto-`GET` links in inbound mail, which would silently burn a consume-on-GET token before the real user ever opens it.
3. `POST /account/email/confirm/{token}` (unauthenticated — the link may be opened on a different device than the one that requested the change) atomically consumes the token and commits the new address to `users.email` in one transaction (`email_change::consume_and_apply`), then:
   - If the browser making this POST holds the session that belongs to the account being changed, that session's stored credential-version marker is refreshed in the same request (so it keeps working) and the response redirects to `/account/profile` with a success flash.
   - Otherwise it redirects to `/login?notice=email-changed` ("Email changed. Please sign in again.").
   - Either way, a background task emails a "your email was changed" notice to the **old** address.
4. Because `User::credential_version()` now hashes `email` as well as `password_hash` (see the Middleware & Auth doc), every *other* active session for the account — any device that wasn't the one completing the confirmation — is invalidated on its next request, the same way a password change already invalidates other sessions.

`GET /account/profile` shows a "change to `{new_email}` pending, expires `{time}`" banner (`ProfileData.pending_email_change`, via `email_change::find_pending_for_user`) whenever a request is outstanding. Requesting a new change supersedes any prior unconsumed one — `email_change::create` deletes it first, same single-active-token invariant as `password_resets`. There's no separate "cancel" action in this first pass; letting the old token expire (or requesting a new change) is the only way to abandon one. GDPR erasure (`user::erase_personal_data`) now also deletes any pending `email_changes` row for the account — see the Users & Roles doc's GDPR erasure section.
