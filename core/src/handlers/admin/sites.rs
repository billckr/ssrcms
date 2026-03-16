//! Admin handlers for site management (list, create, switch, settings).

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
use crate::middleware::admin_auth::{AdminUser, SESSION_CURRENT_SITE_KEY};
use crate::handlers::admin::appearance::copy_dir_all;
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
            let (admin_email, user_count, subscriber_count, post_count, page_count) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
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
            let (admin_email, user_count, subscriber_count, post_count, page_count) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
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
            });
        }
    } else {
        // Non-global-admin: fetch all sites the user has any role on.
        let site_roles = crate::models::site_user::list_for_user(&state.db, admin.user.id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("failed to list sites for user {}: {:?}", admin.user.id, e);
                vec![]
            });
        for (s, site_role) in &site_roles {
            let (admin_email, user_count, subscriber_count, post_count, page_count) = tokio::join!(
                crate::models::site::admin_email(&state.db, s.id),
                crate::models::site::user_count(&state.db, s.id),
                crate::models::site::subscriber_count(&state.db, s.id),
                crate::models::site::post_count(&state.db, s.id),
                crate::models::site::page_count(&state.db, s.id),
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
            });
        }
    }

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::sites::render_list(&rows, flash, can_create, &ctx))
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
    Html(admin::pages::sites::render_new(None, &ctx))
}

