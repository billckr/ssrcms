---
title: Users & Roles
group: feature
updated_by: claude
last_updated: 2026-09-08
---

# Users & Roles

> Last updated: 2026-09-08 | Updated by: claude

## Overview

Users are stored in the `users` table. Five global roles are defined: `subscriber`, `author`, `editor`, `site_admin`, and `super_admin`. Site-specific access is stored separately in the `site_users` table using a distinct, smaller set of site-scoped roles: `admin`, `editor`, `author`, `subscriber`. As of 2026-08-18 a user can hold **more than one** of these site roles on the same site simultaneously (e.g. both `editor` and `author` on the same site) — see "Multiple roles per user per site" below. `super_admin`/`site_admin` are global-only concepts and can never appear as a `site_users.role` value, enforced both by the `site_users.role` CHECK constraint and, independently, at the Rust type level (see below). Users can be soft-deleted (content preserved) or hard-deleted (content optionally reassigned).

## How It Works

### Model (`core/src/models/user.rs`)

Key structs:
- `User` — full DB row: `id`, `username`, `email`, `display_name`, `password_hash` (Argon2, never serialized), `bio`, `avatar_media_id`, `role`, `is_active`, `is_protected`, `deleted_at`, `default_site_id`
- `UserContext` — template-safe view: `id`, `username`, `display_name`, `bio`, `role`, `url` (`{base_url}/author/{username}`)
- `UserRole` enum with variants: `Subscriber`, `Author`, `Editor`, `SiteAdmin`, `SuperAdmin`
- `CreateUser`, `UpdateUser` — mutation structs

Password requirements (enforced by `validate_password`, updated 2026-09-07): 12–128 Unicode characters, no mandatory composition rules — the previous 8–12 character range requiring an uppercase letter, a digit, and a symbol from `!@#$%&` is gone, since it blocked passphrases and most password-manager-generated credentials while adding little strength on top of Argon2. Existing shorter passwords are not invalidated; the new rule applies the next time a password is set. Email is normalized (`user::normalize_email` — trim + lowercase) on every create/update/lookup, and migration 0070 adds a `lower(email)` unique index enforcing case-insensitive uniqueness in Postgres too — see the Database Schema doc. Passwords are hashed with Argon2 via `hash_password`. Verification via `User::verify_password`.

Key functions: `create`, `get_by_id`, `get_by_id_include_inactive` (added 2026-08-05 — see Suspend/Reactivate below), `get_by_username`, `get_by_username_include_inactive` (added 2026-08-06), `get_by_email`, `update`, `soft_delete`, `delete`, `delete_and_reassign`, `list`, `list_all` (added 2026-08-05), `deactivate`, `reactivate` (added 2026-08-05), `count`, `count_for_site`, `count_global_admins`, `set_default_site`, `hash_password`, `verify_password`.

`get_by_id`, `get_by_username`, and `get_by_email` all exclude soft-deleted **and suspended** users (`deleted_at IS NULL AND is_active = TRUE`) — this is what actually blocks a suspended account from logging in anywhere (admin, public `/login`, `/account`), since every login path resolves the user through one of these.

