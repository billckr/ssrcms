//! Admin handlers for site management (list, create, switch, settings).

const DEFAULT_MAINTENANCE_MESSAGE: &str =
    "This site is currently undergoing scheduled maintenance. Please check back soon.";

/// Owner, site_admin/admin role, or global admin — the bar for managing a
/// site's own settings (config, maintenance mode, email providers).
pub(crate) async fn require_site_manager(state: &AppState, admin: &AdminUser, site: &crate::models::site::Site, ) -> bool {
    let is_owner = site.owner_user_id == Some(admin.user.id);
    let roles = crate::models::site_user::list_roles_for_user_and_site(&state.db, site.id, admin.user.id)
        .await
        .unwrap_or_default();
    let has_role = roles.contains(&crate::models::site_user::SiteRole::Admin);
    admin.caps.is_global_admin || is_owner || has_role
}

/// Returns true if `h` is a plausibly valid hostname with a real TLD.
/// Labels must be alphanumeric + hyphens, not start/end with a hyphen.
/// TLD must be at least 2 alphabetic characters.
fn is_valid_hostname(h: &str) -> bool {
    let parts: Vec<&str> = h.split('.').collect();
    if parts.len() < 2 { return false; }
    let tld = parts.last().unwrap();
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) { return false; }
    for label in &parts[..parts.len() - 1] {
        if label.is_empty() { return false; }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return false; }
        if label.starts_with('-') || label.ends_with('-') { return false; }
    }
    true
}

use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use std::collections::HashMap;
use serde::Deserialize;
use uuid::Uuid;

use std::path::Path as FsPath;
use crate::app_state::AppState;
use crate::middleware::admin_auth::{AdminUser, SESSION_CURRENT_ROLE_KEY, SESSION_CURRENT_SITE_KEY};
use crate::handlers::admin::themes::copy_dir_all;
use admin::pages::sites::{SiteRow, SiteSettingsData};
use tower_sessions::Session;