#[derive(Deserialize)]
pub struct NewSiteForm {
    pub hostname: String,
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
        return Html(admin::pages::sites::render_new(Some("Hostname cannot be empty."), &ctx)).into_response();
    }
    if !is_valid_hostname(&hostname) {
        return Html(admin::pages::sites::render_new(
            Some("Must be a valid domain (e.g. example.com, my-site.com, sub.example.com)."),
            &ctx,
        )).into_response();
    }

    // When impersonating (super_admin visiting a foreign site), assign the new
    // site to that site's owner rather than to the super admin's own account.
    let owner_id = if admin.caps.is_impersonating {
        if let Some(sid) = admin.site_id {
            sqlx::query_scalar::<_, Option<uuid::Uuid>>(
                "SELECT owner_user_id FROM sites WHERE id = $1",
            )
            .bind(sid)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .flatten()
            .unwrap_or(admin.user.id)
        } else {
            admin.user.id
        }
    } else {
        admin.user.id
    };

    let result = crate::models::site::create_with_defaults(&state.db, &hostname, owner_id)
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

            // Seed the new site's directories and copy the default theme so it
            // appears immediately in the site admin's "My Themes" view.
            let themes_dir = state.config.themes_dir.clone();
            let sites_dir  = state.config.sites_dir.clone();
            let uploads_dir = state.config.uploads_dir.clone();
            let site_id = site.id;
            tokio::task::spawn_blocking(move || {
                // Create sites/{uuid}/themes/ and uploads/{uuid}/ directories.
                let site_themes_dir = FsPath::new(&sites_dir).join(site_id.to_string()).join("themes");
                let site_uploads_dir = FsPath::new(&uploads_dir).join(site_id.to_string());
                if let Err(e) = std::fs::create_dir_all(&site_themes_dir) {
                    tracing::warn!(site_id = %site_id, "failed to create site themes dir: {}", e);
                }
                if let Err(e) = std::fs::create_dir_all(&site_uploads_dir) {
                    tracing::warn!(site_id = %site_id, "failed to create site uploads dir: {}", e);
                }
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
            Html(admin::pages::sites::render_new(Some(&msg), &ctx)).into_response()
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
            crate::models::site_user::get_role(&state.db, uuid, admin.user.id)
                .await
                .ok()
                .flatten()
                .is_some()
        };
        if allowed {
            let _ = session.insert(SESSION_CURRENT_SITE_KEY, uuid.to_string()).await;
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
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    let is_owner = site.owner_user_id == Some(admin.user.id);
    let has_role = matches!(
        crate::models::site_user::get_role(&state.db, id, admin.user.id)
            .await.ok().flatten().as_deref(),
        Some("admin" | "site_admin")
    );
    if !admin.caps.is_global_admin && !is_owner && !has_role {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let cfg = state.get_site_by_id(id)
        .map(|(_, s)| s)
        .unwrap_or_else(|| (*state.settings).clone());
    let data = SiteSettingsData {
        id: site.id.to_string(),
        hostname: site.hostname.clone(),
        site_name: cfg.site_name.clone(),
        site_description: cfg.site_description.clone(),
        language: cfg.language.clone(),
        posts_per_page: cfg.posts_per_page,
        date_format: cfg.date_format.clone(),
    };
    Html(admin::pages::sites::render_settings(&data, None, &ctx)).into_response()
}

#[derive(Deserialize)]
pub struct SiteSettingsForm {
    pub hostname: String,
}

/// POST /admin/sites/{id}/settings — save site hostname.
pub async fn save_site_settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SiteSettingsForm>,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    let is_owner = site.owner_user_id == Some(admin.user.id);
    let has_role = matches!(
        crate::models::site_user::get_role(&state.db, id, admin.user.id)
            .await.ok().flatten().as_deref(),
        Some("admin" | "site_admin")
    );
    if !admin.caps.is_global_admin && !is_owner && !has_role {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let hostname = form.hostname.trim().to_lowercase();

    // Helper: load current site config for error re-renders.
    let cfg = state.get_site_by_id(id)
        .map(|(_, s)| s)
        .unwrap_or_else(|| (*state.settings).clone());

    if hostname.is_empty() {
        let data = SiteSettingsData {
            id: id.to_string(),
            hostname: String::new(),
            site_name: cfg.site_name.clone(),
            site_description: cfg.site_description.clone(),
            language: cfg.language.clone(),
            posts_per_page: cfg.posts_per_page,
            date_format: cfg.date_format.clone(),
        };
        return Html(admin::pages::sites::render_settings(&data, Some("Hostname cannot be empty."), &ctx)).into_response();
    }

    let result = sqlx::query(
        "UPDATE sites SET hostname = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&hostname)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Keep site_url in sync with the new hostname.
            // site_url is always derived as http://{hostname} — port/https
            // overrides are a super_admin concern handled via CLI or direct DB.
            let derived_url = format!("http://{}", hostname);
            let _ = sqlx::query(
                "INSERT INTO site_settings (site_id, key, value)
                 VALUES ($1, 'site_url', $2)
                 ON CONFLICT (site_id, key) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(id)
            .bind(&derived_url)
            .execute(&state.db)
            .await;

            if let Err(e) = state.reload_site_cache().await {
                tracing::warn!("site cache reload failed after settings save: {:?}", e);
            }
            Redirect::to("/admin/sites").into_response()
        }
        Err(e) => {
            let msg = if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                "A site with that hostname already exists.".to_string()
            } else {
                format!("Failed to save: {e}")
            };
            let data = SiteSettingsData {
                id: id.to_string(),
                hostname,
                site_name: cfg.site_name.clone(),
                site_description: cfg.site_description.clone(),
                language: cfg.language.clone(),
                posts_per_page: cfg.posts_per_page,
                date_format: cfg.date_format.clone(),
            };
            Html(admin::pages::sites::render_settings(&data, Some(&msg), &ctx)).into_response()
        }
    }
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
        // Remove the site's data directory (themes + uploads) so no orphaned dirs accumulate.
        let site_data_dir = std::path::Path::new(&state.config.sites_dir).join(id.to_string());
        if site_data_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&site_data_dir) {
                tracing::warn!("failed to remove site data dir for site {}: {:?}", id, e);
            } else {
                tracing::info!("removed site data dir for deleted site {}", id);
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
    let is_owner = site.owner_user_id == Some(admin.user.id);
    let has_role = matches!(
        crate::models::site_user::get_role(&state.db, id, admin.user.id)
            .await.ok().flatten().as_deref(),
        Some("admin" | "site_admin")
    );
    if !admin.caps.is_global_admin && !is_owner && !has_role {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let settings = [
        ("site_name", form.site_name.as_str()),
        ("site_description", form.site_description.as_str()),
        ("language", form.language.as_str()),
        ("date_format", form.date_format.as_str()),
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

    Redirect::to(&format!("/admin/sites/{}/settings", id)).into_response()
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
            return Redirect::to("/admin/sites?flash=Cannot+read+Caddyfile").into_response();
        }
    };

    if caddy_block_exists(&existing, hostname) {
        return Redirect::to("/admin/sites?flash=SSL+already+active+for+this+site").into_response();
    }

    let block = build_caddy_block(
        hostname,
        state.config.port,
        &state.config.uploads_dir,
        &state.config.themes_dir,
    );
    let new_content = format!("{}\n{}\n", existing.trim_end(), block);

    if let Err(e) = std::fs::write(caddyfile_path, &new_content) {
        tracing::error!("provision_ssl: cannot write {}: {:?}", caddyfile_path, e);
        return Redirect::to("/admin/sites?flash=Cannot+write+Caddyfile").into_response();
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
            return Redirect::to("/admin/sites?flash=Caddy+reload+failed%3A+check+server+logs").into_response();
        }
        Err(e) => {
            tracing::error!("provision_ssl: cannot run caddy reload: {:?}", e);
            return Redirect::to("/admin/sites?flash=Cannot+run+caddy+reload").into_response();
        }
    }

    Redirect::to("/admin/sites?flash=SSL+provisioning+started+for+this+site").into_response()
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
fn build_caddy_block(hostname: &str, port: u16, uploads_dir: &str, themes_dir: &str) -> String {
    format!(
        r#"{hostname} {{
    handle /uploads/* {{
        root * {uploads_dir}
        file_server
    }}

    handle /theme/* {{
        root * {themes_dir}
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
        themes_dir  = themes_dir,
    )
}