The initial 2026-08-05 Suspend/Reactivate rollout only swapped `get_by_id` for `get_by_id_include_inactive` in the couple of places suspension itself needed (editing the admin profile, the suspend/reactivate handlers' own guard checks). It missed that the same strict, active-only lookups were also used to fetch a **post's author** for rendering — not just for login. Suspending an author broke, site-wide, every page that rendered one of their posts: the public home page and single-post/page views (`build_post_context` in `handlers/home.rs`), the "recent posts" widget's per-post author lookup and its `author=username` filter (`templates/functions.rs`), the public author-archive page `/author/{username}` (`handlers/archive.rs`), and the admin posts list/edit author display (`handlers/admin/posts.rs`, which already degraded to "Unknown" rather than 404ing, but still lost the name). Fixed 2026-08-06 by switching all of these to `get_by_id_include_inactive` / the new `get_by_username_include_inactive`, and by changing the `recent_posts` author-filter query directly (it queries `users` inline rather than through a model function) from `is_active = TRUE` to `deleted_at IS NULL`. The admin post-edit page also now shows a red "Suspended" badge next to the author's name (reusing the badge markup from the Users list) when the author is suspended, mirroring how the Users list already flags them.

Rule of thumb going forward: suspension must only ever gate **login/session resolution**, never the visibility of already-published content. Any new lookup of a post/page's author (or any other suspend-able user acting purely as an attribution/display value rather than an authenticating principal) should use the `_include_inactive` variant, not the strict one.

### Site User Model (`core/src/models/site_user.rs`)

`SiteUser` struct: `id` (surrogate PK, added 2026-08-18), `site_id`, `user_id`, `role`, `invited_by`, `created_at`.

`SiteRole` enum (added 2026-08-18): `Admin | Editor | Author | Subscriber` — the site-scoped role type. Deliberately has **no** variant for `SuperAdmin`/`SiteAdmin`; there is no `From<UserRole> for SiteRole` conversion either. This means any function typed to take a `SiteRole` (rather than a raw `&str`) cannot compile if handed a global role — the multi-role work leaned on this as defense-in-depth on top of the pre-existing `site_users.role` CHECK constraint, specifically so a bug can never let a global-only role be assigned or session-pinned as a per-site role.

Key functions: `add(pool, site_id, user_id, role: SiteRole, invited_by)` — idempotent per-role insert (as of 2026-08-18; previously an upsert that overwrote any existing role — see "Multiple roles per user per site" below), `remove` (removes ALL of a user's roles on a site), `remove_role` (added 2026-08-18, removes just one), `has_any_role` (added 2026-08-18 — pure access check, replaces the old `get_role` for "is this user allowed on this site" gates), `list_roles_for_user_and_site` (added 2026-08-18, returns `Vec<SiteRole>` — the function the login role picker and the `AdminUser` extractor both call), `update_role` (replaces ALL of a user's roles on a site with exactly one, used by the single-role dropdown flows described below), `list_for_site` (returns `Vec<(User, String)>` — one row per role, so a multi-role user now appears more than once per site in this list), `list_for_user` (returns `Vec<(Site, String)>`, same one-row-per-role caveat), `count_admins` (added 2026-07-22 — counts `role = 'admin'` rows for a site; used to warn before removing/demoting the last one), `sole_admin` (added 2026-07-23 — returns the single user_id if exactly one `role = 'admin'` row exists for a site, independent of `sites.owner_user_id`; used to warn before demoting an admin who isn't the recorded site owner).

The old `get_role` (returned `Option<String>`, arbitrary-one-row semantics) was removed 2026-08-18 once multi-role made "the role" ambiguous — every call site was migrated to either `has_any_role` (access checks) or `list_roles_for_user_and_site` (anywhere the actual role value mattered).

### Auth Handler (`core/src/handlers/auth.rs`)

- `login_post` (`POST /admin/login`) — fetches user by email, verifies password, checks role allows admin access, resolves site from Host header, verifies site access via `site_user::has_any_role` for non-super-admins, writes `admin_user_id` and `current_site_id` to session, clears any stale `current_site_role` pin from a prior session. Does **not** itself decide whether a role pick is needed — that check happens lazily on the next `AdminUser`-guarded request (the post-login `/admin` redirect); see "Multiple roles per user per site" below.
- `public_login_post` (`POST /login`) — subscriber-only login, writes `account_user_id` to session.
- `logout` — removes `admin_user_id` and `current_site_id`.
- `account_logout` — removes `account_user_id`.

### Admin Users Handler (`core/src/handlers/admin/users.rs`)

Handlers: `list`, `new_user`, `save_new`, `edit_user`, `save_edit`, `delete_user`, `suspend_user`, `reactivate_user` (both added 2026-08-05), `bulk_delete_users`, `site_access_page`, `add_site_access`, `remove_site_access`. All require `can_manage_users` (admin or above). Super-admins see all users; site admins see users for their site only.

The `list` handler's "all sites" branch (global admin, no site filter) uses `user::list_all` rather than `user::list` — `list` filters `is_active = TRUE`, which would make a suspended user vanish from the admin UI entirely with no way to reactivate them. `list_all` only excludes soft-deleted rows. Other call sites of `user::list` (e.g. `sites.rs`'s assignable-user dropdown for new-site ownership) correctly keep the active-only filter — you shouldn't be able to hand site ownership to a suspended account.

### Role changes are exclusive to Site Access (changed 2026-07-22)

`save_edit` (`/admin/users/:id/edit`) no longer changes a user's role at all — `admin/src/pages/users.rs::render_editor` renders Role as a read-only `<p>` with a "Change Role" button linking to `/site-access` for existing users (the new-user form at `/admin/users/new` still has an editable role dropdown, since that's an initial assignment, not a change). Reason: the edit page has no site picker, so its old editable dropdown silently applied to whichever site the *acting* admin currently had selected in their own session — ambiguous and easy to misread as a global role change, especially for a target user with multiple site assignments. `/site-access` is explicit about which site is affected and is the only place site-scoped role changes happen now.

