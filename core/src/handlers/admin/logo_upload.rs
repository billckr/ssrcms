//! Admin sidebar logo upload/reset — the "Branding" card on /admin/settings'
//! General tab. Split out of settings.rs (which owns the plain-form fields)
//! since this needs multipart handling, same split convention as
//! appearance.rs/appearance_upload.rs for theme zip uploads.

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

/// Logos are tiny — deliberately not tied to the site's general
/// `max_upload_mb` setting (which can be set as high as 1000 MB for media).
const MAX_LOGO_BYTES: usize = 2 * 1024 * 1024;
const BRANDING_DIR: &str = "admin/static/branding";
const LOGO_FILENAMES: &[&str] = &["logo.svg", "logo.png", "logo.webp"];

fn redirect_with_flash(msg: &str) -> Redirect {
    Redirect::to(&format!("/admin/settings?flash={}", msg.replace(' ', "+")))
}

/// Remove any existing `admin/static/branding/logo.*` file, regardless of
/// which format is currently active — called before writing a new upload
/// (so e.g. switching from an SVG to a PNG logo doesn't leave the old SVG
/// behind to keep winning `detect_admin_logo()`'s priority order) and on
/// reset.
fn remove_existing_logo_files() {
    for name in LOGO_FILENAMES {
        let path = std::path::Path::new(BRANDING_DIR).join(name);
        if path.is_file() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("failed to remove old logo file '{}': {}", path.display(), e);
            }
        }
    }
}

pub async fn upload_logo(State(state): State<AppState>, admin: AdminUser, mut multipart: Multipart) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let mut filename: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            filename = field.file_name().map(|s| s.to_string());
            match field.bytes().await {
                Ok(b) => bytes = Some(b.to_vec()),
                Err(e) => {
                    tracing::error!("failed to read logo upload field: {:?}", e);
                    return redirect_with_flash("Failed to read uploaded file. Please try again.").into_response();
                }
            }
        }
    }

    let Some(bytes) = bytes else {
        return redirect_with_flash("No file received.").into_response();
    };
    if bytes.is_empty() {
        return redirect_with_flash("No file received.").into_response();
    }
    if bytes.len() > MAX_LOGO_BYTES {
        return redirect_with_flash("Logo file too large. Maximum size is 2 MB.").into_response();
    }

    let ext = match detect_logo_format(filename.as_deref().unwrap_or(""), &bytes) {
        Some(ext) => ext,
        None => return redirect_with_flash("Unsupported file type. Upload an SVG, PNG, or WebP image.").into_response(),
    };

    if ext == "svg" {
        if let Err(reason) = validate_svg_safety(&bytes) {
            tracing::warn!("logo upload rejected — {}", reason);
            return redirect_with_flash("That SVG couldn't be accepted — it contains scripting or unsafe content.").into_response();
        }
    }

    if let Err(e) = std::fs::create_dir_all(BRANDING_DIR) {
        tracing::error!("failed to create branding dir: {}", e);
        return redirect_with_flash("Failed to save logo. Please try again.").into_response();
    }

    remove_existing_logo_files();

    let dest = std::path::Path::new(BRANDING_DIR).join(format!("logo.{ext}"));
    if let Err(e) = std::fs::write(&dest, &bytes) {
        tracing::error!("failed to write logo file '{}': {}", dest.display(), e);
        return redirect_with_flash("Failed to save logo. Please try again.").into_response();
    }

    state.reload_logo_url();
    redirect_with_flash("Logo updated.").into_response()
}

pub async fn reset_logo(State(state): State<AppState>, admin: AdminUser) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    remove_existing_logo_files();
    state.reload_logo_url();
    redirect_with_flash("Logo is now text.").into_response()
}

/// Decide the format to save as from the upload's filename extension and a
/// magic-byte sniff of its actual content — the filename extension alone
/// can't be trusted (client-controlled), and browsers don't reliably set a
/// correct `Content-Type` on a multipart file part either.
fn detect_logo_format(filename: &str, bytes: &[u8]) -> Option<&'static str> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".svg") && looks_like_svg(bytes) {
        return Some("svg");
    }
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("png");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    None
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    String::from_utf8_lossy(head).contains("<svg")
}

/// Reject (rather than try to rewrite/sanitize) any SVG containing scripting
/// or content that could execute if someone navigates directly to the
/// uploaded file's URL — embedding it via `<img>` (how the sidebar logo is
/// actually used) is already safe from script execution, but a direct
/// navigation to the file's own URL is not. Deliberately conservative: a
/// false positive just means re-exporting a cleaner SVG; a false negative is
/// a stored XSS on a URL any admin can be sent.
fn validate_svg_safety(bytes: &[u8]) -> Result<(), &'static str> {
    let text = String::from_utf8_lossy(bytes).to_lowercase();
    if text.contains("<script") {
        return Err("contains <script>");
    }
    if text.contains("<foreignobject") {
        return Err("contains <foreignObject>");
    }
    if text.contains("javascript:") {
        return Err("contains a javascript: URI");
    }
    if text.contains("<!entity") || text.contains("<!doctype") {
        return Err("contains DOCTYPE/ENTITY (possible XXE)");
    }
    if has_event_handler_attribute(&text) {
        return Err("contains an event-handler attribute (onload/onclick/...)");
    }
    Ok(())
}

/// Cheap scan for ` on<word>=` — catches `onload=`, `onclick=`, `onerror=`,
/// etc. without needing a full XML/attribute parser for what's meant to be a
/// quick reject check, not a general-purpose SVG sanitizer.
fn has_event_handler_attribute(lowercase_text: &str) -> bool {
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex_lite::Regex::new(r#"\son[a-z]+\s*="#).unwrap());
    re.is_match(lowercase_text)
}
