---
title: Admin Panel
group: system
updated_by: claude
last_updated: 2026-09-09
---
# Admin Panel

> Last updated: 2026-09-09 | Updated by: claude

## Overview

The admin panel (`/admin/*`) is the management interface for content, media, users, sites,
settings, menus, themes, and the visual page builder. Handlers live in
`core/src/handlers/admin/` (one file per area); shared page shell, capability struct, and
render helpers live in the `admin` crate (`admin/src/lib.rs`, `admin/src/pages/`). All routes
require an authenticated admin session via the `AdminUser` extractor
(`core/src/middleware/admin_auth.rs` — see the `middleware` doc for how it resolves the current
site and capabilities).

## How It Works

### Layout and capabilities

`admin::admin_page()` in `admin/src/lib.rs` wraps every admin page in a shared sidebar/header
shell built from `admin::PageContext` (constructed per-request by `page_ctx`/`page_ctx_full` in
`core/src/handlers/admin/mod.rs` from `AdminUser.caps`). The sidebar shows/hides nav items based
on capability flags (`can_manage_users`, `can_manage_sites`, `can_manage_plugins`,
`can_manage_settings`, `can_manage_content`, `can_manage_themes`, `can_manage_taxonomies`,
`can_manage_forms`, `can_manage_pages`) and renders live sidebar badges: unread form submissions
count, pending-review post count, and pending-review page count (all computed in
`page_ctx_full`, which also carries a "visiting" badge and site-switcher link when a super_admin
is impersonating a site). The **Plugins** nav item and all `/admin/plugins/*` routes remain
disabled pre-launch — `core/src/handlers/admin/plugins.rs` implements a full
install/upload/activate/deactivate/delete flow mirroring `themes.rs`, but it is not wired
into `router.rs` at all, so it is currently unreachable in production.

`admin::html_escape`, `admin::live_search_script` (a small vanilla-JS 300ms-debounce fetch
helper used by the Posts and Users list pages' live search), and the shared media-picker
modal/iframe (`admin::media_picker_modal_html`, `admin::picker_page`) are also defined in
`admin/src/lib.rs`.

### Sidebar logo (added 2026-08-06)

The sidebar brand area (top-left corner, where `app_name` normally shows as text) can be replaced
with a custom logo image. This is **convention-based, not a DB setting or admin UI upload** — by
design, since it's a rarely-changed branding asset (set once, maybe changed if the company
rebrands), not user content that needs a Media Library entry.

- Drop a file at `admin/static/branding/logo.svg` (or `.png`/`.jpg`/`.webp`, checked in that
  priority order via `app_state::detect_admin_logo`) and restart the app
  (`./app.sh restart`). Deleting the file and restarting reverts to the `app_name` text.
- Detected once at startup, cached as `AppState.logo_url` (`Option<String>`, the public
  `/admin/static/branding/logo.{ext}` URL or `None`). Threaded into `admin::PageContext.logo_url`
  by `page_ctx` (`core/src/handlers/admin/mod.rs`) and rendered by `admin_page()`
  (`admin/src/lib.rs`) as `<img class="brand-logo">` in place of the text when set. No hot-reload —
  changing the file requires a restart, same tradeoff as `PORT` and other `.env`-era startup-only
  config, which is an acceptable tradeoff given how infrequently this changes.
- **Sizing**: the available brand box is fixed at **180px wide** (sidebar is 220px via
  `--sidebar-w`, minus 20px horizontal padding on each side from `.brand`) and the `.brand-logo`
  CSS rule uses a **fixed (not max-) height of 38px** — `admin/style/admin.css`. Using a fixed
  height rather than `max-height` matters: without it, the browser doesn't know the image's
  rendered size until it decodes, so the box collapses to 0 height and then snaps down once the
  image loads — a visible layout shift on every admin page navigation (each admin page is a fresh
  server-rendered document, not a SPA, so this repeats on every click). A fixed height reserves
  the space immediately regardless of load timing.
- For a logo to **fill the width without overflowing**, design it close to the box's aspect ratio,
  **180:38 ≈ 4.7:1** (a wide wordmark shape, not a square icon) — width is capped via
  `max-width: 100%` and scales down proportionally if the image is wider than that ratio, but a
  narrower/more-square image will hit the 38px height cap first and leave empty space on the
  sides rather than stretching to fill. SVG is preferred since it's resolution-independent at any
  of these sizes; if using a raster format, export close to 2x the target box (e.g. ~360×76) for
  sharpness on high-DPI displays.
- `.brand`'s own vertical padding was reduced from `1rem` to `.6rem` top/bottom (horizontal
  padding unchanged at `1.25rem`, which is what defines the 180px width budget above) to keep the
  total header height close to what it was with text-only, since the 38px image is taller than a
  single line of `app_name` text at the sidebar's default font size.