/// GET /admin/sites — list sites.
/// super_admin sees all sites (can manage all).
/// site_admin sees owned sites (can manage) plus sites they're assigned to (switch only).
/// editors/authors see only sites they're assigned to (switch only).
pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let flash = params.get("flash").map(|s| s.as_str());
    // Require at minimum a logged-in admin user; subscribers/unauthenticated are blocked by AdminUser extractor.
    // All roles that reach here may view the page.
    let cs = state.site_hostname(admin.site_id);
    let can_create = admin.caps.can_manage_sites;

    // Read the Caddyfile once to determine SSL status for each site.
    let caddyfile_content = std::fs::read_to_string(&state.config.caddyfile_path).unwrap_or_default();

    // Build site list with per-row manage flag.
    let mut rows: Vec<SiteRow> = Vec::new();

    if admin.caps.is_global_admin && !admin.caps.is_impersonating {
        // True super admin view — see all sites.
        let sites = crate::models::site::list(&state.db).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list sites: {:?}", e);
            vec![]
        });

        // Collect the set of site IDs that are the default_site_id of their
        // non-super_admin owner — these get the "primary domain" badge.
        let primary_ids: std::collections::HashSet<Uuid> = sqlx::query_scalar(
            r#"SELECT s.id FROM sites s
               JOIN users u ON u.id = s.owner_user_id
               WHERE u.role != 'super_admin'
                 AND u.default_site_id = s.id
                 AND u.deleted_at IS NULL"#,
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        for s in &sites {
            let (admin_email, user_count, subscriber_count, post_count, page_count, maintenance_mode) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
                crate::app_state::get_site_setting(&state.db, s.id, "maintenance_mode"),
            );
            let is_sys_default = admin.user.default_site_id == Some(s.id);
            rows.push(SiteRow {
                id: s.id.to_string(),
                hostname: s.hostname.clone(),
                admin_email: admin_email.unwrap_or(None),
                user_count: user_count.unwrap_or(0),
                subscriber_count: subscriber_count.unwrap_or(0),
                post_count: post_count.unwrap_or(0),
                page_count: page_count.unwrap_or(0),
                is_default: is_sys_default,
                can_manage: true,
                ssl_active: caddy_block_exists(&caddyfile_content, &s.hostname),
                // Only show primary-domain badge for non-system-domain sites.
                is_primary_domain: !is_sys_default && primary_ids.contains(&s.id),
                maintenance_mode: maintenance_mode.as_deref() == Some("true"),
            });
        }
    } else if admin.caps.is_global_admin && admin.caps.is_impersonating {
        // Super admin impersonating — show all sites owned by the current site's owner.
        let (sites, owner_default_site_id) = if let Some(site_id) = admin.site_id {
            let owner_row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
                "SELECT owner_user_id, (SELECT default_site_id FROM users WHERE id = s.owner_user_id) \
                 FROM sites s WHERE s.id = $1",
            )
            .bind(site_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let (owner_id, default_site_id) = owner_row
                .map(|(oid, dsi)| (oid, dsi))
                .unwrap_or((None, None));

            let sites = match owner_id {
                Some(owner) => crate::models::site::list_by_owner(&state.db, owner)
                    .await
                    .unwrap_or_default(),
                None => crate::models::site::get_by_id(&state.db, site_id)
                    .await
                    .map(|s| vec![s])
                    .unwrap_or_default(),
            };
            (sites, default_site_id)
        } else {
            (vec![], None)
        };

        for s in &sites {
            let (admin_email, user_count, subscriber_count, post_count, page_count, maintenance_mode) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
                crate::app_state::get_site_setting(&state.db, s.id, "maintenance_mode"),
            );
            rows.push(SiteRow {
                id: s.id.to_string(),
                hostname: s.hostname.clone(),
                admin_email: admin_email.unwrap_or(None),
                user_count: user_count.unwrap_or(0),
                subscriber_count: subscriber_count.unwrap_or(0),
                post_count: post_count.unwrap_or(0),
                page_count: page_count.unwrap_or(0),
                is_default: false,
                can_manage: true,
                ssl_active: caddy_block_exists(&caddyfile_content, &s.hostname),
                is_primary_domain: owner_default_site_id == Some(s.id),
                maintenance_mode: maintenance_mode.as_deref() == Some("true"),
            });
        }
    } else {
        // Non-global-admin: the current site plus any other sites where they
        // hold the 'admin' role. Editor/author roles on other sites stay
        // confined to that site's own login.
        let site_roles = crate::models::site_user::list_for_user_scoped(&state.db, admin.user.id, admin.site_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("failed to list sites for user {}: {:?}", admin.user.id, e);
                vec![]
            });
        for (s, site_role) in &site_roles {
            let (admin_email, user_count, subscriber_count, post_count, page_count, maintenance_mode) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
                crate::app_state::get_site_setting(&state.db, s.id, "maintenance_mode"),
            );
            // can_manage if they own the site or hold an admin role on it.
            // Delete is separately blocked for the default site in the renderer.
            let can_manage = s.owner_user_id == Some(admin.user.id)
                || matches!(site_role.as_str(), "admin" | "site_admin");
            rows.push(SiteRow {
                id: s.id.to_string(),
                hostname: s.hostname.clone(),
                admin_email: admin_email.unwrap_or(None),
                user_count: user_count.unwrap_or(0),
                subscriber_count: subscriber_count.unwrap_or(0),
                post_count: post_count.unwrap_or(0),
                page_count: page_count.unwrap_or(0),
                is_default: admin.user.default_site_id == Some(s.id),
                can_manage,
                ssl_active: caddy_block_exists(&caddyfile_content, &s.hostname),
                is_primary_domain: false,
                maintenance_mode: maintenance_mode.as_deref() == Some("true"),
            });
        }
    }

    // Search + pagination are applied in-memory rather than pushed into SQL —
    // each branch above already builds `rows` via N+1 per-site enrichment
    // queries (admin_email/user_count/etc.), not a single paginated query,
    // so filtering/slicing the already-built Vec is the simplest option that
    // doesn't require restructuring those three branches.
    let search = params.get("search").map(|s| s.trim()).unwrap_or("");
    if !search.is_empty() {
        let needle = search.to_lowercase();
        rows.retain(|r| {
            r.hostname.to_lowercase().contains(&needle)
                || r.admin_email.as_deref().unwrap_or("").to_lowercase().contains(&needle)
        });
    }

    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("");
    let dir = params.get("dir").map(|s| s.as_str()).unwrap_or("");
    match sort {
        "admin" => rows.sort_by_key(|r| r.admin_email.as_deref().unwrap_or("").to_lowercase()),
        "users" => rows.sort_by_key(|r| r.user_count),
        "subs"  => rows.sort_by_key(|r| r.subscriber_count),
        "posts" => rows.sort_by_key(|r| r.post_count),
        "pages" => rows.sort_by_key(|r| r.page_count),
        "hostname" => rows.sort_by_key(|r| r.hostname.to_lowercase()),
        _ => {}
    }
    if !sort.is_empty() && dir == "desc" {
        rows.reverse();
    }

    const PER_PAGE: i64 = 20;
    let total = rows.len() as i64;
    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let page = params.get("page").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).clamp(1, total_pages);
    let start = ((page - 1) * PER_PAGE) as usize;
    let end = (start + PER_PAGE as usize).min(rows.len());
    let page_rows = rows.get(start..end).unwrap_or(&[]);

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    if params.contains_key("partial") {
        return Html(admin::pages::sites::sites_list_fragment(page_rows, page, total_pages, search, sort, dir, &ctx));
    }

    Html(admin::pages::sites::render_list(page_rows, flash, can_create, page, total_pages, search, sort, dir, &ctx))
}

