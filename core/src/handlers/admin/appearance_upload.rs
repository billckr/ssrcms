//! Theme installation: zip upload and "create from default" — the two ways a
//! new theme directory gets created on disk. Split out of appearance.rs, which
//! also owns the theme list/activate/delete/screenshot handlers.

use axum::{
    extract::{Multipart, State, Form},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path as FsPath, PathBuf};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::appearance::render_create_theme_form;

use super::appearance::{copy_dir_all, render_appearance_list, url_encode_param, REQUIRED_TEMPLATES};

/// Fallback upload limit used only if config fails to load (should never happen at runtime).
const DEFAULT_MAX_ZIP_BYTES: usize = 25 * 1024 * 1024;

// ── Zip upload ─────────────────────────────────────────────────────────────────

pub async fn upload_theme(
    State(state): State<AppState>,
    admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    // Collect the zip bytes from the multipart field named "file".
    let max_bytes = (state.config.max_upload_mb as usize)
        .saturating_mul(1024 * 1024)
        .max(DEFAULT_MAX_ZIP_BYTES);
    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            match field.bytes().await {
                Ok(b) if b.len() <= max_bytes => zip_bytes = Some(b.to_vec()),
                Ok(_) => {
                    let msg = format!("Upload too large. Maximum size is {} MB.", state.config.max_upload_mb);
                    return render_appearance_list(&state, Some(&msg), &ctx, admin.site_id, "my")
                        .await
                        .into_response();
                }
                Err(e) => {
                    tracing::error!("failed to read theme zip field: {:?}", e);
                    return render_appearance_list(&state, Some("Failed to read uploaded file. Please try again."), &ctx, admin.site_id, "my")
                        .await
                        .into_response();
                }
            }
        }
    }

    let zip_bytes = match zip_bytes {
        Some(b) => b,
        None => return render_appearance_list(&state, Some("No file received."), &ctx, admin.site_id, "my").await.into_response(),
    };

    // Route the upload to the correct subdirectory.
    // Super admins upload to themes/global/; site admins upload to sites/<site_id>/themes/.
    let themes_parent = state.config.themes_dir.clone();
    let sites_parent  = state.config.sites_dir.clone();
    let target_dir = if admin.caps.is_global_admin {
        format!("{}/global", themes_parent)
    } else if let Some(sid) = admin.site_id {
        format!("{}/{}/themes", sites_parent, sid)
    } else {
        return render_appearance_list(&state, Some("No site selected. Cannot install theme."), &ctx, admin.site_id, "my")
            .await
            .into_response();
    };

    // Ensure target directory exists.
    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        tracing::error!("failed to create theme target dir '{}': {}", target_dir, e);
        return render_appearance_list(&state, Some("Failed to prepare theme directory."), &ctx, admin.site_id, "my")
            .await
            .into_response();
    }

    // Run zip extraction on a blocking thread (zip crate is synchronous).
    let result = tokio::task::spawn_blocking(move || {
        extract_and_install_theme(&zip_bytes, &target_dir)
    })
    .await;

    match result {
        Ok(Ok(theme_name)) => {
            tracing::info!("theme '{}' installed successfully", theme_name);

            // Always reload the active theme in Tera after a successful upload.
            let active = state.active_theme.read().unwrap().clone();
            if let Err(e) = state.templates.switch_theme(&active) {
                tracing::error!("theme '{}' installed but Tera reload of '{}' failed: {:?}", theme_name, active, e);
                return render_appearance_list(&state, Some("Theme installed but could not be reloaded. Please restart the server."), &ctx, admin.site_id, "my")
                    .await
                    .into_response();
            }
            tracing::info!("reloaded active theme '{}' after installing '{}'", active, theme_name);

            render_appearance_list(&state, Some(&format!("Theme '{}' installed successfully.", theme_name)), &ctx, admin.site_id, "my")
                .await
                .into_response()
        }
        Ok(Err(msg)) => {
            tracing::warn!("theme upload rejected: {}", msg);
            render_appearance_list(&state, Some("Installation failed. Please try again."), &ctx, admin.site_id, "my").await.into_response()
        }
        Err(e) => {
            tracing::error!("theme upload task panicked: {:?}", e);
            render_appearance_list(&state, Some("Installation failed. Please try again."), &ctx, admin.site_id, "my")
                .await
                .into_response()
        }
    }
}

