//! Admin handlers for site management (list, create, switch, settings).

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::{AdminUser, SESSION_CURRENT_SITE_KEY};
use admin::pages::sites::{SiteRow, SiteSettingsData};
use tower_sessions::Session;

/// GET /admin/sites — list sites.
/// super_admin sees all sites (can manage all).
/// site_admin sees owned sites (can manage) plus sites they're assigned to (switch only).
/// editors/authors see only sites they're assigned to (switch only).
pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    // Require at minimum a logged-in admin user; subscribers/unauthenticated are blocked by AdminUser extractor.
    // All roles that reach here may view the page.
    let cs = state.site_hostname(admin.site_id);
    let can_create = admin.caps.can_manage_sites;

    // Build site list with per-row manage flag.
    let mut rows: Vec<SiteRow> = Vec::new();

    if admin.caps.is_global_admin {
        let sites = crate::models::site::list(&state.db).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list sites: {:?}", e);
            vec![]
        });
        for s in &sites {
            let post_count = crate::models::site::post_count(&state.db, s.id).await.unwrap_or(0);
            rows.push(SiteRow {
                id: s.id.to_string(),
                hostname: s.hostname.clone(),
                post_count,
                is_default: admin.user.default_site_id == Some(s.id),
                can_manage: true,
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
            let post_count = crate::models::site::post_count(&state.db, s.id).await.unwrap_or(0);
            // can_manage if they are the owner of this site
            let can_manage = s.owner_user_id == Some(admin.user.id);
            rows.push(SiteRow {
                id: s.id.to_string(),
                hostname: s.hostname.clone(),
                post_count,
                is_default: admin.user.default_site_id == Some(s.id),
                can_manage,
            });
            let _ = site_role; // used for future display
        }
    }

    let ctx = super::page_ctx(&admin, &cs);
    Html(admin::pages::sites::render_list(&rows, None, can_create, &ctx))
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
    let ctx = super::page_ctx(&admin, &cs);
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
    let ctx = super::page_ctx(&admin, &cs);
    let hostname = form.hostname.trim().to_lowercase();
    if hostname.is_empty() {
        return Html(admin::pages::sites::render_new(Some("Hostname cannot be empty."), &ctx)).into_response();
    }

    let result = crate::models::site::create_with_defaults(&state.db, &hostname, admin.user.id)
        .await
        .map(|_| ());

    match result {
        Ok(_) => {
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
    if !admin.caps.is_global_admin && !is_owner {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let ctx = super::page_ctx(&admin, &cs);
    let data = SiteSettingsData {
        id: site.id.to_string(),
        hostname: site.hostname.clone(),
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
    if !admin.caps.is_global_admin && !is_owner {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx(&admin, &cs);
    let hostname = form.hostname.trim().to_lowercase();
    if hostname.is_empty() {
        let data = SiteSettingsData { id: id.to_string(), hostname: String::new() };
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
            let data = SiteSettingsData { id: id.to_string(), hostname };
            Html(admin::pages::sites::render_settings(&data, Some(&msg), &ctx)).into_response()
        }
    }
}

/// POST /admin/sites/{id}/delete — delete a site.
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.is_global_admin {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if let Err(e) = crate::models::site::delete(&state.db, id).await {
        tracing::error!("failed to delete site {}: {:?}", id, e);
    } else if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after delete: {:?}", e);
    }
    Redirect::to("/admin/sites").into_response()
}