/// Assignable users for the site-owner dropdown on the new-site form: the
/// acting admin themselves (as "You", pinned first) plus every site_admin-role
/// user, since only site_admin accounts (or the super_admin creating them)
/// can own a site. Editors, authors, and subscribers are never site owners.
async fn fetch_assignable_users(state: &AppState, current_user_id: Uuid) -> Vec<admin::pages::sites::UserOption> {
    let mut opts = vec![admin::pages::sites::UserOption {
        id: current_user_id.to_string(),
        label: "You".to_string(),
    }];
    let mut others: Vec<_> = crate::models::user::list(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|u| u.role == "site_admin" && u.id != current_user_id)
        .map(|u| admin::pages::sites::UserOption {
            id: u.id.to_string(),
            label: format!("{} ({})", u.display_name, u.email),
        })
        .collect();
    opts.append(&mut others);
    opts
}

/// GET /admin/sites/new — new site form.
/// Available to super_admin and site_admin roles.
pub async fn new_site(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    if !admin.caps.can_manage_sites {
        return Html("<h1>403 Forbidden</h1>".to_string());
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let data = admin::pages::sites::NewSiteData {
        existing_user_id: admin.user.id.to_string(),
        existing_users: fetch_assignable_users(&state, admin.user.id).await,
        ..Default::default()
    };
    Html(admin::pages::sites::render_new(&data, None, &ctx))
}

#[derive(Deserialize, Default)]
pub struct NewSiteForm {
    pub hostname: String,
    /// "existing" | "new" — which Site Admin sub-form was submitted.
    pub user_assignment: Option<String>,
    pub existing_user_id: Option<String>,
    pub new_username: Option<String>,
    pub new_email: Option<String>,
    pub new_display_name: Option<String>,
    pub new_password: Option<String>,
}

/// Rebuild the new-site form's prefill data after a validation failure, so the
/// admin doesn't have to retype the hostname or re-enter the new-user fields
/// (password excluded — never echo it back).
async fn rebuild_new_site_data(state: &AppState, admin: &AdminUser, form: &NewSiteForm, hostname: &str) -> admin::pages::sites::NewSiteData {
    admin::pages::sites::NewSiteData {
        hostname: hostname.to_string(),
        user_assignment: form.user_assignment.clone().unwrap_or_else(|| "existing".to_string()),
        existing_user_id: form.existing_user_id.clone().unwrap_or_default(),
        new_username: form.new_username.clone().unwrap_or_default(),
        new_email: form.new_email.clone().unwrap_or_default(),
        new_display_name: form.new_display_name.clone().unwrap_or_default(),
        existing_users: fetch_assignable_users(state, admin.user.id).await,
    }
}

/// POST /admin/sites — create a new site.
/// super_admin uses plain `create()`; site_admin uses `create_with_defaults()` which
/// seeds site_settings and registers them as owner/admin in a single transaction.
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<NewSiteForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_sites {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let hostname = form.hostname.trim().to_lowercase();
    if hostname.is_empty() {
        let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
        return Html(admin::pages::sites::render_new(&data, Some("Hostname cannot be empty."), &ctx)).into_response();
    }
    if !is_valid_hostname(&hostname) {
        let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
        return Html(admin::pages::sites::render_new(
            &data,
            Some("Must be a valid domain (e.g. example.com, my-site.com, sub.example.com)."),
            &ctx,
        )).into_response();
    }

    // Resolve who owns/admins the new site. A site admin (non-global) is
    // always the owner of anything they create — no user picker at all, and
    // any submitted user_assignment/existing_user_id/new_* fields are
    // ignored — this used to let a site admin hand a new site off to a
    // brand-new user, who then became the sole owner while the creating
    // site admin got no site_users row at all, so the site silently
    // vanished from their own /admin/sites list (still visible to
    // super_admin, who sees everything, which is what made it look like
    // the site had been reassigned to the super_admin account). Only a
    // global admin retains the existing/new-user sub-form flexibility,
    // since a super_admin is routinely creating sites on behalf of others.
    // This branches on `is_global_admin`, not `can_manage_sites` — the two are
    // currently equivalent in practice (only super_admin and site_admin ever
    // reach this handler) but are separate fields on AdminCaps. See the note
    // on `can_manage_sites` in middleware/admin_auth.rs: if a new role tier
    // ever gets `can_manage_sites: true`, this branch (and render_new()'s
    // template choice) need re-checking, not just assumed to still be correct.
    let owner_id: Uuid = if !admin.caps.is_global_admin {
        admin.user.id
    } else {
    match form.user_assignment.as_deref() {
        Some("existing") => {
            let Some(uid) = form.existing_user_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()) else {
                let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                return Html(admin::pages::sites::render_new(&data, Some("Please select a user."), &ctx)).into_response();
            };
            // "You" (the acting admin) is always a valid choice, even for a
            // super_admin who wouldn't otherwise show up in the site_admin list.
            let valid = uid == admin.user.id || matches!(
                crate::models::user::get_by_id(&state.db, uid).await,
                Ok(u) if u.role == "site_admin"
            );
            if !valid {
                let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                return Html(admin::pages::sites::render_new(&data, Some("Selected user not found."), &ctx)).into_response();
            }
            uid
        }
        Some("new") => {
            let username = form.new_username.clone().unwrap_or_default().trim().to_lowercase();
            let email = form.new_email.clone().unwrap_or_default().trim().to_lowercase();
            let display_name = form.new_display_name.clone().unwrap_or_default().trim().to_string();
            let password = form.new_password.clone().unwrap_or_default();

            if let Err(msg) = crate::models::user::validate_username(&username) {
                let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                return Html(admin::pages::sites::render_new(
                    &data,
                    Some(msg),
                    &ctx,
                )).into_response();
            }
            if email.is_empty() {
                let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                return Html(admin::pages::sites::render_new(&data, Some("Email cannot be empty."), &ctx)).into_response();
            }
            if let Err(msg) = crate::models::user::validate_password(&password) {
                let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                return Html(admin::pages::sites::render_new(&data, Some(msg), &ctx)).into_response();
            }

            let create_user = crate::models::user::CreateUser {
                username: username.clone(),
                display_name: if display_name.is_empty() { username.clone() } else { display_name },
                email,
                password,
                role: crate::models::user::UserRole::SiteAdmin,
            };
            match crate::models::user::create(&state.db, &create_user).await {
                Ok(new_user) => new_user.id,
                Err(e) => {
                    let msg = if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                        "A user with that username or email already exists.".to_string()
                    } else {
                        format!("Failed to create user: {e}")
                    };
                    let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
                    return Html(admin::pages::sites::render_new(&data, Some(&msg), &ctx)).into_response();
                }
            }
        }
        _ => {
            let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
            return Html(admin::pages::sites::render_new(&data, Some("Please select a site admin."), &ctx)).into_response();
        }
    }
    };

    // A site admin creating a site is always creating it "underneath" the
    // site they're currently logged into — that's what makes this a
    // non-top-level site (no System Settings, inherits branding — see
    // Site::parent_site_id's doc comment). A global admin's sites are always
    // top-level, regardless of which site they happened to be viewing.
    let parent_site_id = if admin.caps.is_global_admin { None } else { admin.site_id };
    let result = crate::models::site::create_with_defaults(&state.db, &hostname, Some(owner_id), parent_site_id)
        .await;

    match result {
        Ok(site) => {
            tracing::info!(
                user_id = %admin.user.id,
                user_email = %admin.user.email,
                role = if admin.caps.is_global_admin { "super_admin" } else { "site_admin" },
                hostname = %hostname,
                "site created",
            );
            super::audit(&state, &admin, "site.created", "site", Some(site.id), &hostname, Some(site.id)).await;

            // Seed the new site's directories and copy the default theme so it
            // appears immediately in the site admin's "My Themes" view.
            let themes_dir   = state.config.themes_dir.clone();
            let sites_dir    = state.config.sites_dir.clone();
            let uploads_dir  = state.config.uploads_dir.clone();
            let site_id      = site.id;
            let site_hostname = hostname.clone();
            tokio::task::spawn_blocking(move || {
                // Create sites/{uuid}/themes/ and uploads/{uuid}/ directories.
                let site_themes_dir  = FsPath::new(&sites_dir).join(site_id.to_string()).join("themes");
                let site_uploads_dir = FsPath::new(&uploads_dir).join(site_id.to_string());
                if let Err(e) = std::fs::create_dir_all(&site_themes_dir) {
                    tracing::warn!(site_id = %site_id, "failed to create site themes dir: {}", e);
                }
                if let Err(e) = std::fs::create_dir_all(&site_uploads_dir) {
                    tracing::warn!(site_id = %site_id, "failed to create site uploads dir: {}", e);
                }
                // Create hostname symlink: uploads/{hostname} → {uuid} so Caddy's
                // file_server and the Axum uploads handler can serve files
                // without exposing the UUID in public URLs.
                crate::handlers::uploads::ensure_hostname_symlink(&uploads_dir, &site_hostname, site_id);
                // Copy the global default theme into sites/{uuid}/themes/default/.
                let src = FsPath::new(&themes_dir).join("global").join("default");
                let dst = site_themes_dir.join("default");
                if src.is_dir() && !dst.exists() {
                    if let Err(e) = copy_dir_all(&src, &dst) {
                        tracing::warn!(site_id = %site_id, "failed to seed default theme for new site: {}", e);
                    } else {
                        tracing::info!(site_id = %site_id, "seeded default theme for new site");
                    }
                }
            }).await.ok();

            if let Err(e) = state.reload_site_cache().await {
                tracing::warn!("site cache reload failed after create: {:?}", e);
            }
            Redirect::to("/admin/sites").into_response()
        }
        Err(e) => {
            let msg = if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                "A site with that hostname already exists.".to_string()
            } else {
                format!("Failed to create site: {e}")
            };
            let data = rebuild_new_site_data(&state, &admin, &form, &hostname).await;
            Html(admin::pages::sites::render_new(&data, Some(&msg), &ctx)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SwitchForm {
    pub site_id: String,
}

/// POST /admin/sites/switch — switch the current site in session.
/// site_admin can only switch to sites they are assigned to; super_admin can switch to any.
pub async fn switch(
    State(state): State<AppState>,
    admin: AdminUser,
    session: Session,
    Form(form): Form<SwitchForm>,
) -> impl IntoResponse {
    if let Ok(uuid) = form.site_id.parse::<Uuid>() {
        // For site_admin: verify they actually have a role on the target site.
        let allowed = if admin.caps.is_global_admin {
            true
        } else {
            crate::models::site_user::has_any_role(&state.db, uuid, admin.user.id)
                .await
                .unwrap_or(false)
        };
        if allowed {
            let _ = session.insert(SESSION_CURRENT_SITE_KEY, uuid.to_string()).await;
            // A different site can have a completely different role set for the
            // same user — any pinned role must not carry over.
            let _ = session.remove::<String>(SESSION_CURRENT_ROLE_KEY).await;
        } else {
            tracing::warn!("site_admin {} attempted to switch to unauthorised site {}", admin.user.id, uuid);
        }
    }
    Redirect::to("/admin")
}

/// GET /admin/sites/go-home — switch session back to the super admin's default site.
/// Accepts an optional `?next=/some/path` query param to control the redirect destination.
/// Defaults to /admin (dashboard). Used by the header badge (?next omitted) and the
/// sidebar email link (?next=/admin/profile).
pub async fn go_home(
    admin: AdminUser,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(default_site_id) = admin.user.default_site_id {
        let _ = session.insert(SESSION_CURRENT_SITE_KEY, default_site_id.to_string()).await;
        let _ = session.remove::<String>(SESSION_CURRENT_ROLE_KEY).await;
    }
    // Only allow relative paths starting with /admin to prevent open-redirect.
    let next = params.get("next")
        .filter(|p| p.starts_with("/admin"))
        .map(|p| p.as_str())
        .unwrap_or("/admin");
    Redirect::to(next)
}

/// GET /admin/sites/{id}/settings — edit site hostname.
pub async fn site_settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let flash = params.get("flash").map(|s| s.as_str());
    let cs = state.site_hostname(admin.site_id);
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let cfg = state.get_site_by_id(id)
        .map(|(_, s)| s)
        .unwrap_or_else(|| (*state.settings).clone());
    let admin_email_placeholder = crate::models::site::admin_email(&state.db, id).await
        .ok()
        .flatten()
        .unwrap_or_default();
    let maintenance_mode = crate::app_state::get_site_setting(&state.db, id, "maintenance_mode")
        .await
        .as_deref() == Some("true");
    let maintenance_message = crate::app_state::get_site_setting(&state.db, id, "maintenance_message")
        .await
        .unwrap_or_else(|| DEFAULT_MAINTENANCE_MESSAGE.to_string());
    let providers = crate::models::email_provider::list_for_site(&state.db, id).await.unwrap_or_default()
        .into_iter()
        .map(|p| {
            let config = crate::models::email_provider::decrypt_config(&state.config.secret_key, &p);
            let hint = config.as_ref().map(|c| c.display_hint());
            let field_placeholders = config
                .map(|c| c.field_placeholders().into_iter().map(|(k, v)| (k.to_string(), v)).collect())
                .unwrap_or_default();
            admin::pages::sites::EmailProviderSummary {
                id: p.id.to_string(),
                label: p.label,
                provider_type: p.provider_type,
                verified: p.verified,
                hint,
                field_placeholders,
            }
        })
        .collect();
    let data = SiteSettingsData {
        id: site.id.to_string(),
        hostname: site.hostname.clone(),
        site_name: cfg.site_name.clone(),
        site_description: cfg.site_description.clone(),
        language: cfg.language.clone(),
        posts_per_page: cfg.posts_per_page,
        date_format: cfg.date_format.clone(),
        admin_email: cfg.admin_email.clone().unwrap_or_default(),
        admin_email_placeholder,
        allow_registration: cfg.allow_registration,
        permalink_structure: cfg.permalink_structure.clone(),
        maintenance_mode,
        maintenance_message,
        providers,
    };
    Html(admin::pages::sites::render_settings(&data, flash, &ctx)).into_response()
}

/// POST /admin/sites/{id}/delete — delete a site.
/// super_admin can delete any site.
/// site_admin (owner) can delete their own site unless it is their default site.
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Fetch site to verify ownership and default status.
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };

    let is_owner = site.owner_user_id == Some(admin.user.id);
    let is_default = admin.user.default_site_id == Some(id);
    let allowed = admin.caps.is_global_admin || (is_owner && !is_default);

    if !allowed {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if let Err(e) = crate::models::site::delete(&state.db, id).await {
        tracing::error!("failed to delete site {}: {:?}", id, e);
    } else {
        tracing::info!(
            user_id = %admin.user.id,
            user_email = %admin.user.email,
            role = if admin.caps.is_global_admin { "super_admin" } else { "site_admin" },
            site_id = %id,
            hostname = %site.hostname,
            "site deleted",
        );
        // site_id is None here (not Some(id)) — the site row is already
        // gone by this point, so a per-site audit view scoped by site_id
        // could never surface its own deletion event anyway; this still
        // shows up in the global admin's unfiltered view via target_id.
        super::audit(&state, &admin, "site.deleted", "site", Some(id), &site.hostname, None).await;
        // Remove the site's data directory (themes + uploads) so no orphaned dirs accumulate.
        let site_data_dir = std::path::Path::new(&state.config.sites_dir).join(id.to_string());
        if site_data_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&site_data_dir) {
                tracing::warn!("failed to remove site data dir for site {}: {:?}", id, e);
            } else {
                tracing::info!("removed site data dir for deleted site {}", id);
            }
        }
        // Remove hostname symlink: uploads/{hostname} → uploads/{uuid}/
        let sym_path = std::path::Path::new(&state.config.uploads_dir).join(&site.hostname);
        if sym_path.is_symlink() {
            if let Err(e) = std::fs::remove_file(&sym_path) {
                tracing::warn!("failed to remove upload symlink for '{}': {:?}", site.hostname, e);
            }
        }
        // Also remove the site's upload subdirectory under uploads/{uuid}/.
        let site_upload_dir = std::path::Path::new(&state.config.uploads_dir).join(id.to_string());
        if site_upload_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&site_upload_dir) {
                tracing::warn!("failed to remove upload dir for site {}: {:?}", id, e);
            } else {
                tracing::info!("removed upload dir for deleted site {}", id);
            }
        }
        // Remove the site's plugin directory (plugins/sites/{id}/).
        // The site_plugins DB rows are cleaned up automatically via ON DELETE CASCADE.
        let site_plugin_dir = std::path::Path::new(&state.config.plugins_dir)
            .join("sites")
            .join(id.to_string());
        if site_plugin_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&site_plugin_dir) {
                tracing::warn!("failed to remove plugin dir for site {}: {:?}", id, e);
            } else {
                tracing::info!("removed plugin dir for deleted site {}", id);
            }
        }
        if let Err(e) = state.reload_site_cache().await {
            tracing::warn!("site cache reload failed after delete: {:?}", e);
        }
    }
    Redirect::to("/admin/sites").into_response()
}

