# Multi-Site Guide

Synaptic Signals supports running multiple client sites from a single database and binary. This guide explains the architecture, how to set up additional sites, and how to migrate an existing single-site install.

---

## Architecture Overview

- **Single PostgreSQL database** — all sites share one DB. Content is isolated by a `site_id` UUID column on `posts`, `taxonomies`, `media`, and `site_settings`.
- **Domain-based routing** — the `Host` header is matched against the `sites` table. Each request is scoped to the matching site's settings and content.
- **Global user pool** — users exist once across all sites. Per-site roles are stored in the `site_users` junction table.
- **Single binary** — one running `synaptic` process serves all sites. Use Caddy as a reverse proxy with multiple domain names pointing to the same port.

---

## Database Tables

| Table | Purpose |
|-------|---------|
| `sites` | One row per site (id, hostname) |
| `site_users` | Maps users to sites with a role (admin/editor/author/subscriber) |
| `posts.site_id` | Scopes posts to a site |
| `taxonomies.site_id` | Scopes categories/tags to a site |
| `media.site_id` | Scopes uploads to a site |
| `site_settings.site_id` | Scopes key-value settings to a site |

---

## Migrating an Existing Single-Site Install

After upgrading to a version that includes migrations 0008–0011, run the backfill command **once**:

```bash
synaptic-cli site init --hostname your-domain.com
```

This command:
1. Creates a `sites` row for your primary domain
2. Backfills all existing posts, taxonomies, media, and settings with the new `site_id`
3. Adds all existing users to the new site with their current roles

Restart Synaptic Signals after running `site init`.

---

## Adding a Second Site

### Via CLI

```bash
synaptic-cli site create --hostname client.example.com
```

Then add a user to the new site via the admin UI (`/admin/sites`) or add them manually:

```sql
INSERT INTO site_users (site_id, user_id, role)
VALUES ('<site-uuid>', '<user-uuid>', 'admin');
```

### Via Admin UI

1. Navigate to **Sites** in the admin sidebar
2. Click **New Site**
3. Enter the hostname (e.g. `client.example.com`)
4. Click **Create Site**

---

## Switching Between Sites in Admin

The admin dashboard shows the currently selected site. Click **Switch site** or navigate to `/admin/sites` to change the active site. Site selection is stored in your session — each browser session can be on a different site simultaneously.

---

## DNS and Caddy Configuration

Point multiple DNS A-records to your server's IP, then configure Caddy to route all of them to the same Synaptic process:

```
client-a.example.com {
    reverse_proxy localhost:3000
}

client-b.example.com {
    reverse_proxy localhost:3000
}
```

Caddy will obtain TLS certificates automatically for each domain via Let's Encrypt.

---

## User and Role Management

There are two tiers of admin privilege:

| Account type | How identified | Scope |
|---|---|---|
| **Super Admin** | `users.role = 'super_admin'` | All sites. Can manage Sites, themes (global), plugins, all users. `is_protected = TRUE` on the install-time account — never deletable. |
| **Site admin** | `site_users.role = 'admin'` | One site only. Cannot see or manage super admin accounts. |

Per-site roles in `site_users`:

| Role | Permissions |
|------|------------|
| `admin` | Full access to site content and settings |
| `editor` | Create, edit, and publish all posts |
| `author` | Create and edit own posts only |
| `subscriber` | Read-only access |

A single user can have different roles on different sites. Super admin accounts
(`users.role = 'super_admin'`) always have full access across all sites regardless
of `site_users` entries.

### Protected accounts

The install-time super admin account is automatically marked `is_protected = TRUE` by
`synaptic-cli install`. Migration 0013 retroactively protects any pre-existing `admin`
accounts on upgrade. Protected accounts have no delete button in the admin UI and all
server-side deletion attempts are rejected regardless of the requester's role.

### Theme uploads per site

- **Super Admin** theme uploads land in `themes/global/` — available to all sites.
- **Site Admin** theme uploads land in `themes/sites/<site_id>/` — scoped to that site.

Both super admins and site admins can see global themes in the Appearance panel.
Site admins additionally see their site-specific themes.

---

## Theme and Plugin Sharing

Themes and plugins are stored on the filesystem and are available to all sites. Each site can activate a different theme via its own `active_theme` setting in `site_settings`.

---

## Site Management CLI Reference

```
synaptic-cli site init --hostname <domain>
    Initialize multi-site on an existing single-site install.
    Run once after applying migrations 0008-0011.

synaptic-cli site create --hostname <domain>
    Create a new empty site.

synaptic-cli site list
    List all sites with post counts.

synaptic-cli site delete --id <uuid>
    Delete a site and all its content (with confirmation prompt).
```

---

## Verification Checklist

After setup:
- [ ] `synaptic-cli site list` shows your sites
- [ ] Admin `/admin/sites` shows the site switcher
- [ ] DNS resolves both domains to your server
- [ ] Caddy issues TLS certificates for both domains
- [ ] Each site serves different content at its root URL
- [ ] Admin sessions are scoped: switching sites in one tab doesn't affect another browser session