### Dashboard (`dashboard.rs`)

Renders role-scoped stats: editors/admins/super_admins see site-wide published/draft/pending
post counts, page count, user/subscriber counts (site-scoped for site staff, cross-site for a
true super_admin, owner-scoped when impersonating), and site counts. Authors additionally see
their own published/draft/pending counts plus two charts built from raw SQL aggregation queries
— posts-published-over-time and post-views-over-time (from the `post_views` table) — bucketed
by week/month/year via a `DashboardQuery` (`range`/`views_range`/`year`/`views_year`) query
param, each zero-filled for the selected bucket.

### Post/Page Editor (`posts.rs`)

- Quill rich-text editor feeding a hidden `content` input; publishing is blocked
  (`content_is_empty`) when the stripped HTML is blank.
- List views (`list`/`list_pages`) support pagination, a status filter, and live search
  (stop-word-stripped `ILIKE` on title) — a `partial` query param returns just the table-fragment
  HTML for the JS live-search to swap in, mirroring the Users page pattern.
- Author role is restricted: can only edit their own draft/pending posts, can only save as
  draft/pending (never publish directly), sees "pending review" badges scoped to their own posts.
- Pages support parent/child hierarchy (`parent_id`, excluding self from the parent dropdown via
  `fetch_parent_options`) and a template override dropdown populated by `scan_templates()`, which
  walks the active theme's `templates/` directory (site-specific copy preferred over global) and
  excludes reserved templates (`base`, `page`, `index`, `single`, `archive`, `search`, `404`) and
  anything under `partials/`.
- Optional post/page password protection (Argon2 hash, never round-tripped in plaintext) and a
  per-post `comments_enabled` toggle.
- Post list link targets are bare `/{slug}` (no `/blog/` prefix) per the router's URL
  unification.

### Categories & Tags (`taxonomy.rs`)

Shared `list_terms` helper renders both categories and tags (differentiated by `taxonomy` param);
create validates the slug format and surfaces duplicate-name/slug DB errors as a friendly flash.
Delete enforces site ownership for non-global-admins.

### Media Library (`media.rs`, `upload.rs`)

- Paginated grid (10/page) with mime-type filter (image/video/audio/document), folder filter,
  and per-type counts; authors see only their own uploads.
