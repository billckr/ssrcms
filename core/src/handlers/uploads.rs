//! Unified upload file handler.
//!
//! Serves files at two URL shapes:
//! - `/uploads/{filename}` (current `Media::url()` output) — site resolved
//!   from the Host header, same as everywhere else site context is derived
//!   from the request. In production this shape never reaches Axum at all —
//!   Caddy serves it directly, rooted at that domain's own symlinked folder.
//! - `/uploads/{key}/{*rest}` (legacy — links generated before the hostname
//!   segment was dropped) where `key` is a site UUID or hostname, kept
//!   working indefinitely so already-published content doesn't break.
//!
//! In development (Axum only, no Caddy in front) this handler serves both
//! shapes; in production only the legacy shape can reach here, since the new
//! shape is intercepted by Caddy before it does.

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use crate::app_state::AppState;

/// GET /uploads/{*path}
///
/// A bare filename (no `/`) resolves the site from the Host header. A path
/// with a `/` is treated as the legacy `{key}/{rest}` shape, where the first
/// segment is a UUID or hostname and hostname → UUID resolution happens via
/// OS symlinks or the site cache fallback.
pub async fn serve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<String>,
) -> Response {
    let uploads_dir = &state.config.uploads_dir;

    let file_path = match path.split_once('/') {
        Some((key, rest)) if !rest.is_empty() => resolve_file_path(uploads_dir, key, rest, &state),
        Some(_) => None, // trailing slash with empty rest — not a real file
        None => {
            // Bare filename: resolve the site from the Host header, same as
            // every other host-derived lookup (see admin_auth.rs).
            let host = headers
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .map(|raw| raw.split(':').next().unwrap_or(raw));
            match host.and_then(|h| state.resolve_site(h)) {
                Some((site, _)) => resolve_file_path(uploads_dir, &site.id.to_string(), &path, &state),
                None => None,
            }
        }
    };

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
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Create (or repair) the `uploads/{hostname} → {site_uuid}` symlink used to
/// serve media under a hostname-aliased URL instead of the raw UUID.
///
/// The target is written as a bare UUID string — relative to the symlink's
/// own directory (`uploads/`) — rather than joined onto `uploads_dir`. A
/// target like `uploads/{uuid}` is only valid if resolved from outside
/// `uploads/`, but the OS resolves a symlink's relative target from the
/// symlink's own parent directory, so joining `uploads_dir` again produced a
/// dangling `uploads/uploads/{uuid}` path. Caddy (production) has no
/// fallback for a broken symlink, so this must never regress.
pub fn ensure_hostname_symlink(uploads_dir: &str, hostname: &str, site_id: uuid::Uuid) {
    let sym = std::path::Path::new(uploads_dir).join(hostname);
    let target = site_id.to_string();

    // A broken symlink still occupies the directory entry (symlink() fails
    // with EEXIST over it) but `Path::exists()` reports false for it, since
    // that call follows the link. Detect and remove stale/broken links so
    // they get recreated below instead of silently failing forever.
    if let Ok(meta) = sym.symlink_metadata() {
        if meta.file_type().is_symlink() && !sym.exists() {
            let _ = std::fs::remove_file(&sym);
        }
    }

    if !sym.exists() {
        if let Err(e) = std::os::unix::fs::symlink(&target, &sym) {
            tracing::warn!("failed to create upload symlink for '{}': {}", hostname, e);
        } else {
            tracing::info!("created upload symlink: {} -> {}/", hostname, target);
        }
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
