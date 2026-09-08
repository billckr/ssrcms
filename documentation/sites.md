---
title: Sites & Multisite
group: feature
updated_by: claude
last_updated: 2026-08-16
---
# Sites & Multisite

> Last updated: 2026-08-15 | Updated by: claude

## Overview

SynapCMS supports multiple sites from a single installation. Each site has its own hostname, content, users, themes, settings, and plugin activations.

## How It Works

### Data Model

- `sites`: `id` (UUID), `hostname`, `owner_user_id`, `created_at`, `updated_at`.
- `site_settings`: per-site settings (name, tagline, base URL, active theme, etc.). PK is `site_id`.
- `site_users`: maps users to sites with per-site roles.
- `site_plugins`: tracks active plugins per site.

### Ownership vs. Site Admin role (updated 2026-07-22)

`sites.owner_user_id` and `site_users.role = 'admin'` are related but distinct: a site can have **more than one** `admin`-role user (`core/src/handlers/admin/users.rs::add_site_access`), but only one of them is ever the `owner_user_id` — a lighter-weight "primary contact" marker, not a permission gate (permissions come entirely from `site_users.role` via `AdminCaps::from_roles` in `core/src/middleware/admin_auth.rs`, which only checks `site_role == "admin"`, never `owner_user_id`).

Whenever a user's `site_users.role` is changed away from `'admin'` (via `add_site_access` or `save_edit`) and that user is the site's current `owner_user_id`, the handler now also clears `owner_user_id` to `NULL`. Before this fix, demoting the owner left `owner_user_id` stale — `/admin/sites` reads `owner_user_id` (via `site::admin_email`) for its "admin" column independently of `site_users.role`, so the two views could silently disagree about who a site's admin was. `remove_site_access` already cleared ownership correctly on full removal; the gap was specific to in-place role changes.

### In-Memory Cache

At startup, `AppState` loads all sites into `site_cache: Arc<RwLock<HashMap<String, (Site, SiteSettings)>>>` keyed by hostname. Rebuilt via `state.reload_site_cache()` after site create/delete.

### Site Resolution

The `CurrentSite` middleware extractor resolves hostname to `(Site, SiteSettings)` on every public request. Cache hits are validated against the DB. Unknown or unconfigured hostnames return HTTP 404. There is no empty-cache fallback.

### Admin Management

`core/src/handlers/admin/sites.rs` handles CRUD. Global admins manage all sites; site admins see only their site. `POST /admin/sites/switch` updates a session variable to change the active site in the admin panel. On the Sites list (`admin/src/pages/sites.rs`), the "Switch to this site" action icon is hidden for whichever site matches `ctx.current_site` (added 2026-08-05) — previously it was shown even for the site you were already viewing, which was harmless (the form would just re-switch to the same site) but pointless clutter.

### New-site Site Admin assignment (added 2026-07-22)

`/admin/sites/new` (`admin_sites::new_site` / `create`) now has a "Site Admin" section alongside the hostname field, styled to match `/admin/users/new` (shared `.profile-container` card, `.user-form-grid` layout): **Assign later** (unchanged default — the creating admin becomes temporary owner/admin, or if impersonating, the currently-visited site's owner), **Existing user** (a dropdown of active non-super_admin users; picking one sets them as `owner_user_id` and registers them via `site_user::add`), or **New user** (inline username/email/display name/password fields, same live validation as `/admin/users/new` — username slugify-from-display-name, password requirements checklist, email format check; creates the account with global role `site_admin` and assigns them as owner). Previously the form only accepted a hostname, requiring a separate trip to `/admin/users/:id/site-access` to assign anyone.

### Dashboard Sites stat card (added 2026-07-22)

The admin dashboard (`core/src/handlers/admin/dashboard.rs`, `admin/src/pages/dashboard.rs`) now shows a **Sites** card between Pending and Users, linking to `/admin/sites`. Count is scoped like the sites list itself: total sites system-wide for a true super_admin (`site::count`), sites owned by the current site's owner when a super_admin is impersonating (`site::count_by_owner`), or sites the user has any role on otherwise (`site_user::list_for_user(...).len()`). The stat panel grid grew from `.stat-panel-6` to a new `.stat-panel-7` CSS rule to fit the extra tile.

### Settings page tabs (updated 2026-08-15)

`/admin/sites/{id}/settings` is organized into three tabs — **General**, **Maintenance**, **Email Settings** — styled with the same `.page-tabs` JS-toggled-panel pattern Form Designer uses (all panels render into the DOM at once, a small inline script toggles `.active` on click; no page reload between tabs). General also carries the Support/Site-ID card that used to sit above the tabs as its own section.

### Multi-provider email (added 2026-08-15, replaces the old single-Mailgun-account model)

The Email Settings tab replaced the old single "Email (Mailgun)" card with a full provider system — a site can configure any number of named email accounts (Mailgun, SMTP, SendGrid, Postmark), verify each with a test send, and forms pick which one to use individually. Full details, including the data model, sending logic, and setup steps: see the **Email Providers** doc and `docs/email-providers-guide.md` in the repo. The old per-site Mailgun override (`site_settings.mailgun_domain`/`mailgun_api_key_encrypted`, `/admin/sites/{id}/mail-config`) is gone — the install-wide `.env` Mailgun account is now the only site-independent fallback, used when a form has no provider selected.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET/POST | /admin/sites | `admin_sites::list / create` | List and create sites |
| GET | /admin/sites/new | `admin_sites::new_site` | New site form |
| POST | /admin/sites/switch | `admin_sites::switch` | Switch active site |
| GET | /admin/sites/{id}/settings | `admin_sites::site_settings` | Per-site settings (General/Maintenance/Email Settings tabs) |
| POST | /admin/sites/{id}/site-config | `admin_sites::save_site_config` | Save site config (General tab) |
| POST | /admin/sites/{id}/maintenance | `admin_sites::save_maintenance` | Toggle maintenance mode (Maintenance tab) |
| POST | /admin/sites/{id}/email-providers | `admin_email_providers::create` | Add an email provider (Email Settings tab) — see the **Email Providers** doc |
| POST | /admin/sites/{id}/delete | `admin_sites::delete` | Delete site |
| POST | /admin/sites/{id}/provision-ssl | `admin_sites::provision_ssl` | Provision SSL |

## Security Notes

- Site deletion cascades to `site_settings`, `site_users`, `site_plugins`, `email_providers`, and removes the site plugin directory from disk.
- Only global admins can create or delete sites.
- Email provider credentials are encrypted (AES-256-GCM, keyed off `SECRET_KEY`) before being written to the database — see the **Email Providers** doc for details specific to that system.

