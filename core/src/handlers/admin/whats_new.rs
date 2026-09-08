//! GET /admin/whats-new — in-app "what's new" page, linked from the
//! dashboard's update-available notice (`handlers::admin::dashboard`).

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::whats_new::WhatsNewData;

pub async fn show(State(state): State<AppState>, admin: AdminUser) -> impl IntoResponse {
    if !admin.caps.is_global_admin {
        return (
            StatusCode::FORBIDDEN,
            Html("<h1>403 Forbidden</h1>".to_string()),
        )
            .into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let latest = state.latest_release.read().ok().and_then(|r| r.clone());
    let update_available = latest
        .as_ref()
        .map(|r| r.tag_name != state.current_version)
        .unwrap_or(false)
        && !crate::version::is_source_build(&state.current_version);

    let data = WhatsNewData {
        current_version: state.current_version.clone(),
        latest_tag: latest.as_ref().map(|r| r.tag_name.clone()),
        latest_url: latest.as_ref().map(|r| r.html_url.clone()),
        latest_body: latest.as_ref().map(|r| r.body.clone()),
        can_self_update: update_available
            && !admin.caps.is_impersonating
            && state.config.self_update_enabled,
    };

    Html(admin::pages::whats_new::render(&data, &ctx)).into_response()
}
