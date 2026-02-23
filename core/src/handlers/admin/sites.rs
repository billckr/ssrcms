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

/// GET /admin/sites — list all sites.
pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let sites = crate::models::site::list(&state.db).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list sites: {:?}", e);
        vec![]
    });

    let mut rows = Vec::with_capacity(sites.len());
    for s in &sites {
        let post_count = crate::models::site::post_count(&state.db, s.id).await.unwrap_or(0);
        rows.push(SiteRow {
            id: s.id.to_string(),
            hostname: s.hostname.clone(),
            post_count,
        });
    }

    Html(admin::pages::sites::render_list(&rows, None, &cs))
}

/// GET /admin/sites/new — new site form.
pub async fn new_site(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    Html(admin::pages::sites::render_new(None, &cs))
}

#[derive(Deserialize)]
pub struct NewSiteForm {
    pub hostname: String,
}

/// POST /admin/sites — create a new site.
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<NewSiteForm>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let hostname = form.hostname.trim().to_lowercase();
    if hostname.is_empty() {
        return Html(admin::pages::sites::render_new(Some("Hostname cannot be empty."), &cs)).into_response();
    }
    match crate::models::site::create(&state.db, &hostname).await {
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
            Html(admin::pages::sites::render_new(Some(&msg), &cs)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SwitchForm {
    pub site_id: String,
}

/// POST /admin/sites/switch — switch the current site in session.
pub async fn switch(
    _admin: AdminUser,
    session: Session,
    Form(form): Form<SwitchForm>,
) -> impl IntoResponse {
    if let Ok(uuid) = form.site_id.parse::<Uuid>() {
        let _ = session.insert(SESSION_CURRENT_SITE_KEY, uuid.to_string()).await;
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
    match crate::models::site::get_by_id(&state.db, id).await {
        Ok(site) => {
            let data = SiteSettingsData {
                id: site.id.to_string(),
                hostname: site.hostname.clone(),
            };
            Html(admin::pages::sites::render_settings(&data, None, &cs)).into_response()
        }
        Err(_) => Redirect::to("/admin/sites").into_response(),
    }
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
    let cs = state.site_hostname(admin.site_id);
    let hostname = form.hostname.trim().to_lowercase();
    if hostname.is_empty() {
        let data = SiteSettingsData { id: id.to_string(), hostname: String::new() };
        return Html(admin::pages::sites::render_settings(&data, Some("Hostname cannot be empty."), &cs)).into_response();
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
            Html(admin::pages::sites::render_settings(&data, Some(&msg), &cs)).into_response()
        }
    }
}

/// POST /admin/sites/{id}/delete — delete a site.
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = crate::models::site::delete(&state.db, id).await {
        tracing::error!("failed to delete site {}: {:?}", id, e);
    } else if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after delete: {:?}", e);
    }
    Redirect::to("/admin/sites")
}