- Folders (`media_folders` table) support create/delete (delete offers "unassign" vs. "delete
  files too").
- `/admin/media` doubles as an iframe-embeddable browser/picker (`?browser=1` / `?picker=1`)
  used by the shared media-picker modal and the sidebar's "Media" nav link.
- JSON API (`api_list`, `api_update_meta`, `api_update_folder`) backs the inline media UI; all
  three verify site ownership before returning/mutating data, and folder assignment additionally
  verifies the target folder belongs to the same site.
- Upload (`upload.rs`) slugifies the original filename, appends an 8-char UUID suffix, stores
  files under a per-site `uploads/{site_uuid}/` subdirectory, and reads image dimensions directly
  from the uploaded bytes via `imagesize` (no disk round-trip). Alt text/title/caption are
  sanitized via the shared `sanitize_media_text` (strips tags and `&"\``, caps at 35 chars).

### Nav Menus (`menus.rs`)

CRUD for `nav_menus`/`nav_menu_items` (see the `database` doc for schema). Item URLs are
validated (`clean_url`) to only allow relative paths, `http(s)://`, and `mailto:` — rejecting
`javascript:`/`data:` schemes. All menu/item mutations re-verify the menu belongs to the caller's
site unless global admin.

### Comments (`comments.rs`)

A single `delete` handler — editors/admins/above may delete any comment; authors may not
(gated by `can_manage_content`, which is true for all content roles including author, so this is
enforced only at the route's capability check, not per-author-ownership).

### Forms (`forms.rs`)

Lists distinct submitted form names with unread/submission counts, per-form submission view
(marks all as read on view), CSV export (RFC 4180 escaped, columns ordered
name/email/subject/message first then alphabetical), single/bulk delete, and a block/unblock
toggle backed by the `form_blocks` table.

### Users (`users.rs`) — largest handler in this group

- `/admin/users` list is split into two tabs — **Site Users** (staff, i.e. any non-subscriber
  role) and **Subscribers** — each independently paginated (`USERS_PER_PAGE = 20`) and
  live-searched (display name/username/email, case-insensitive substring; `partial=1` returns a
  `tbody` fragment fetched via `live_search_script`). A super_admin gets a site filter dropdown;
  when impersonating, the dropdown and default filter are scoped to the visited site's owner
  rather than exposing the super_admin's own sites.
- Create/edit forms enforce username format (lowercase alphanumeric + hyphen), password policy
  (`user::validate_password`), and hostname format when the "create a new site for this user"
  option is chosen. New-site creation seeds `sites/{uuid}/themes/`, `uploads/{uuid}/`, a hostname
  symlink under `uploads/`, and copies the global default theme — done in a `spawn_blocking` task
  so it doesn't block the request.
- Role editing on `/admin/users/{id}/edit` is deliberately **read-only for role** — role changes
  now go exclusively through the dedicated Site Access UI (see below) "so it's explicit about
  which site is affected."
- Delete has four guards, in order: no self-delete, cannot delete a protected account, only a
  global admin may delete another global admin, and the last global admin account can never be
  deleted (checked via `count_global_admins`).
- **Site Access** (`site_access_page`, `add_site_access`, `remove_site_access`): assigns/removes
  a user's role on a specific site. Assigning `site_admin` when the target site already has a
  different non-super_admin owner triggers a `displaced_action` decision
  (`remove` / `demote_author` / `add_additional`) that the UI must resolve first (posting without
  one returns `?error=site_admin_exists`); demoting a site's `admin` role away also clears
  `sites.owner_user_id` if the demoted user was the owner, keeping the owner column and
  `site_users.role` from silently disagreeing.

### Sites (`sites.rs`)

- List scoping: a true super_admin sees every site (with a "primary domain" badge derived from
  each owner's `default_site_id`); an impersonating super_admin sees only sites owned by the
  visited site's owner; other staff see only sites they hold a role on.
- New-site creation (`create`) validates hostname format, optionally creates a new Site Admin
  user or assigns an existing one, and (matching the users.rs flow) seeds the new site's
  directories, hostname upload symlink, and default theme copy in a background
  `spawn_blocking` task.
- `switch`/`go_home` manage the `current_site_id` session key (site_admin can only switch to
  sites they have a role on); `go_home` restricts its `?next=` redirect target to paths starting
  with `/admin` to prevent open redirects.
- `delete` removes the DB row plus the site's data directory
  (`sites/{uuid}/`), upload directory and hostname symlink, and plugin directory
  (`plugins/sites/{uuid}/` — DB rows there cascade automatically).
- `site_settings`/`save_site_config` edit per-site display settings (name, description,
  language, date format, posts-per-page) stored via `set_site_setting` into the generic
  `site_settings` table. **Maintenance mode and IP allow/deny-list settings are not exposed here
  or anywhere in the admin UI** — they are set directly in `site_settings` (keys documented in
  the `middleware` doc) via `synap site maintenance|allow-ip|block-ip`, not through any HTTP
  handler.
- `provision_ssl` appends a Caddy reverse-proxy block for the site's hostname to the Caddyfile
  (idempotent — checks `caddy_block_exists` first) and shells out to `caddy reload` against the
  local Caddy admin API (no `sudo` needed/allowed, since `NoNewPrivileges` blocks it).

### Page Builder (`builder.rs`)

Full CRUD for builder projects (`builder_projects`) and their pages
(`page_compositions`) — project list/create/rename/activate/deactivate/delete, per-project page
list/create/duplicate/set-homepage/delete, and a JSON save/load/publish API that reads/writes
`draft_composition` (editor) vs. `composition` (live, only updated on Publish). A project can
only be activated once it has at least one *published* page. Two parallel page editor UIs are
served — `edit_page` (v1) and `edit_page2` (v2, newer) — both rendering the same
`admin::pages::builder::render_editor` shell with a `use_v2` flag. This is a large subsystem;
see the dedicated `builder` documentation slug for the composition JSON schema and zone
internals — this doc only tracks its presence in the admin routing surface.

### Themes (`themes.rs`, 1700+ lines)

Not one of this pass's four target slugs, but shares the admin handler directory and the
`copy_dir_all` helper reused by `users.rs`, `sites.rs`, and `plugins.rs` for seeding
new-site/new-user theme copies. Themes resolve from three tiers — `themes/global/`,
`themes/private/` (super_admin only), and `sites/{uuid}/themes/` — with path-traversal guards
(canonicalize + `starts_with`) on every filesystem operation, and a global/private theme is
lazily copied into the site's own folder the first time it's activated so it appears in "My
Themes". Includes a file-in-theme editor with per-file `.bak` restore, and an uploaded-zip
installer with a required-file check (`REQUIRED_TEMPLATES`).

### Documentation Viewer (`documentation.rs`)

`/admin/documentation` — super_admin only — scans the configured `documentation/` directory for
Markdown files, reads each file's `---`-delimited frontmatter, and renders the resulting index and
sections through `admin::pages::documentation::render_list`. Documents are ordered with the
`system` group first, then `feature`, then any other group, and alphabetically by title within a
group. Adding a valid Markdown file such as `translations.md` therefore adds it to the in-app index
without a database seed or hard-coded registry.

### Settings & Profile (`settings.rs`, `profile.rs`)

`settings.rs` edits `app_settings` (installation-wide `app_name`/`timezone`/`max_upload_mb`/
`default_theme`/`ai_translation_enabled` — the last one a kill switch for the whole AI Post
Translation feature, see that doc's Administrator Workflow §0) — gated by `can_manage_settings`,
which per `admin_auth.rs` is only true for a super_admin viewing their own default/home site.
`profile.rs` lets any admin user edit their own email/display name/bio and change their own
password (current-password re-verification + policy check).

`POST /admin/profile/sign-out-other-devices` (added 2026-09-08) invalidates every other active
session for the signed-in admin/staff account — same "Sign out other devices" mechanism as the
subscriber-facing `/account/profile` (see the **Account Area** doc for the full explanation);
`profile::sign_out_other_devices` mirrors `account::sign_out_other_devices` exactly, just against
`SESSION_CREDENTIAL_VERSION_KEY` instead of the account-side session key.

### Two-Factor Authentication / TOTP MFA (added 2026-09-11)

Staff-only (`/admin/login` accounts — subscribers are unaffected), self-service, opt-in via
`/admin/profile`'s "Two-factor authentication" card. Pure local RFC 6238/4226 TOTP — no
third-party service, no SMS.

- `GET`/`POST /admin/profile/2fa/setup/start` → `/admin/profile/2fa/setup` → `.../confirm`:
  enrollment requires re-confirming the current password (`profile::mfa_setup_start`), generates a
  fresh secret (`user_totp::start_enrollment`), and shows a QR code (`utils::totp::qr_svg`,
  rendered inline as SVG — no image codec, no external QR service) plus the manual-entry secret.
  Confirming with the first real code from the scanned authenticator app
  (`user_totp::confirm_enrollment`) sets `user_totp.enabled_at` and issues 10 recovery codes
  (`mfa_recovery_code::generate_batch`), shown exactly once.
- `POST /admin/profile/2fa/disable` and `.../recovery-codes/regenerate`: both also require the
  current password. Each sends the account a notification email (reusing `mail::send_for_site`,
  resolving a site via `admin.site_id.or(admin.user.default_site_id)` — the same fallback problem
  and solution as staff self-service email changes, see the **Account Area** doc) so a
  hijacked-session attacker silently toggling MFA as a persistence mechanism gets caught by the
  real owner.
- Login gate lives in `handlers::auth::login_post` / `mfa_login_form` / `mfa_login_post`: once the
  password check (and role/site-access checks) succeed for an enrolled account, the session gets a
  short-lived pending state (`SESSION_MFA_PENDING_*` keys, 5-minute window — see the Middleware
  doc) instead of a full login, and the browser is sent to `/admin/login/mfa`. Only a correct TOTP
  code or an unused recovery code completes the login (`finish_admin_login`, shared with the
  non-MFA path so both write the exact same session keys and audit-log entry). Submitting the
  right second factor twice in a row is rejected — `user_totp.last_used_step` blocks replaying the
  same code within its own 30-second validity window. The `/admin/login/mfa` route is rate-limited
  the same way `/admin/login` itself is (`middleware::auth_security`, flow `"admin-mfa"`, keyed by
  user id).
- The one-time recovery-codes page (`admin::pages::profile_mfa::render_recovery_codes`) also offers
  a client-side "Download codes" button — builds a `Blob`/`URL.createObjectURL` download of a
  `.txt` file, no server round-trip.
- `ON DELETE CASCADE` on both `user_totp` and `mfa_recovery_codes` means deleting a user cleans
  these up automatically; GDPR `erase_personal_data` never touches them since it's
  subscriber-only and MFA is staff-only, so the two code paths never overlap.

## Routes / Endpoints

See the `routing` doc for the full `/admin/*` route table (all admin routes are defined in
`core/src/router.rs`).

## Security Notes

- All admin routes require an authenticated admin session (`AdminUser` extractor); unauthenticated
  requests redirect to `/admin/login`.
- `Cache-Control: no-store` applied to all `/admin/*` responses (`router.rs`).
- Every mutating handler that isn't global-admin-only re-verifies the target row's `site_id`
  matches `admin.site_id` (or ownership, for sites) before allowing the action — site isolation
  is enforced per-handler, not centrally.
- Filesystem operations (theme/plugin install, activate, file editor) canonicalize paths and
  check `starts_with` against the allowed root before touching disk, guarding against `..`
  traversal from user-supplied theme/plugin/file names.
- `/admin/plugins/*` handlers exist in source but are not registered in `router.rs` — the
  feature is code-complete but inert pre-launch.
