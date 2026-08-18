//! Site admin's own "System Settings" — per-site admin branding (sidebar
//! name/logo shown to everyone logged into that site's admin). Distinct from
//! handlers/admin/settings.rs, the agency-wide page super_admin sees instead
//! — gated by `can_manage_site_settings`, which is deliberately mutually
//! exclusive with super_admin's `can_manage_settings` (see AdminCaps).

use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use std::collections::HashMap;

use crate::app_state::{set_site_setting, AppState};
use crate::middleware::admin_auth::AdminUser;

fn redirect_with_flash(msg: &str) -> Redirect {
    Redirect::to(&format!("/admin/site-settings?flash={}", msg.replace(' ', "+")))
}

pub async fn view(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_site_settings {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    };
    let flash = params.get("flash").map(|s| s.as_str());
    let cs = state.site_hostname(Some(site_id));
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let brand_name = state
        .get_site_by_id(site_id)
        .and_then(|(_, settings)| settings.admin_brand_name)
        .unwrap_or_default();
    let has_site_logo = crate::app_state::detect_site_admin_logo(site_id).is_some();
    Html(admin::pages::site_settings::render(flash, &brand_name, has_site_logo, &ctx)).into_response()
}

pub async fn save_general(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_site_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    };

    let brand_name = form.get("brand_name").map(|s| s.trim()).unwrap_or("");

    let mut error: Option<String> = None;
    if let Err(e) = set_site_setting(&state.db, site_id, "admin_brand_name", brand_name).await {
        tracing::error!("failed to save admin_brand_name for site {}: {}", site_id, e);
        error = Some("Failed to save settings. Please try again.".to_string());
    }

    if error.is_none() {
        if let Err(e) = state.reload_site_cache().await {
            tracing::warn!("failed to reload site cache: {:?}", e);
        }
    }

    let flash = error.as_deref().unwrap_or("General settings saved.");
    Redirect::to(&format!("/admin/site-settings?flash={}", flash.replace(' ', "+"))).into_response()
}

fn branding_dir(site_id: uuid::Uuid) -> std::path::PathBuf {
    std::path::Path::new("admin/static/branding").join(site_id.to_string())
}

const LOGO_FILENAMES: &[&str] = &["logo.svg", "logo.png", "logo.webp"];

fn remove_existing_site_logo_files(site_id: uuid::Uuid) {
    let dir = branding_dir(site_id);
    for name in LOGO_FILENAMES {
        let path = dir.join(name);
        if path.is_file() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("failed to remove old site logo file '{}': {}", path.display(), e);
            }
        }
    }
}

pub async fn upload_logo(admin: AdminUser, mut multipart: Multipart) -> impl IntoResponse {
    if !admin.caps.can_manage_site_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    };

    let mut filename: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            filename = field.file_name().map(|s| s.to_string());
            match field.bytes().await {
                Ok(b) => bytes = Some(b.to_vec()),
                Err(e) => {
                    tracing::error!("failed to read site logo upload field: {:?}", e);
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
    if bytes.len() > super::logo_upload::MAX_LOGO_BYTES {
        return redirect_with_flash("Logo file too large. Maximum size is 2 MB.").into_response();
    }

    let ext = match super::logo_upload::detect_logo_format(filename.as_deref().unwrap_or(""), &bytes) {
        Some(ext) => ext,
        None => return redirect_with_flash("Unsupported file type. Upload an SVG, PNG, or WebP image.").into_response(),
    };

    if ext == "svg" {
        if let Err(reason) = super::logo_upload::validate_svg_safety(&bytes) {
            tracing::warn!("site logo upload rejected — {}", reason);
            return redirect_with_flash("That SVG couldn't be accepted — it contains scripting or unsafe content.").into_response();
        }
    }

    let dir = branding_dir(site_id);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::error!("failed to create site branding dir: {}", e);
        return redirect_with_flash("Failed to save logo. Please try again.").into_response();
    }

    remove_existing_site_logo_files(site_id);

    let dest = dir.join(format!("logo.{ext}"));
    if let Err(e) = std::fs::write(&dest, &bytes) {
        tracing::error!("failed to write site logo file '{}': {}", dest.display(), e);
        return redirect_with_flash("Failed to save logo. Please try again.").into_response();
    }

    redirect_with_flash("Logo updated.").into_response()
}

pub async fn reset_logo(admin: AdminUser) -> impl IntoResponse {
    if !admin.caps.can_manage_site_settings {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    };
    remove_existing_site_logo_files(site_id);
    redirect_with_flash("Logo is now text.").into_response()
}