#[derive(Deserialize)]
pub struct SiteConfigForm {
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
    #[serde(default)]
    pub admin_email: String,
    #[serde(default)]
    pub allow_registration: Option<String>,
    #[serde(default)]
    pub permalink_structure: String,
}

/// POST /admin/sites/{id}/site-config — save site name, description, language, etc.
pub async fn save_site_config(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SiteConfigForm>,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    // %postname% is what post-URL lookups key off of (see
    // handlers::page::try_post_permalink) — required, and must be the final
    // token so the "last segment = slug" resolution rule always holds.
    let permalink_structure = form.permalink_structure.trim();
    let valid_permalink = permalink_structure.ends_with("%postname%")
        || permalink_structure.ends_with("%postname%/");
    if !valid_permalink {
        return Redirect::to(&format!(
            "/admin/sites/{id}/settings?flash=Permalink+structure+must+end+with+%25postname%25&tab=general"
        )).into_response();
    }

    let allow_registration = if form.allow_registration.is_some() { "true" } else { "false" };
    let settings = [
        ("site_name", form.site_name.as_str()),
        ("site_description", form.site_description.as_str()),
        ("language", form.language.as_str()),
        ("date_format", form.date_format.as_str()),
        ("admin_email", form.admin_email.trim()),
        ("allow_registration", allow_registration),
        ("permalink_structure", permalink_structure),
    ];
    for (key, value) in &settings {
        if let Err(e) = crate::app_state::set_site_setting(&state.db, id, key, value).await {
            tracing::error!("failed to save site config '{}' for site {}: {:?}", key, id, e);
        }
    }
    let ppp = form.posts_per_page.to_string();
    if let Err(e) = crate::app_state::set_site_setting(&state.db, id, "posts_per_page", &ppp).await {
        tracing::error!("failed to save posts_per_page for site {}: {:?}", id, e);
    }

    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after site config save: {:?}", e);
    }

    Redirect::to(&format!("/admin/sites/{}/settings?flash=Saved.&tab=general", id)).into_response()
}

