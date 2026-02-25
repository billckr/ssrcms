use axum::{extract::State, http::StatusCode, response::{Html, IntoResponse}};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::plugins::{PluginRow, render};

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let rows: Vec<PluginRow> = state.loaded_plugins.iter().map(|m| {
        let mut hooks: Vec<(String, String)> = m.hooks
            .iter()
            .map(|(hook, tmpl)| (hook.clone(), tmpl.clone()))
            .collect();
        hooks.sort_by(|a, b| a.0.cmp(&b.0));

        let mut routes: Vec<String> = m.routes.keys().cloned().collect();
        routes.sort();

        let mut meta_fields: Vec<String> = m.meta_fields.keys().cloned().collect();
        meta_fields.sort();

        PluginRow {
            name: m.plugin.name.clone(),
            version: m.plugin.version.clone(),
            api_version: m.plugin.api_version.clone(),
            author: m.plugin.author.clone(),
            description: m.plugin.description.clone(),
            hooks,
            routes,
            meta_fields,
        }
    }).collect();

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(render(&rows, &ctx)).into_response()
}
