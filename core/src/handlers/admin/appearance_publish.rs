//! Copying themes between library tiers: global/private → site ("Get Theme")
//! and private → global ("Publish"). Split out of appearance.rs, which also
//! owns the theme list/activate/delete/screenshot handlers.

use axum::{
    extract::{State, Form},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use std::fs;
use std::path::Path as FsPath;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

use super::appearance::{copy_dir_all, render_appearance_list};

// ── Get Theme (copy global → site, no activation) ─────────────────────────────

#[derive(Deserialize)]
pub struct GetThemeForm {
    pub theme: String,
    /// "global" or "private" — tells the handler which directory to copy from.
    pub source: Option<String>,
}

pub async fn get_theme(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<GetThemeForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let name = form.theme.trim().to_string();
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return render_appearance_list(&state, Some("Invalid theme name."), &ctx, admin.site_id, "global")
            .await.into_response();
    }

    let themes_dir = &state.config.themes_dir;

    // Determine source directory. Only super_admin may get private themes.
    let from_private = form.source.as_deref() == Some("private") && admin.caps.is_global_admin;
    let source = if from_private {
        FsPath::new(themes_dir).join("private").join(&name)
    } else {
        FsPath::new(themes_dir).join("global").join(&name)
    };
    let return_filter = if from_private { "private" } else { "global" };

    if !source.is_dir() {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, return_filter)
            .await.into_response();
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            return render_appearance_list(
                &state, Some("No site selected."),
                &ctx, admin.site_id, "global",
            ).await.into_response();
        }
    };

    let dest = FsPath::new(&state.config.sites_dir).join(site_id.to_string()).join("themes").join(&name);
    if dest.exists() {
        // Already copied — just send them to their themes.
        return Redirect::to("/admin/appearance?filter=my").into_response();
    }

    let source_owned = source.to_path_buf();
    let dest_owned = dest.to_path_buf();
    match tokio::task::spawn_blocking(move || copy_dir_all(&source_owned, &dest_owned)).await {
        Ok(Ok(())) => {
            tracing::info!("get_theme: copied '{}' ({}) to site {}", name, return_filter, site_id);
            Redirect::to("/admin/appearance?filter=my").into_response()
        }
        Ok(Err(e)) => {
            tracing::error!("get_theme: copy failed for '{}': {}", name, e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            render_appearance_list(&state, Some("Failed to get theme. Please try again."), &ctx, admin.site_id, return_filter)
                .await.into_response()
        }
        Err(e) => {
            tracing::error!("get_theme: task panicked: {:?}", e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            render_appearance_list(&state, Some("Failed to get theme. Please try again."), &ctx, admin.site_id, return_filter)
                .await.into_response()
        }
    }
}

// ── Publish Theme (copy private → global) ─────────────────────────────────────

#[derive(Deserialize)]
pub struct PublishThemeForm {
    pub theme: String,
}

pub async fn publish_theme(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PublishThemeForm>,
) -> impl IntoResponse {
    // Only super_admin may publish themes to the global library.
    if !admin.caps.is_global_admin {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let name = form.theme.trim().to_string();
    let cs = state.site_hostname(admin.site_id);

    macro_rules! err {
        ($msg:expr) => {{
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            return render_appearance_list(&state, Some($msg), &ctx, admin.site_id, "private")
                .await
                .into_response();
        }};
    }

    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        err!("Invalid theme name.");
    }

    let themes_dir = &state.config.themes_dir;
    let private_path = FsPath::new(themes_dir).join("private").join(&name);
    let global_path  = FsPath::new(themes_dir).join("global").join(&name);

    if !private_path.is_dir() {
        err!("Private theme not found.");
    }

    // If a global copy exists, remove it first (caller confirmed via JS).
    if global_path.exists() {
        if let Err(e) = fs::remove_dir_all(&global_path) {
            tracing::error!("publish_theme: failed to remove existing global copy '{}': {:?}", name, e);
            err!("Failed to overwrite existing global theme. Please try again.");
        }
    }

    let src = private_path.to_path_buf();
    let dst = global_path.to_path_buf();
    match tokio::task::spawn_blocking(move || copy_dir_all(&src, &dst)).await {
        Ok(Ok(())) => {
            tracing::info!("publish_theme: '{}' published to global by super_admin", name);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            render_appearance_list(
                &state,
                Some(&format!("Theme '{}' is now in the global library.", name)),
                &ctx,
                admin.site_id,
                "private",
            )
            .await
            .into_response()
        }
        Ok(Err(e)) => {
            tracing::error!("publish_theme: copy failed for '{}': {}", name, e);
            err!("Failed to publish theme. Please try again.");
        }
        Err(e) => {
            tracing::error!("publish_theme: task panicked: {:?}", e);
            err!("Failed to publish theme. Please try again.");
        }
    }
}