/// POST /admin/sites/{id}/maintenance — toggle maintenance mode for a site.
/// Checked live by core/src/middleware/maintenance.rs on every public request
/// to this site — takes effect immediately, no restart or cache reload needed.
pub async fn save_maintenance(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let enabled = form.contains_key("maintenance_mode");
    let trimmed = form.get("maintenance_message").map(|s| s.trim()).filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_MAINTENANCE_MESSAGE);
    let message: String = trimmed.chars().take(250).collect();
    let message = message.as_str();

    if let Err(e) = crate::app_state::set_site_setting(&state.db, id, "maintenance_mode", if enabled { "true" } else { "false" }).await {
        tracing::error!("failed to save maintenance_mode for site {}: {:?}", id, e);
    }
    if let Err(e) = crate::app_state::set_site_setting(&state.db, id, "maintenance_message", message).await {
        tracing::error!("failed to save maintenance_message for site {}: {:?}", id, e);
    }

    Redirect::to(&format!("/admin/sites/{}/settings?flash=Saved.&tab=maintenance", id)).into_response()
}

/// POST /admin/sites/{id}/provision-ssl
/// Appends a Caddy block for the site's hostname to the Caddyfile and reloads
/// Caddy so it begins provisioning a Let's Encrypt certificate.
/// Super-admin only; idempotent (no-op if the block already exists).
pub async fn provision_ssl(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.is_global_admin {
        return Redirect::to("/admin/sites?flash=Forbidden").into_response();
    }

    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s)  => s,
        Err(_) => return Redirect::to("/admin/sites?flash=Site+not+found").into_response(),
    };

    let caddyfile_path = &state.config.caddyfile_path;
    let hostname       = &site.hostname;

    let existing = match std::fs::read_to_string(caddyfile_path) {
        Ok(c)  => c,
        Err(e) => {
            tracing::error!("provision_ssl: cannot read {}: {:?}", caddyfile_path, e);
            return Redirect::to("/admin/sites?flash=Cannot+read+SSL+configuration").into_response();
        }
    };

    if caddy_block_exists(&existing, hostname) {
        return Redirect::to("/admin/sites?flash=SSL+already+active+for+this+site").into_response();
    }

    if !dns_points_here(hostname).await {
        tracing::info!(hostname = %hostname, "provision_ssl: DNS not pointing here yet, refusing to provision");
        return Redirect::to(
            "/admin/sites?flash=DNS+for+this+domain+doesn%27t+point+to+this+server+yet+%E2%80%94+wait+for+it+to+propagate+and+try+again"
        ).into_response();
    }

    let block = build_caddy_block(
        hostname,
        state.config.port,
        &state.config.uploads_dir,
    );
    let new_content = format!("{}\n{}\n", existing.trim_end(), block);

    if let Err(e) = std::fs::write(caddyfile_path, &new_content) {
        tracing::error!("provision_ssl: cannot write {}: {:?}", caddyfile_path, e);
        return Redirect::to("/admin/sites?flash=Cannot+write+SSL+configuration").into_response();
    }

    // Run caddy reload directly — no sudo needed since caddy reload just talks
    // to the Caddy admin API on localhost:2019. sudo is blocked by NoNewPrivileges.
    let result = std::process::Command::new("/usr/bin/caddy")
        .args([
            "reload",
            "--config", caddyfile_path,
            "--adapter", "caddyfile",
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            tracing::info!(hostname = %hostname, "SSL provisioned via Caddy");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::error!("provision_ssl: caddy reload failed: {}", stderr);
            return Redirect::to("/admin/sites?flash=Failed+to+enable+SSL%3A+check+server+logs").into_response();
        }
        Err(e) => {
            tracing::error!("provision_ssl: cannot run caddy reload: {:?}", e);
            return Redirect::to("/admin/sites?flash=Failed+to+enable+SSL%3A+check+server+logs").into_response();
        }
    }

    Redirect::to("/admin/sites?flash=SSL+provisioning+started+for+this+site").into_response()
}

