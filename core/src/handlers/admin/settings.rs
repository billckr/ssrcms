use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use std::collections::HashMap;

use crate::app_state::{set_app_setting, AppState};
use crate::middleware::admin_auth::AdminUser;

pub async fn settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let flash = params.get("flash").map(|s| s.as_str());
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let (app_name, timezone, max_upload_mb) = {
        let s = state.app_settings.read().unwrap();
        (s.app_name.clone(), s.timezone.clone(), s.max_upload_mb.max(0) as u64)
    };
    let sites: Vec<(uuid::Uuid, String)> = crate::models::site::list(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.id, s.hostname))
        .collect();
    Html(admin::pages::settings::render(flash, &app_name, &timezone, max_upload_mb, &sites, &ctx)).into_response()
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

    if tab == "general" {
        let app_name = form.get("app_name").map(|s| s.trim()).unwrap_or("Synaptic");

        let mut error: Option<String> = None;
        if let Err(e) = set_app_setting(&state.db, "app_name", app_name).await {
            tracing::error!("failed to save app_name: {}", e);
            error = Some("Failed to save settings. Please try again.".to_string());
        }

        if error.is_none() {
            if let Err(e) = state.reload_app_settings().await {
                tracing::warn!("failed to reload app_settings cache: {}", e);
            }
        }

        let flash = error.as_deref().unwrap_or("General settings saved.");
        return Redirect::to(&format!("/admin/settings?flash={}", flash.replace(' ', "+"))).into_response();
    }

    if tab == "localisation" {
        let timezone = form.get("timezone").map(|s| s.trim()).unwrap_or("UTC");

        let mut error: Option<String> = None;
        if let Err(e) = set_app_setting(&state.db, "timezone", timezone).await {
            tracing::error!("failed to save timezone: {}", e);
            error = Some("Failed to save settings. Please try again.".to_string());
        }

        if error.is_none() {
            if let Err(e) = state.reload_app_settings().await {
                tracing::warn!("failed to reload app_settings cache: {}", e);
            }
        }

        let flash = error.as_deref().unwrap_or("Localisation settings saved.");
        return Redirect::to(&format!("/admin/settings?flash={}", flash.replace(' ', "+"))).into_response();
    }

    if tab == "uploads" {
        let max_upload_mb: Option<i64> = form.get("max_upload_mb").and_then(|s| s.trim().parse().ok());

        let flash = match max_upload_mb {
            Some(mb) if mb >= 1 && mb <= 1000 => {
                match set_app_setting(&state.db, "max_upload_mb", &mb.to_string()).await {
                    Ok(()) => {
                        if let Err(e) = state.reload_app_settings().await {
                            tracing::warn!("failed to reload app_settings cache: {}", e);
                        }
                        "Upload settings saved."
                    }
                    Err(e) => {
                        tracing::error!("failed to save max_upload_mb: {}", e);
                        "Failed to save settings. Please try again."
                    }
                }
            }
            _ => "Max upload size must be between 1 and 1000 MB.",
        };
        return Redirect::to(&format!("/admin/settings?flash={}", flash.replace(' ', "+"))).into_response();
    }

    Redirect::to("/admin/settings").into_response()
}