/// Extract a theme zip into a temp directory, validate structure, then move it to target_dir.
/// Returns the theme name on success, or a user-facing error string on failure.
fn extract_and_install_theme(zip_bytes: &[u8], target_dir: &str) -> Result<String, String> {
    use std::io::Cursor;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "File does not appear to be a valid zip archive.".to_string())?;

    // Detect top-level directory prefix (many zips wrap content in a folder).
    let prefix = find_theme_prefix(&mut archive)?;

    // Extract into a temp directory.
    let tmp_dir = tempdir_in(target_dir)?;
    let tmp_path = tmp_dir.clone();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let raw_name = entry.name().to_string();

        let relative = if prefix.is_empty() {
            raw_name.clone()
        } else {
            match raw_name.strip_prefix(&prefix) {
                Some(r) => r.to_string(),
                None => continue,
            }
        };

        if relative.is_empty() {
            continue;
        }

        // Security: reject any path that tries to escape the temp directory.
        if relative.contains("..") || relative.starts_with('/') || relative.starts_with('\\') {
            return Err("Zip contains invalid paths. Installation aborted.".to_string());
        }

        let dest = PathBuf::from(&tmp_path).join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&dest)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read zip entry: {}", e))?;
            fs::write(&dest, &buf)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    // Validate: theme.toml must be present and parseable.
    let toml_path = PathBuf::from(&tmp_path).join("theme.toml");
    let toml_content = fs::read_to_string(&toml_path)
        .map_err(|_| "theme.toml not found in zip. Is this a valid Synaptic Signals theme?".to_string())?;

    let parsed: toml::Table = toml::from_str(&toml_content)
        .map_err(|_| "theme.toml is not valid TOML.".to_string())?;

    let theme_name = parsed
        .get("theme")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .ok_or("theme.toml is missing [theme] name field.".to_string())?
        .to_string();

    if theme_name.is_empty() || theme_name.contains('/') || theme_name.contains('\\') || theme_name.contains("..") {
        return Err("theme.toml contains an invalid theme name.".to_string());
    }

    // Validate required templates.
    let missing: Vec<&str> = REQUIRED_TEMPLATES
        .iter()
        .copied()
        .filter(|rel| !PathBuf::from(&tmp_path).join(rel).exists())
        .collect();

    if !missing.is_empty() {
        return Err(format!(
            "Theme is missing required files: {}",
            missing.join(", ")
        ));
    }

    // Move temp dir to target_dir/<name>, replacing any existing theme of the same name.
    let final_path = PathBuf::from(target_dir).join(&theme_name);
    if final_path.exists() {
        fs::remove_dir_all(&final_path)
            .map_err(|e| format!("Failed to replace existing theme: {}", e))?;
    }
    fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("Failed to install theme: {}", e))?;

    Ok(theme_name)
}

/// Find the top-level directory prefix inside a zip where theme.toml lives.
/// Returns an empty string if theme.toml is at the root.
///
/// Prefers a root-level theme.toml over one nested inside a subdirectory —
/// this prevents a stale nested copy of an old theme from being used when the
/// zip contains both a root theme.toml and a subdirectory with its own toml.
fn find_theme_prefix(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Result<String, String> {
    let mut nested: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip: {}", e))?;
        let name = entry.name().to_string();
        if name == "theme.toml" {
            // Root-level wins immediately.
            return Ok(String::new());
        }
        // Only record the shallowest nested theme.toml (one level deep).
        if name.ends_with("/theme.toml") && name.matches('/').count() == 1 && nested.is_none() {
            let prefix = name[..name.len() - "theme.toml".len()].to_string();
            nested = Some(prefix);
        }
    }
    nested.ok_or("theme.toml not found in zip. Is this a valid Synaptic Signals theme?".to_string())
}