/// Does `hostname` currently resolve (via the system DNS resolver, which also
/// honors /etc/hosts) to this machine's real, outward-routable IP?
///
/// Deliberately does NOT accept loopback as a match: real ACME/Let's Encrypt
/// issuance (what provisioning actually triggers) can never succeed for a
/// loopback-only domain — it needs a publicly reachable server. A local dev
/// machine's /etc/hosts entries pointing at 127.0.0.1 are for the catch-all
/// `tls internal` Caddy block, not for real certificate issuance, so those
/// domains should correctly be reported as "not pointing here yet."
async fn dns_points_here(hostname: &str) -> bool {
    let resolved: Vec<std::net::IpAddr> = match tokio::net::lookup_host((hostname, 0)).await {
        Ok(addrs) => addrs.map(|a| a.ip()).collect(),
        Err(e) => {
            tracing::info!(hostname = %hostname, error = %e, "dns_points_here: lookup failed");
            return false;
        }
    };
    if resolved.is_empty() {
        return false;
    }
    let local_ip = outbound_local_ip();
    resolved.iter().any(|ip| Some(*ip) == local_ip)
}

/// This machine's IP on the interface the OS would use to reach the public
/// internet. Uses the standard "connect a UDP socket, read back local_addr"
/// trick — connect() on UDP just resolves routing locally, no packets sent.
fn outbound_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

