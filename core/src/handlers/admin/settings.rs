use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Form,
};
use std::collections::HashMap;

use crate::app_state::{set_app_setting, AppState};
use crate::middleware::admin_auth::AdminUser;

pub async fn settings(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let (app_name, timezone) = {
        let s = state.app_settings.read().unwrap();
        (s.app_name.clone(), s.timezone.clone())
    };
    let admin_email = state.config.admin_email.clone().unwrap_or_default();
    Html(admin::pages::settings::render(None, &app_name, &timezone, &admin_email, &ctx)).into_response()
}

pub async fn save_settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, Html("Forbidden".to_string())).into_response();
    }

    let tab = form.get("tab").map(|s| s.as_str()).unwrap_or("general");
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let admin_email = state.config.admin_email.clone().unwrap_or_default();

    if tab == "general" {
        let app_name = form.get("app_name").map(|s| s.trim()).unwrap_or("Synaptic");
        let timezone = form.get("timezone").map(|s| s.trim()).unwrap_or("UTC");

        let mut error: Option<String> = None;
        if let Err(e) = set_app_setting(&state.db, "app_name", app_name).await {
            tracing::error!("failed to save app_name: {}", e);
            error = Some("Failed to save settings. Please try again.".to_string());
        }
        if error.is_none() {
            if let Err(e) = set_app_setting(&state.db, "timezone", timezone).await {
                tracing::error!("failed to save timezone: {}", e);
                error = Some("Failed to save settings. Please try again.".to_string());
            }
        }

        if error.is_none() {
            if let Err(e) = state.reload_app_settings().await {
                tracing::warn!("failed to reload app_settings cache: {}", e);
            }
        }

        let (a_name, tz) = {
            let s = state.app_settings.read().unwrap();
            (s.app_name.clone(), s.timezone.clone())
        };
        let flash = error.as_deref().unwrap_or("General settings saved.");
        return Html(admin::pages::settings::render(Some(flash), &a_name, &tz, &admin_email, &ctx)).into_response();
    }

    // Non-general tabs — just re-render with no change.
    let (app_name, timezone) = {
        let s = state.app_settings.read().unwrap();
        (s.app_name.clone(), s.timezone.clone())
    };
    Html(admin::pages::settings::render(None, &app_name, &timezone, &admin_email, &ctx)).into_response()
}