/// Create a uniquely-named temporary directory inside themes_dir.
/// Returns the path as a String.
fn tempdir_in(dir: &str) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let tmp_name = format!(".theme_upload_tmp_{}", ts);
    let tmp_path = PathBuf::from(dir).join(&tmp_name);
    fs::create_dir_all(&tmp_path)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    tmp_path.to_str()
        .map(|s| s.to_string())
        .ok_or("Temp path is not valid UTF-8.".to_string())
}

// ── Create Theme ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateThemeForm {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    /// `"public"` → `themes/global/`; anything else (including absent) → `themes/private/`.
    /// Only respected for super_admin; site_admin themes always go to `themes/sites/<id>/`.
    pub visibility: Option<String>,
}

pub async fn create_form(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(render_create_theme_form(None, &ctx)).into_response()
}

pub async fn create_theme(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<CreateThemeForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! form_err {
        ($msg:expr) => {
            return Html(render_create_theme_form(Some($msg), &ctx)).into_response()
        };
    }

    // Validate name
    let name = form.name.trim().to_string();
    if name.is_empty() {
        form_err!("Theme name is required.");
    }
    if name.len() > 64 {
        form_err!("Theme name must be 64 characters or less.");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.starts_with('.') {
        form_err!("Theme name must not contain slashes, backslashes, '..', or start with a dot.");
    }

    // Determine target directory.
    // super_admin: public → themes/global/<name>/, private → themes/private/<name>/
    // site_admin:  always → sites/<id>/themes/<name>/  (visibility ignored)
    let themes_parent = &state.config.themes_dir;
    let is_public = form.visibility.as_deref() == Some("public");
    let target_dir: PathBuf = if admin.caps.is_global_admin {
        if is_public {
            FsPath::new(themes_parent).join("global").join(&name)
        } else {
            FsPath::new(themes_parent).join("private").join(&name)
        }
    } else if let Some(sid) = admin.site_id {
        FsPath::new(&state.config.sites_dir).join(sid.to_string()).join("themes").join(&name)
    } else {
        form_err!("No site selected. Cannot create theme.");
    };

    // Check uniqueness
    if target_dir.exists() {
        form_err!("A theme with that name already exists.");
    }

    // Copy the global default theme as the starting point so the new theme
    // has the same templates, CSS, and assets as the default.
    let default_src = FsPath::new(themes_parent).join("global").join("default");
    if !default_src.is_dir() {
        form_err!("The global 'default' theme was not found. Cannot create theme.");
    }
    if let Err(e) = copy_dir_all(&default_src, &target_dir) {
        tracing::error!("create_theme: failed to copy default theme to '{}': {}", name, e);
        let _ = fs::remove_dir_all(&target_dir);
        form_err!("Failed to copy theme files. Please try again.");
    }

    // Overwrite theme.toml with user-supplied name, description, and author.
    let description = form.description.as_deref().unwrap_or("").trim().to_string();
    let author = form.author.as_deref().unwrap_or("").trim().to_string();
    let toml_content = format!(
        "[theme]\nname = \"{name}\"\nversion = \"1.0.0\"\ndescription = \"{description}\"\nauthor = \"{author}\"\n",
        name = name.replace('"', "\\\""),
        description = description.replace('"', "\\\""),
        author = author.replace('"', "\\\""),
    );
    if let Err(e) = fs::write(target_dir.join("theme.toml"), toml_content.as_bytes()) {
        tracing::error!("create_theme: failed to write theme.toml for '{}': {}", name, e);
        let _ = fs::remove_dir_all(&target_dir);
        form_err!("Failed to write theme files. Please try again.");
    }

    tracing::info!("theme '{}' created by {}", name, if admin.caps.is_global_admin { "super_admin" } else { "site_admin" });
    Redirect::to(&format!("/admin/appearance/editor/{}", url_encode_param(&name))).into_response()
}