/// Returns true if the Caddyfile already contains a block for `hostname`.
/// Matches lines where the hostname is the sole token before `{` (bare domain blocks).
pub fn caddy_block_exists(caddyfile: &str, hostname: &str) -> bool {
    caddyfile.lines().any(|line| {
        let t = line.trim();
        t == hostname
            || t.starts_with(&format!("{} ", hostname))
            || t.starts_with(&format!("{},", hostname))
            || t.starts_with(&format!("{}{{", hostname))
    })
}

/// Build the Caddyfile block to append for a new site.
///
/// `/theme/static/*` deliberately has no theme name in the URL — which
/// theme's files get served is resolved per-request in
/// `handlers/theme_static.rs` (Host header -> site -> active theme). A flat
/// Caddy file_server can't do that resolution, so `/theme/*` must fall
/// through to `reverse_proxy` -> Axum, not be handled here. See
/// deployment/Caddyfile.template for the same rule.
fn build_caddy_block(hostname: &str, port: u16, uploads_dir: &str) -> String {
    format!(
        r#"{hostname} {{
    # Serve uploads directly — bypass Axum — but ONLY the bare-filename shape
    # (/uploads/{{filename}}, what public pages use via Media::url()). Rooted
    # at THIS site's own uploads/{hostname}/ -> uploads/{{site-uuid}}/ symlink
    # (the app maintains one per site), so a bare filename resolves with no
    # need to repeat the hostname in the path.
    #
    # The admin media UI instead builds UUID-prefixed URLs
    # (/uploads/{{site-uuid}}/{{filename}}), since admin can be browsed via a
    # host that isn't this site's own domain (e.g. a shared dev host).
    # Matching bare filenames only (path_regexp, no further `/` after the
    # filename) means anything with more path segments — including that
    # UUID-prefixed shape — falls through to reverse_proxy -> Axum below,
    # whose handlers/uploads.rs already resolves that shape correctly. A
    # blanket /uploads/* match here would otherwise double up the site's own
    # directory with the UUID segment and 404 every admin-uploaded image.
    # See deployment/Caddyfile.template for the same rule.
    @upload_file {{
        path_regexp ^/uploads/[^/]+$
    }}
    handle @upload_file {{
        uri strip_prefix /uploads
        root * {uploads_dir}/{hostname}
        file_server
    }}

    reverse_proxy localhost:{port}

    encode zstd gzip

    header {{
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "SAMEORIGIN"
        Referrer-Policy "strict-origin-when-cross-origin"
        -Server
    }}

    log {{
        output file /var/log/caddy/{hostname}.log
        format json
    }}
}}"#,
        hostname    = hostname,
        port        = port,
        uploads_dir = uploads_dir,
    )
}
