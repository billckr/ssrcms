//! Dynamic theme static file handler.
//!
//! Serves files from `themes/{active_theme}/static/` at the `/theme/static/*path` route.
//! Reads the active theme from AppState on every request so theme switches are reflected
//! immediately without a restart.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

use crate::app_state::AppState;

pub async fn serve(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    let active_theme = state.active_theme.read().unwrap().clone();

    // Resolve theme directory: global/, then sites/*/, then legacy flat layout.
    let static_base = if let Some(theme_dir) = state.templates.resolve_theme_dir(&active_theme) {
        theme_dir.join("static")
    } else {
        // Fallback for legacy flat layout (themes/<name>/static/).
        std::path::Path::new(&state.config.themes_dir)
            .join(&active_theme)
            .join("static")
    };

    tracing::debug!("theme_static: serving '{}' from theme '{}'", path, active_theme);

    let requested = static_base.join(&path);

    // Canonicalize the base dir first; if the theme doesn't exist, 404.
    let canonical_base = match static_base.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    // Canonicalize the requested path; non-existent files return 404.
    let canonical_file = match requested.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    // Path traversal guard: the resolved file must stay inside the static dir.
    if !canonical_file.starts_with(&canonical_base) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Only serve files, not directories.
    if canonical_file.is_dir() {
        return StatusCode::NOT_FOUND.into_response();
    }

    match tokio::fs::read(&canonical_file).await {
        Ok(bytes) => {
            let content_type = content_type_for_path(&canonical_file);
            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn content_type_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("css")          => "text/css; charset=utf-8",
        Some("js")           => "application/javascript; charset=utf-8",
        Some("html")         => "text/html; charset=utf-8",
        Some("svg")          => "image/svg+xml",
        Some("png")          => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif")          => "image/gif",
        Some("ico")          => "image/x-icon",
        Some("webp")         => "image/webp",
        Some("woff")         => "font/woff",
        Some("woff2")        => "font/woff2",
        Some("ttf")          => "font/ttf",
        Some("otf")          => "font/otf",
        Some("json" | "map") => "application/json",
        Some("xml")          => "application/xml",
        _                    => "application/octet-stream",
    }
}