### Edit form: Role folded into the same panel as a 4th section (changed 2026-08-05)

`render_editor` renders both the new-user and edit-user forms inside a single `.card-boxed` panel (`.card-boxed-section` per field group — see the documentation page's card-boxed-style conventions). Section order: (1) Display Name, Username, Email, Password (Display Name now comes first, swapped 2026-08-05 to match how people actually think about the fields), (2) Role + site assignment (new-user form only — an editable role dropdown plus new-site/existing-site picker), (3) a live requirements checklist (Username/Password/Role, `.form-note`/`.pw-dot` pattern), and on the **edit** form only, (4) a Role display section — current role (read-only `<p>`) + "Change Role" button linking to `/site-access`, plus a read-only table of the user's existing site assignments (hostname/role, one row per site, sourced from `UserEdit.site_roles` populated via `site_user::list_for_user`). This is display-only; changing any of those roles still requires going through `/site-access`.

Previously (2026-07-23–2026-08-04) the Role section was a separate panel below the form — first as its own `.profile-container` card, later restyled to `.card-boxed` — but as a standalone panel it had no `max-width` constraint and stretched full page width while the form above it was capped at 580px. Folding it into the same panel as a 4th section fixed both the visual mismatch and the "why is this a separate page section" ambiguity — it's part of the same "editing this user" task.

Server-side, `display_name` is now also length-validated (≤60 chars, `validate_display_name`) on both `save_new` and `save_edit` — previously unbounded.

### Multiple Site Admins per site (changed 2026-07-22)

`site_users.role = 'admin'` is no longer capped at one holder per site. `add_site_access`'s `"site_admin"` branch: if the target site has no owner yet, the new user becomes owner (`sites.owner_user_id`) and is promoted to global role `site_admin`, same as before. If the site already has an owner, the site-access page's modal now offers three choices instead of forcing a swap: **Add as an additional Site Admin** (`displaced_action=add_additional` — existing owner and admin access untouched, new user just gets a second `site_users.role='admin'` row), **Remove from site** (`displaced_action=remove` — existing admin loses access, ownership transfers), or **Demote to Author, transfer ownership** (`displaced_action=demote_author`).

### Multiple roles per user per site + login-time role picker (added 2026-08-18)

A user can now hold more than one `site_users` role on the same site at once (e.g. both `editor` and `author`) — previously `site_users.role` was capped at exactly one row per `(site_id, user_id)` pair by the table's primary key.

**Schema (migration `0062_site_users_multi_role.sql`):** `site_users`' primary key changed from `(site_id, user_id)` to a surrogate `id UUID`, with a new `UNIQUE(site_id, user_id, role)` constraint replacing the old composite PK's uniqueness guarantee. The `role` CHECK constraint (`'admin' | 'editor' | 'author' | 'subscriber'`) was deliberately left untouched — it never allowed `super_admin`/`site_admin` before and still doesn't; see the `SiteRole` enum note above for the matching Rust-level guarantee.

**Session/UX model:** the chosen UX is "pick one role, act as if you only had that one for the rest of the session" — not "union of permissions across all held roles." Concretely:
- `AdminUser.site_role` (`core/src/middleware/admin_auth.rs`) is `Option<SiteRole>`, not a raw string. It is re-derived on every request (no role is cached in the session cookie) from `site_user::list_roles_for_user_and_site`:
  - 0 roles on the current site → falls back to trying the user's *global* role as a site role (works for `editor`/`author`; correctly yields `None` for `site_admin`/`super_admin`, since those aren't valid `SiteRole` values — this matches the pre-2026-08-18 fallback behavior for those roles, which never matched any site-role comparison either).
  - 1 role → used directly, no picker involved.
  - ≥2 roles → requires a valid `SESSION_CURRENT_ROLE_KEY` session value that is still one of the roles currently held (a role can be revoked after being pinned — re-validated every request, not just at pick time). If missing or invalid, the extractor returns `AdminAuthError::RolePickRequired`, which redirects to `/admin/pick-role` instead of failing the request.
