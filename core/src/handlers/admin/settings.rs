use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::settings::SettingsData;

pub async fn settings(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    // If the admin is scoped to a site, show that site's settings; otherwise use global fallback.
    let s = admin.site_id
        .and_then(|sid| state.get_site_by_id(sid))
        .map(|(_, settings)| settings)
        .unwrap_or_else(|| (*state.settings).clone());
    let data = SettingsData {
        site_name: s.site_name.clone(),
        site_description: s.site_description.clone(),
        language: s.language.clone(),
        posts_per_page: s.posts_per_page,
        date_format: s.date_format.clone(),
    };
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx(&admin, &cs);
    Html(admin::pages::settings::render(&data, None, &ctx)).into_response()
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

pub async fn save_settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<SettingsForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            tracing::warn!("save_settings: no site selected, cannot save per-site settings");
            return Redirect::to("/admin/settings").into_response();
        }
    };

    let settings = [
        ("site_name", form.site_name.as_str()),
        ("site_description", form.site_description.as_str()),
        ("language", form.language.as_str()),
        ("date_format", form.date_format.as_str()),
    ];

    for (key, value) in &settings {
        if let Err(e) = crate::app_state::set_site_setting(&state.db, site_id, key, value).await {
            tracing::error!("failed to save setting '{}': {:?}", key, e);
        }
    }
    let ppp = form.posts_per_page.to_string();
    if let Err(e) = crate::app_state::set_site_setting(&state.db, site_id, "posts_per_page", &ppp).await {
        tracing::error!("failed to save setting 'posts_per_page': {:?}", e);
    }

    if let Err(e) = state.reload_site_cache().await {
        tracing::error!("failed to reload site cache after settings save: {:?}", e);
    }

    Redirect::to("/admin/settings").into_response()
}
