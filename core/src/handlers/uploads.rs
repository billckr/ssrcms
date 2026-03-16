//! Unified upload file handler.
//!
//! Serves files at `/uploads/{key}/{*rest}` where `key` is either:
//! - A site UUID: served from `uploads/{uuid}/{rest}`
//! - A site hostname: resolved via the `uploads/{hostname}/` symlink to `uploads/{uuid}/`
//!
//! In production (Caddy) the hostname symlink is served by Caddy's file_server directly.
//! In development (Axum only) this handler serves both UUID and hostname paths.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

use crate::app_state::AppState;

/// GET /uploads/{*path}
///
/// First path segment is a UUID or hostname; the rest is the filename.
/// Hostname → UUID resolution happens via OS symlinks or the site cache fallback.
pub async fn serve(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    let parts: Vec<&str> = path.splitn(2, '/').collect();
    let (key, rest) = match parts.as_slice() {
        [k, r] => (k.to_string(), r.to_string()),
        _      => return StatusCode::NOT_FOUND.into_response(),
    };

    if rest.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }

    // Resolve the filesystem path. Try the key as-is first (works for UUIDs and
    // symlinked hostnames). Fall back to site-cache lookup for hostnames without
    // a symlink (e.g. newly created sites before app restart creates the symlink).
    let uploads_dir = &state.config.uploads_dir;
    let file_path = resolve_file_path(uploads_dir, &key, &rest, &state);

    let file_path = match file_path {
        Some(p) => p,
        None    => return StatusCode::NOT_FOUND.into_response(),
    };

    // Canonicalize to resolve symlinks and guard against path traversal.
    let canonical_file = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let uploads_canonical = match std::path::Path::new(uploads_dir).canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if !canonical_file.starts_with(&uploads_canonical) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if canonical_file.is_dir() {
        return StatusCode::NOT_FOUND.into_response();
    }

    match tokio::fs::read(&canonical_file).await {
        Ok(bytes) => {
            let ct = content_type_for_path(&canonical_file);
            (
                [
                    (header::CONTENT_TYPE, ct),
                    (header::CACHE_CONTROL, "public, max-age=31536000"),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Resolve the full filesystem path for a file, trying the direct path first
/// (which works for UUIDs and symlinked hostnames) then falling back to the
/// site cache when a hostname has no symlink yet.
fn resolve_file_path(
    uploads_dir: &str,
    key: &str,
    rest: &str,
    state: &AppState,
) -> Option<std::path::PathBuf> {
    let direct = std::path::Path::new(uploads_dir).join(key).join(rest);
    if direct.parent().map(|p| p.exists()).unwrap_or(false) {
        return Some(direct);
    }

    // Direct path doesn't exist: if key looks like a hostname (not a UUID),
    // resolve via the site cache to get the UUID directory.
    if key.parse::<uuid::Uuid>().is_err() {
        if let Some((site, _)) = state.resolve_site(key) {
            let uuid_path = std::path::Path::new(uploads_dir)
                .join(site.id.to_string())
                .join(rest);
            return Some(uuid_path);
        }
    }

    None
}

fn content_type_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png")           => "image/png",
        Some("gif")           => "image/gif",
        Some("webp")          => "image/webp",
        Some("avif")          => "image/avif",
        Some("svg")           => "image/svg+xml",
        Some("ico")           => "image/x-icon",
        Some("mp4")           => "video/mp4",
        Some("webm")          => "video/webm",
        Some("mov")           => "video/quicktime",
        Some("avi")           => "video/x-msvideo",
        Some("mp3")           => "audio/mpeg",
        Some("wav")           => "audio/wav",
        Some("ogg")           => "audio/ogg",
        Some("flac")          => "audio/flac",
        Some("pdf")           => "application/pdf",
        Some("zip")           => "application/zip",
        Some("doc")           => "application/msword",
        Some("docx")          => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls")           => "application/vnd.ms-excel",
        Some("xlsx")          => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _                     => "application/octet-stream",
    }
}