- Global admins (`super_admin`) always get a synthetic `Some(SiteRole::Admin)` without ever consulting `site_users` — this bypass path is unchanged by the multi-role work.
- `GET/POST /admin/pick-role` (`core/src/handlers/admin/role_picker.rs`, page in `admin/src/pages/role_picker.rs`): shown when a role pick is required. Renders a `<select>` of the roles the user actually holds on the current site plus a single hexagon-icon pill submit button; shows the *site's own hostname* as the page heading (not the app name), and applies the visitor's saved theme preference before first paint, matching the standalone login page's conventions. The POST handler re-validates the submitted role against `list_roles_for_user_and_site` before pinning it — a posted role the user doesn't actually hold is rejected (logged, not silently accepted) rather than trusted from the form. Uses a separate lightweight `PickRoleUser` extractor (session user_id + site_id only, no role resolution) specifically so this route itself can't recurse into `RolePickRequired`.
- `sites::switch` and `sites::go_home` (`core/src/handlers/admin/sites.rs`) both clear `SESSION_CURRENT_ROLE_KEY` whenever they change the session's current site, since a different site can have an entirely different role set for the same user — the picker re-triggers on the next request if the new site also needs one.

**`site_user::add` semantics changed:** previously an upsert (`ON CONFLICT (site_id, user_id) DO UPDATE SET role = EXCLUDED.role`) that silently replaced any existing role — safe only because a user could hold exactly one. Now it's an idempotent per-role insert (`ON CONFLICT (site_id, user_id, role) DO UPDATE SET invited_by = COALESCE(...)`): re-adding a role the user already holds is a no-op, and adding a *different* role no longer removes any role they already had. Two other raw-SQL `site_users` upserts that predated this change and still used the old 2-column conflict target were found and fixed in the same pass: `site::create_with_defaults` (site creation's owner-as-admin seeding) and the CLI's `synap user create`.

**Users list (`/admin/users`) display:** a user with multiple roles on the same site previously showed one duplicated domain badge per role. As of 2026-08-18 it shows a single badge per site with a `+` suffix when more than one role is held there, and the full role list (e.g. "beth.com — Author, Editor") in the badge's hover tooltip — sourced from a small in-handler grouping fix in `admin/users.rs::list` (the raw membership query now also selects `su.role` and dedupes by `site_id` before building `UserRow.site_hostnames`/`site_role_labels`).

**Not changed by this work:** the Users-page single-role edit dropdown and `/site-access`'s Add/Remove flows still operate on "replace this user's role(s) on this site with exactly one" (`update_role`) — assigning *multiple* roles to a user is currently only reachable by calling `site_user::add` more than once (e.g. via the dev-tools seeding endpoint or directly), not through a dedicated multi-select UI. A proper multi-role assignment UI on `/site-access` is a natural follow-up, not yet built.

### Owner/role desync bug (fixed 2026-07-22)

Demoting a user away from `'admin'` via `add_site_access`'s editor/author/subscriber branch (or previously via `save_edit`, before role changes were removed from that form) updated `site_users.role` but never cleared `sites.owner_user_id` — since `/admin/sites` reads `owner_user_id` independently of `site_users.role` for its admin display, a demoted user could keep showing as "admin" there while `/site-access` correctly showed their new (lower) role. Both `add_site_access` and `save_edit` now clear `owner_user_id` (`UPDATE sites SET owner_user_id = NULL WHERE id = $1 AND owner_user_id = $2`) whenever the demoted user is the site's current owner — matching what `remove_site_access` already did on full removal. The site-access page's JS also warns before this happens: *"{name} is currently the Site Admin and owner of {site}. Changing their role will remove that access and site ownership. Continue?"*

### Last-admin-on-a-site warning (added 2026-07-22)

`site_access_page` now computes an `is_last_admin` flag per site assignment (`role == "admin" && site_user::count_admins(site_id) <= 1`) and threads it into `SiteAssignmentRow`. The remove button's confirm dialog uses a stronger message when it's the only admin: *"{hostname} has no other Site Admin. Removing this access will leave the site with no one able to manage it (other than a super admin). Continue?"* This is a warning, not a hard block — a super_admin always retains access via `AdminCaps::is_global_admin` regardless, so the risk is losing the *site owner's own* ability to manage their site, not a platform lockout.

### Sole-admin demotion warning gap on the Add form (fixed 2026-07-23)

