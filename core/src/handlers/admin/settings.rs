use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::settings::SettingsData;

pub async fn settings(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    let s = state.settings.as_ref();
    let data = SettingsData {
        site_name: s.site_name.clone(),
        site_description: s.site_description.clone(),
        base_url: s.base_url.clone(),
        language: s.language.clone(),
        posts_per_page: s.posts_per_page,
        date_format: s.date_format.clone(),
    };
    Html(admin::pages::settings::render(&data, None))
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub site_name: String,
    pub site_description: String,
    pub base_url: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

pub async fn save_settings(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<SettingsForm>,
) -> impl IntoResponse {
    let settings = [
        ("site_name", form.site_name.as_str()),
        ("site_description", form.site_description.as_str()),
        ("base_url", form.base_url.as_str()),
        ("language", form.language.as_str()),
        ("date_format", form.date_format.as_str()),
    ];

    for (key, value) in &settings {
        if let Err(e) = crate::app_state::set_site_setting(&state.db, key, value).await {
            tracing::error!("failed to save setting '{}': {:?}", key, e);
        }
    }
    let ppp = form.posts_per_page.to_string();
    if let Err(e) = crate::app_state::set_site_setting(&state.db, "posts_per_page", &ppp).await {
        tracing::error!("failed to save setting 'posts_per_page': {:?}", e);
    }

    Redirect::to("/admin/settings")
}