The demotion warning described above only covered the **Remove** button. The **Add** form (re-selecting a site the user is already assigned to and choosing a different role, which upserts via `site_user::add`'s `ON CONFLICT ... DO UPDATE`) had its own, narrower warning that only fired when the target user equalled `sites.owner_user_id` (`SiteOption.existing_admin_id`, from `fetch_site_options`). That check missed two realistic, UI-reachable cases: a Site Admin added via **"Add as an additional Site Admin"** (never became the recorded owner), and an admin left over after the actual owner was removed from the site via `remove_site_access` (which clears `owner_user_id` but does not transfer it to a remaining admin). In both cases the user could be the site's *only* admin yet not match `existing_admin_id`, so demoting them via the Add form went through silently, leaving the site with no site-level admin. Fix: `SiteOption` gained `sole_admin_id`/`sole_admin_name`, populated in `site_access_page` via the new `site_user::sole_admin(site_id)` (independent of ownership); the Add form's submit handler now shows the same style of confirm — *"{name} is the only Site Admin for {site}. Changing their role will leave the site with no Site Admin. Continue?"* — whenever the selected site's sole admin matches the user being edited, regardless of who owns the site.

### Suspend / Reactivate (added 2026-08-05)

A lightweight alternative to delete: `suspend_user` (`POST /admin/users/:id/suspend`) sets `is_active = FALSE`, immediately blocking login everywhere without touching the account's content (posts, pages, media all untouched — unlike `delete_and_reassign`). `reactivate_user` (`POST /admin/users/:id/reactivate`) reverses it. Both are icon buttons (`user-x.svg`/`user-check.svg`, toggling based on `UserRow.is_active`) next to Edit/Delete on the Users list rows (`admin/src/pages/users.rs::build_staff_rows`/`build_sub_rows`), with a red "Suspended" badge and dimmed row (`opacity:.65`) for suspended accounts.

`suspend_user` guards mirror `delete_user`'s: no self-suspend, can't suspend a protected account, only a global admin may suspend another global admin, and the last global admin can never be suspended (same lockout risk as deleting them — checked via `count_global_admins`, which only counts active admins, so a suspended super_admin doesn't count toward the "last one" total). `reactivate_user` only requires `can_manage_users` — reactivating is the safe direction, no lockout risk to guard against. The underlying `is_active` column and a `deactivate()` model function already existed (added with the base user schema) but were never wired to any route or UI before this — `is_active = TRUE` was already a filter on every login-lookup query, so the blocking mechanism worked the moment it was called; only the CLI/admin-UI path to call it was missing.

Suspending an author was found (2026-08-06) to also break public rendering of their posts site-wide, not just their own login — see the `_include_inactive` discussion under Model above for the full list of affected pages and the fix. Suspension is meant to gate login only.

### Erase Personal Data — GDPR erasure (added 2026-08-19)

Subscribers only (staff accounts are a business relationship, not the self-service "forget me" case GDPR erasure targets). An "Erase Personal Data" icon (`shield.svg`) on each subscriber row in the Subscribers tab opens a review page (`GET /admin/users/{id}/erase-personal-data`) before anything happens — nothing is erased on click alone.

What erasure does (`user::erase_personal_data`): anonymizes the `users` row in place — username/email/display_name/bio/avatar replaced with placeholders, `password_hash` replaced with a random unusable value, `is_active` set false, `personal_data_erased_at` stamped. The row is **not deleted** — `posts.author_id`/`media.uploaded_by` are `ON DELETE RESTRICT` and `comments.author_id` is `ON DELETE CASCADE`, so a hard delete would either fail or silently wipe their comment history off other people's posts. Since `comments` has no separate author-identity columns (pure FK to `users`), anonymizing the row already anonymizes every comment they left; only `comments.ip_address` needs clearing separately (`comment::clear_ip_for_author`), since that's stored per-comment. Also deleted: `saved_posts` rows, pending `password_resets`, and (added 2026-09-08) pending `email_changes` requests for the account — required even though `email_changes.user_id` is `ON DELETE CASCADE`, since this function anonymizes the row in place rather than deleting it, so that cascade never fires.

`form_submissions` and `mail_log` have no `user_id` FK at all (submitters/recipients aren't required to have an account), so there's nothing to erase automatically there. Instead, the review page searches both by the subscriber's email (`form_submission::find_by_email`, `mail_log::find_by_email` — best-effort `ILIKE` text search, not an exact match) across every site the subscriber holds a role on, and shows matches as checkboxes (checked by default) for the admin to confirm or exclude before submitting.

Deliberately **not** touched: `audit_log` (the site's own accountability trail — GDPR generally allows retaining logs needed for security/legal purposes) and active sessions (`tower_sessions` stores an opaque blob with no queryable `user_id`, so there's no clean way to invalidate just this user's session — same known gap as `suspend_user`, which has never invalidated sessions either; the random password + `is_active = false` block any *new* login, but an already-active session isn't force-killed).

Every erasure is recorded to `audit_log` (`user.personal_data_erased`) with the original email as the target label, captured before it's overwritten.

### `is_protected` fix for CLI-created super_admins (fixed 2026-07-22)

`synap user create` lets you pick `super_admin` from its role menu but previously never set `is_protected`, unlike `install` which hardcodes it `TRUE`. A super_admin created this way silently defaulted to `is_protected = FALSE` (the migration 0012 column default), making them invisible to `dev reset`'s admin lookup, exempt from delete-protection, and unable to be auto-assigned as site owner. `cli/src/commands/user.rs` now sets `is_protected = (role == "super_admin")` on insert.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/users | `users::list` | List users |
| GET | /admin/users/new | `users::new_user` | New user form |
| POST | /admin/users/new | `users::save_new` | Create user |
| GET | /admin/users/{id}/edit | `users::edit_user` | Edit user form |
| POST | /admin/users/{id}/edit | `users::save_edit` | Save user edits |
| POST | /admin/users/{id}/delete | `users::delete_user` | Delete user |
| POST | /admin/users/{id}/suspend | `users::suspend_user` | Suspend login (content untouched) |
| POST | /admin/users/{id}/reactivate | `users::reactivate_user` | Restore login access |
| GET/POST | /admin/users/{id}/erase-personal-data | `users::erase_personal_data_review` / `erase_personal_data` | GDPR erasure review + confirm (subscribers only) |
| POST | /admin/users/bulk-delete | `users::bulk_delete_users` | Bulk delete |
| GET | /admin/users/{id}/site-access | `users::site_access_page` | Site access management |
| POST | /admin/users/{id}/site-access/add | `users::add_site_access` | Add site role |
| POST | /admin/users/{id}/site-access/remove | `users::remove_site_access` | Remove site role |
| GET | /admin/pick-role | `role_picker::show` | Login-time role picker (added 2026-08-18) — shown when the user holds >1 role on the current site |
| POST | /admin/pick-role | `role_picker::submit` | Pin the chosen role (server-revalidated) to the session |

## Database Schema

`users` table: `id UUID PK`, `username TEXT UNIQUE`, `email TEXT UNIQUE`, `display_name TEXT`, `password_hash TEXT`, `bio TEXT`, `avatar_media_id UUID`, `role TEXT`, `is_active BOOL`, `is_protected BOOL` (migration 0012), `deleted_at TIMESTAMPTZ` (migration 0016), `default_site_id UUID` (migration 0018), `personal_data_erased_at TIMESTAMPTZ` (migration 0067, 2026-08-19).

`site_users` table: `id UUID PK` (surrogate, migration 0062, 2026-08-18), `site_id UUID`, `user_id UUID`, `role TEXT` (CHECK: `admin | editor | author | subscriber` — never `super_admin`/`site_admin`), `invited_by UUID`, `created_at TIMESTAMPTZ` — `UNIQUE(site_id, user_id, role)` (migration 0062; previously `PRIMARY KEY (site_id, user_id)`, which is what capped a user to one role per site before 2026-08-18).

## Security Notes

`password_hash` has `#[serde(skip_serializing)]` so it never appears in JSON or template context. `is_protected` users cannot be deleted or suspended (enforced in handlers). Soft-delete preserves all authored content. `delete_and_reassign` reassigns posts and media before removing the user row. Suspension is fully reversible and never touches content — the more surgical option when delete is too destructive (e.g. a subscriber flagged for abuse, or a staff member on leave). Changing or recovering a password — or, as of 2026-09-08, completing a subscriber's verified self-service email change (see the Account Area doc) — immediately invalidates every other active session for that account via a credential-version marker compared on each request — see the session-rotation note in the Middleware & Auth doc (added 2026-09-07). This also means a staff edit to another user's email via `/admin/users/{id}/edit` now has the same side effect: it logs that user out of every other active session, exactly as an admin-forced password change already does.
