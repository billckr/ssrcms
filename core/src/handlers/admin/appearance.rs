use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State, Form},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path as FsPath, PathBuf};
use uuid::Uuid;

use crate::app_state::{AppState, set_site_setting};
use crate::middleware::admin_auth::AdminUser;
use admin::pages::appearance::{ThemeInfo, render_with_flash, render_create_theme_form};

/// Required template files every valid theme must provide.
const REQUIRED_TEMPLATES: &[&str] = &[
    "templates/base.html",
    "templates/index.html",
    "templates/single.html",
    "templates/page.html",
    "templates/archive.html",
    "templates/search.html",
    "templates/404.html",
];

/// Maximum permitted zip upload size: 50 MB.
const MAX_ZIP_BYTES: usize = 50 * 1024 * 1024;

#[derive(Deserialize, Default)]
pub struct AppearanceQuery {
    #[serde(default)]
    pub filter: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<AppearanceQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let filter = q.filter.as_deref().unwrap_or("my");
    render_appearance_list(&state, None, &ctx, admin.site_id, filter).await.into_response()
}

// ── Activate ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ActivateForm {
    pub theme: String,
}

pub async fn activate(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<ActivateForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let themes_dir = &state.config.themes_dir;
    let cs = state.site_hostname(admin.site_id);

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Reject obviously invalid names before any filesystem access.
    if form.theme.contains("..") || form.theme.contains('/') || form.theme.contains('\\') {
        return render_appearance_list(&state, Some("Invalid theme name."), &ctx, admin.site_id, "my").await.into_response();
    }

    let global_dir = FsPath::new(themes_dir).join("global");
    let site_dir = admin.site_id.map(|id| FsPath::new(themes_dir).join("sites").join(id.to_string()));

    // Resolve which directory the theme lives in.
    let theme_path = if global_dir.join(&form.theme).is_dir() {
        global_dir.join(&form.theme)
    } else if let Some(ref sd) = site_dir {
        if sd.join(&form.theme).is_dir() {
            sd.join(&form.theme)
        } else {
            tracing::warn!("theme activation failed: theme '{}' not found in global or site dirs", form.theme);
            return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
        }
    } else {
        tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
    };

    // Path traversal guard: theme must stay within global/ or sites/<id>/.
    let canonical_theme = match theme_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response(),
    };
    let canonical_global = global_dir.canonicalize().unwrap_or_default();
    let canonical_site = site_dir
        .as_ref()
        .and_then(|sd| sd.canonicalize().ok())
        .unwrap_or_default();
    if !canonical_theme.starts_with(&canonical_global) && !canonical_theme.starts_with(&canonical_site) {
        tracing::warn!("activate path traversal attempt: theme_name={:?}", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            tracing::warn!("theme activate: no site selected, cannot save per-site setting");
            return render_appearance_list(&state, Some("No site selected. Run 'synaptic-cli site init' first."), &ctx, admin.site_id, "my").await.into_response();
        }
    };

    // If a site admin is activating a global theme, copy it to their site dir
    // first so they get their own editable version. Skip if a site copy already exists.
    if !admin.caps.is_global_admin && canonical_theme.starts_with(&canonical_global) {
        let site_copy = FsPath::new(themes_dir)
            .join("sites")
            .join(site_id.to_string())
            .join(&form.theme);
        if !site_copy.exists() {
            let src = canonical_theme.clone();
            let dst = site_copy.clone();
            match tokio::task::spawn_blocking(move || copy_dir_all(&src, &dst)).await {
                Ok(Ok(())) => tracing::info!("auto-copied global theme '{}' to site {}", form.theme, site_id),
                Ok(Err(e)) => {
                    tracing::error!("activate: failed to copy theme '{}' to site dir: {}", form.theme, e);
                    return render_appearance_list(&state, Some("Failed to copy theme to your site. Please try again."), &ctx, admin.site_id, "my").await.into_response();
                }
                Err(e) => {
                    tracing::error!("activate: copy task panicked: {:?}", e);
                    return render_appearance_list(&state, Some("Failed to copy theme to your site. Please try again."), &ctx, admin.site_id, "my").await.into_response();
                }
            }
        }
    }

    if let Err(e) = set_site_setting(&state.db, site_id, "active_theme", &form.theme).await {
        tracing::error!("failed to save active_theme to DB: {:?}", e);
        return render_appearance_list(&state, Some("Failed to activate theme. Please try again."), &ctx, admin.site_id, "my").await.into_response();
    }

    if let Err(e) = state.templates.switch_theme(&form.theme) {
        tracing::error!("failed to switch theme to '{}': {:?}", form.theme, e);
        return render_appearance_list(&state, Some("Theme files could not be loaded. Please try again."), &ctx, admin.site_id, "my").await.into_response();
    }

    *state.active_theme.write().unwrap() = form.theme.clone();

    // Keep the in-memory site_cache in sync so the static file handler
    // immediately serves assets from the newly selected theme.
    state.update_site_theme_in_cache(site_id, &form.theme);

    Redirect::to("/admin/appearance").into_response()
}

// ── Delete ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteForm {
    pub theme: String,
}

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<DeleteForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! err {
        ($msg:expr) => {
            return render_appearance_list(&state, Some($msg), &ctx, admin.site_id, "my")
                .await
                .into_response()
        };
    }

    // Reject obviously invalid names.
    if form.theme.contains("..") || form.theme.contains('/') || form.theme.contains('\\') || form.theme.is_empty() {
        err!("Invalid theme name.");
    }

    let themes_dir = &state.config.themes_dir;
    let global_path = FsPath::new(themes_dir).join("global").join(&form.theme);
    let site_path = admin.site_id
        .map(|id| FsPath::new(themes_dir).join("sites").join(id.to_string()).join(&form.theme));

    // Determine whether this is a global or site theme.
    let (theme_path, is_global) = if global_path.is_dir() {
        (global_path, true)
    } else if let Some(ref sp) = site_path {
        if sp.is_dir() {
            (sp.clone(), false)
        } else {
            err!("Theme not found.");
        }
    } else {
        err!("Theme not found.");
    };

    // Authorization: only super admins may delete global themes.
    if is_global && !admin.caps.is_global_admin {
        err!("Only super admins can delete global themes.");
    }

    // Active theme guard (server-side, even though UI hides the button).
    let active_for_site: Option<String> = if let Some(sid) = admin.site_id {
        sqlx::query_scalar(
            "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'active_theme'",
        )
        .bind(sid)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
    } else {
        None
    };

    if active_for_site.as_deref() == Some(form.theme.as_str()) {
        err!("Cannot delete the active theme. Activate a different theme first.");
    }

    // In-use guard for global themes: block if any site has it active.
    if is_global {
        let in_use: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM site_settings WHERE key = 'active_theme' AND value = $1",
        )
        .bind(&form.theme)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if in_use > 0 {
            err!("Cannot delete: this theme is currently active on one or more sites.");
        }
    }

    // Path traversal guard: confirm the theme dir is a direct child of the expected parent.
    let expected_parent = if is_global {
        match FsPath::new(themes_dir).join("global").canonicalize() {
            Ok(p) => p,
            Err(_) => err!("Theme not found."),
        }
    } else {
        let sid = admin.site_id.unwrap();
        match FsPath::new(themes_dir).join("sites").join(sid.to_string()).canonicalize() {
            Ok(p) => p,
            Err(_) => err!("Theme not found."),
        }
    };

    let canonical_theme = match theme_path.canonicalize() {
        Ok(p) => p,
        Err(_) => err!("Theme not found."),
    };

    if canonical_theme.parent() != Some(expected_parent.as_path()) {
        tracing::warn!("delete path traversal attempt: theme={:?}", form.theme);
        err!("Invalid theme path.");
    }

    // Remove the theme directory.
    if let Err(e) = fs::remove_dir_all(&canonical_theme) {
        tracing::error!("failed to delete theme '{}': {:?}", form.theme, e);
        err!("Failed to delete theme. Please try again.");
    }

    tracing::info!("theme '{}' deleted by {}", form.theme, if admin.caps.is_global_admin { "super_admin" } else { "site_admin" });
    render_appearance_list(&state, Some(&format!("Theme '{}' deleted.", form.theme)), &ctx, admin.site_id, "my")
        .await
        .into_response()
}

// ── Screenshot ─────────────────────────────────────────────────────────────────

pub async fn screenshot(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme_name): Path<String>,
) -> Response {
    let themes_dir = FsPath::new(&state.config.themes_dir);
    let global_dir = themes_dir.join("global");
    let site_dir = admin.site_id.map(|id| themes_dir.join("sites").join(id.to_string()));

    // Try global dir first, then site dir.
    let dirs_to_search: Vec<PathBuf> = std::iter::once(global_dir)
        .chain(site_dir.into_iter())
        .collect();

    for dir in &dirs_to_search {
        let candidate = dir.join(&theme_name).join("screenshot.png");
        let canonical_dir = match dir.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let canonical_candidate = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical_candidate.starts_with(&canonical_dir) {
            tracing::warn!("screenshot path traversal attempt: theme_name={:?}", theme_name);
            continue;
        }
        if let Ok(bytes) = fs::read(&canonical_candidate) {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }

    StatusCode::NOT_FOUND.into_response()
}

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
    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            match field.bytes().await {
                Ok(b) if b.len() <= MAX_ZIP_BYTES => zip_bytes = Some(b.to_vec()),
                Ok(_) => {
                    return render_appearance_list(&state, Some("Upload too large. Maximum size is 50 MB."), &ctx, admin.site_id, "my")
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
    // Super admins upload to themes/global/; site admins upload to themes/sites/<site_id>/.
    let themes_parent = state.config.themes_dir.clone();
    let target_dir = if admin.caps.is_global_admin {
        format!("{}/global", themes_parent)
    } else if let Some(sid) = admin.site_id {
        format!("{}/sites/{}", themes_parent, sid)
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

    // Determine target directory
    let themes_parent = &state.config.themes_dir;
    let target_dir: PathBuf = if admin.caps.is_global_admin {
        FsPath::new(themes_parent).join("global").join(&name)
    } else if let Some(sid) = admin.site_id {
        FsPath::new(themes_parent).join("sites").join(sid.to_string()).join(&name)
    } else {
        form_err!("No site selected. Cannot create theme.");
    };

    // Check uniqueness
    if target_dir.exists() {
        form_err!("A theme with that name already exists.");
    }

    // Create directories
    let templates_dir = target_dir.join("templates");
    let static_dir = target_dir.join("static");
    if let Err(e) = fs::create_dir_all(&templates_dir).and_then(|_| fs::create_dir_all(&static_dir)) {
        tracing::error!("create_theme: failed to create dirs for '{}': {}", name, e);
        form_err!("Failed to create theme directory. Please try again.");
    }

    // Write theme.toml
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

    // Scaffold template files
    let templates: &[(&str, &str)] = &[
        ("templates/newsletter.html", r#"{% extends "base.html" %}

{% block title %}{{ page.title }} — {{ site.name }}{% endblock title %}

{% block content %}
<article class="single-page newsletter-page">
  <header class="contact-header">
    <h1 class="contact-title">{{ page.title }}</h1>
  </header>

  {% if page.content %}
  <div class="page-content">
    {{ page.content | safe }}
  </div>
  {% endif %}

  {% if request.query.submitted %}
  <div class="newsletter-success" role="alert">
    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24"
         fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
         aria-hidden="true">
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
      <polyline points="22 4 12 14.01 9 11.01"/>
    </svg>
    You're signed up! Thanks for subscribing.
  </div>
  {% else %}
  <form class="newsletter-form" method="POST" action="/form/newsletter">
    <!-- Honeypot — hidden from real users, bots fill it in -->
    <div class="newsletter-honeypot" aria-hidden="true" tabindex="-1">
      <label for="_hp">Leave this blank</label>
      <input type="text" id="_hp" name="_honeypot" tabindex="-1" autocomplete="off">
    </div>

    <div class="newsletter-field">
      <label for="nl-email">Email address <span class="newsletter-required" aria-hidden="true">*</span></label>
      <input type="email" id="nl-email" name="email" required autocomplete="email"
             placeholder="you@example.com">
    </div>

    <div class="newsletter-field newsletter-field--checkbox">
      <label class="newsletter-checkbox-label">
        <input type="checkbox" name="terms_accepted" value="yes" required>
        I agree to the <a href="/terms" target="_blank" rel="noopener noreferrer">Terms &amp; Conditions</a>
        and consent to receiving email newsletters.
        <span class="newsletter-required" aria-hidden="true">*</span>
      </label>
    </div>

    <button type="submit" class="newsletter-submit" id="newsletter-submit-btn">Subscribe</button>
  </form>
  <script>
    document.querySelector('.newsletter-form').addEventListener('submit', function() {
      var btn = document.getElementById('newsletter-submit-btn');
      btn.disabled = true;
      btn.textContent = 'Subscribing…';
    });
  </script>
  {% endif %}
</article>

<style>
/* ── Newsletter page ── */
.newsletter-page { max-width: 520px; }

.newsletter-form {
  margin-top: 2rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.newsletter-honeypot {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}

.newsletter-field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.newsletter-field label {
  font-size: 0.875rem;
  font-weight: 600;
  color: #333;
}

.newsletter-required { color: #c00; }

.newsletter-field input[type="email"] {
  padding: 0.6rem 0.85rem;
  font-size: 1rem;
  font-family: inherit;
  border: 1.5px solid #ccc;
  border-radius: 4px;
  background: #fff;
  color: #333;
  transition: border-color 0.15s, box-shadow 0.15s;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}

.newsletter-field input[type="email"]:focus {
  border-color: #555;
  box-shadow: 0 0 0 3px rgba(0,0,0,0.08);
}

.newsletter-field--checkbox { flex-direction: row; align-items: flex-start; gap: 0; }

.newsletter-checkbox-label {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  font-size: 0.875rem;
  font-weight: 400;
  color: #333;
  cursor: pointer;
  line-height: 1.5;
}

.newsletter-checkbox-label input[type="checkbox"] {
  margin-top: 0.2rem;
  flex-shrink: 0;
  width: 1rem;
  height: 1rem;
  accent-color: #333;
  cursor: pointer;
}

.newsletter-checkbox-label a {
  color: #333;
  text-decoration: underline;
}

.newsletter-submit {
  align-self: flex-start;
  padding: 0.65rem 1.5rem;
  font-size: 1rem;
  font-weight: 600;
  font-family: inherit;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s, transform 0.1s;
}

.newsletter-submit:hover { background: #111; }
.newsletter-submit:active { transform: scale(0.98); }
.newsletter-submit:disabled { opacity: 0.6; cursor: not-allowed; }

.newsletter-success {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin-top: 1.5rem;
  padding: 1rem 1.25rem;
  background: #f0fdf4;
  border: 1.5px solid #86efac;
  border-radius: 6px;
  color: #166534;
  font-weight: 500;
}

.newsletter-success svg { flex-shrink: 0; color: #16a34a; }
</style>
{% endblock content %}
"#),
        ("templates/contact-page.html", r#"{% extends "base.html" %}

{% block title %}{{ page.title }} — {{ site.name }}{% endblock title %}

{% block content %}
<article class="single-page contact-page">
  <header class="contact-header">
    <h1 class="contact-title">{{ page.title }}</h1>
  </header>

  {% if page.content %}
  <div class="page-content">
    {{ page.content | safe }}
  </div>
  {% endif %}

  {% if request.query.submitted %}
  <div class="contact-success" role="alert">
    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24"
         fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
         aria-hidden="true">
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
      <polyline points="22 4 12 14.01 9 11.01"/>
    </svg>
    Thanks! Your message has been sent. We'll be in touch soon.
  </div>
  {% else %}
  <form class="contact-form" method="POST" action="/form/contact">
    <!-- Honeypot — hidden from real users, bots fill it in -->
    <div class="contact-honeypot" aria-hidden="true" tabindex="-1">
      <label for="_hp">Leave this blank</label>
      <input type="text" id="_hp" name="_honeypot" tabindex="-1" autocomplete="off">
    </div>

    <div class="contact-field">
      <label for="cf-name">Your name <span class="contact-required" aria-hidden="true">*</span></label>
      <input type="text" id="cf-name" name="name" required autocomplete="name"
             placeholder="Jane Smith">
    </div>

    <div class="contact-field">
      <label for="cf-email">Email address <span class="contact-required" aria-hidden="true">*</span></label>
      <input type="email" id="cf-email" name="email" required autocomplete="email"
             placeholder="jane@example.com">
    </div>

    <div class="contact-field">
      <label for="cf-subject">Subject</label>
      <input type="text" id="cf-subject" name="subject" placeholder="How can we help?">
    </div>

    <div class="contact-field">
      <label for="cf-message">Message <span class="contact-required" aria-hidden="true">*</span></label>
      <textarea id="cf-message" name="message" rows="6" required
                placeholder="Write your message here…"></textarea>
    </div>

    <button type="submit" class="contact-submit" id="contact-submit-btn">Send message</button>
  </form>
  <script>
    document.querySelector('.contact-form').addEventListener('submit', function() {
      var btn = document.getElementById('contact-submit-btn');
      btn.disabled = true;
      btn.textContent = 'Sending…';
    });
  </script>
  {% endif %}
</article>

<style>
/* ── Contact page ── */
.contact-page { max-width: 640px; }

.contact-header {
  padding: 1.25rem 1.5rem;
  border-radius: 6px;
  border-left: 4px solid #555;
  background: #f8f8f8;
  margin-bottom: 2rem;
}

.contact-title {
  margin: 0;
  font-size: 1.75rem;
  color: #222;
}

.contact-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.contact-honeypot {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}

.contact-field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.contact-field label {
  font-size: 0.875rem;
  font-weight: 600;
  color: #333;
}

.contact-required { color: #c00; }

.contact-field input,
.contact-field textarea {
  padding: 0.6rem 0.85rem;
  font-size: 1rem;
  font-family: inherit;
  border: 1.5px solid #ccc;
  border-radius: 4px;
  background: #fff;
  color: #333;
  transition: border-color 0.15s, box-shadow 0.15s;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}

.contact-field input:focus,
.contact-field textarea:focus {
  border-color: #555;
  box-shadow: 0 0 0 3px rgba(0,0,0,0.08);
}

.contact-field textarea {
  resize: vertical;
  min-height: 8rem;
}

.contact-submit {
  align-self: flex-start;
  padding: 0.65rem 1.5rem;
  font-size: 1rem;
  font-weight: 600;
  font-family: inherit;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s, transform 0.1s;
}

.contact-submit:hover { background: #111; }
.contact-submit:active { transform: scale(0.98); }
.contact-submit:disabled { opacity: 0.6; cursor: not-allowed; }

.contact-success {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin-top: 1.5rem;
  padding: 1rem 1.25rem;
  background: #f0fdf4;
  border: 1.5px solid #86efac;
  border-radius: 6px;
  color: #166534;
  font-weight: 500;
}

.contact-success svg { flex-shrink: 0; color: #16a34a; }
</style>
{% endblock content %}
"#),
        ("templates/base.html", r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{% block title %}{{ site.name }}{% endblock %}</title>
  <link rel="stylesheet" href="/theme/static/style.css">
  {% block head %}{% endblock %}
</head>
<body>
  <header>
    <h1><a href="/">{{ site.name }}</a></h1>
  </header>
  <main>
    {% block content %}{% endblock %}
  </main>
  <footer>
    <p>&copy; {{ site.name }}</p>
  </footer>
</body>
</html>
"#),
        ("templates/index.html", r#"{% extends "base.html" %}
{% block title %}{{ site.name }}{% endblock %}
{% block content %}
  {% for post in posts %}
  <article>
    <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
    <p>{{ post.excerpt }}</p>
  </article>
  {% endfor %}
{% endblock %}
"#),
        ("templates/single.html", r#"{% extends "base.html" %}
{% block title %}{{ post.title }} — {{ site.name }}{% endblock %}
{% block content %}
  <article>
    <h1>{{ post.title }}</h1>
    <div>{{ post.content | safe }}</div>
  </article>
{% endblock %}
"#),
        ("templates/page.html", r#"{% extends "base.html" %}
{% block title %}{{ post.title }} — {{ site.name }}{% endblock %}
{% block content %}
  <article>
    <h1>{{ post.title }}</h1>
    <div>{{ post.content | safe }}</div>
  </article>
{% endblock %}
"#),
        ("templates/archive.html", r#"{% extends "base.html" %}
{% block title %}{{ archive_type | title }}: {% if archive_term %}{{ archive_term.name }}{% endif %} — {{ site.name }}{% endblock %}
{% block content %}
  <h1>{{ archive_type | title }}{% if archive_term %}: {{ archive_term.name }}{% endif %}</h1>
  {% for post in posts %}
  <article>
    <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
    <p>{{ post.excerpt }}</p>
  </article>
  {% endfor %}
{% endblock %}
"#),
        ("templates/search.html", r#"{% extends "base.html" %}
{% block title %}Search — {{ site.name }}{% endblock %}
{% block content %}
  <h1>Search Results</h1>
  <form method="get" action="/search">
    <input type="search" name="q" value="{{ query }}">
    <button type="submit">Search</button>
  </form>
  {% for post in results %}
  <article>
    <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
    <p>{{ post.excerpt }}</p>
  </article>
  {% endfor %}
{% endblock %}
"#),
        ("templates/404.html", r#"{% extends "base.html" %}
{% block title %}Page Not Found — {{ site.name }}{% endblock %}
{% block content %}
  <h1>404 — Page Not Found</h1>
  <p>The page you requested could not be found.</p>
  <p><a href="/">Return home</a></p>
{% endblock %}
"#),
    ];

    for (rel, content) in templates {
        let dest = target_dir.join(rel);
        if let Err(e) = fs::write(&dest, content.as_bytes()) {
            tracing::error!("create_theme: failed to write '{}' for '{}': {}", rel, name, e);
            let _ = fs::remove_dir_all(&target_dir);
            form_err!("Failed to write template files. Please try again.");
        }
    }

    // Starter stylesheet — not a required template, but scaffolded for convenience.
    let css_path = target_dir.join("static").join("style.css");
    let css_content = b"/* -- Reset & base --------------------------------------- */
*, *::before, *::after { box-sizing: border-box; }

body {
  margin: 0;
  font-family: system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif;
  font-size: 1rem;
  line-height: 1.6;
  color: #222;
  background: #fff;
}

img, video { max-width: 100%; height: auto; display: block; }

a { color: #0066cc; text-decoration: none; }
a:hover { color: #004499; text-decoration: underline; }

/* -- Layout --------------------------------------------- */
.container {
  max-width: 860px;
  margin: 0 auto;
  padding: 0 1.25rem;
}

header, main, footer {
  padding: 1.5rem 0;
}

/* -- Typography ----------------------------------------- */
h1, h2, h3, h4, h5, h6 {
  line-height: 1.25;
  margin: 1.5rem 0 0.5rem;
  font-weight: 700;
  color: #111;
}

h1 { font-size: 2rem; }
h2 { font-size: 1.5rem; }
h3 { font-size: 1.25rem; }
h4 { font-size: 1.1rem; }

p { margin: 0 0 1rem; }

ul, ol { margin: 0 0 1rem; padding-left: 1.5rem; }
li { margin-bottom: 0.25rem; }

blockquote {
  margin: 1.5rem 0;
  padding: 0.75rem 1.25rem;
  border-left: 4px solid #ccc;
  color: #555;
}

hr { border: none; border-top: 1px solid #ddd; margin: 2rem 0; }

/* -- Code ----------------------------------------------- */
code, kbd {
  font-family: ui-monospace, \"Cascadia Code\", \"Fira Code\", monospace;
  font-size: 0.875em;
  background: #f3f3f3;
  padding: 0.15em 0.35em;
  border-radius: 3px;
}

pre {
  background: #f3f3f3;
  padding: 1rem 1.25rem;
  border-radius: 4px;
  overflow-x: auto;
  margin: 0 0 1rem;
}

pre code { background: none; padding: 0; font-size: 0.9rem; }

/* -- Forms ---------------------------------------------- */
input, textarea, select, button { font: inherit; }

input[type=\"text\"],
input[type=\"email\"],
input[type=\"search\"],
textarea,
select {
  width: 100%;
  padding: 0.5rem 0.75rem;
  border: 1.5px solid #ccc;
  border-radius: 4px;
  background: #fff;
  color: #222;
  box-sizing: border-box;
}

input:focus, textarea:focus, select:focus {
  outline: none;
  border-color: #555;
  box-shadow: 0 0 0 3px rgba(0,0,0,0.08);
}

button, input[type=\"submit\"] {
  display: inline-block;
  padding: 0.5rem 1.25rem;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 600;
}

button:hover, input[type=\"submit\"]:hover { background: #111; }

/* -- Utility -------------------------------------------- */
.muted { color: #666; font-size: 0.9rem; }
.text-center { text-align: center; }
";
    if let Err(e) = fs::write(&css_path, css_content) {
        tracing::warn!("create_theme: failed to write style.css for '{}': {}", name, e);
        // Non-fatal — theme is still valid without it.
    }

    tracing::info!("theme '{}' created by {}", name, if admin.caps.is_global_admin { "super_admin" } else { "site_admin" });
    Redirect::to(&format!("/admin/appearance/editor/{}", url_encode_param(&name))).into_response()
}

// ── New File ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NewFileForm {
    pub filename: String,
    pub ext: String,
}

pub async fn new_file(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme): Path<String>,
    Form(form): Form<NewFileForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id, !admin.caps.is_global_admin) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    // Helper: re-render editor with a flash error.
    let editor_err = |msg: &'static str| {
        let files = walk_theme_files(&theme_dir);
        let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
            admin::pages::appearance::EditorFile {
                rel_path: f.clone(),
                is_selected: false,
                has_backup: bak_path_for(&theme_dir.join(f)).exists(),
                edited_at: None,
            }
        }).collect();
        Html(admin::pages::appearance::render_theme_editor(
            &theme, &editor_files, None, "", false, Some(msg), &ctx, false,
        )).into_response()
    };

    if !admin.caps.is_global_admin && is_in_global_dir(&theme_dir, &state.config.themes_dir) {
        return editor_err("Global themes cannot be modified. Copy this theme to your site first.");
    }

    // name = bare name the user typed (e.g. "partials/header" or "custom")
    // ext  = dropdown selection: ".html", ".css", or ".js"
    let name = form.filename.trim().to_string();
    let ext  = form.ext.trim().to_string();

    if name.is_empty() || name.len() > 96 {
        return editor_err("Filename must be 1–96 characters.");
    }
    if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
        return editor_err("Invalid filename.");
    }

    // Map extension → subdirectory and initial file content.
    let (subdir, initial_content): (&str, &[u8]) = match ext.as_str() {
        ".html" => ("templates", b"{# New template #}\n"),
        ".css"  => ("static",    b"/* styles */\n"),
        ".js"   => ("static",    b"/* scripts */\n"),
        _       => return editor_err("Invalid file type. Choose .html, .css, or .js."),
    };

    // Full relative path from theme root, e.g. "templates/partials/header.html"
    let rel = format!("{}/{}{}", subdir, name, ext);
    let dest = theme_dir.join(&rel);

    // Create parent dirs if needed, then verify the resolved path stays inside
    // the expected subdirectory (traversal guard).
    if let Some(parent) = dest.parent() {
        let allowed_root = theme_dir.join(subdir);
        let canonical_allowed = match allowed_root.canonicalize() {
            Ok(p) => p,
            Err(_) => return editor_err("Theme directory not found."),
        };
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::error!("new_file: create_dir_all failed: {}", e);
            return editor_err("Failed to create directory.");
        }
        let canonical_parent = parent.canonicalize().unwrap_or(canonical_allowed.clone());
        if !canonical_parent.starts_with(&canonical_allowed) {
            tracing::warn!("new_file: path traversal attempt: rel={:?}", rel);
            return editor_err("Invalid filename.");
        }
    }

    if let Err(e) = fs::write(&dest, initial_content) {
        tracing::error!("new_file: write failed for {:?}: {}", dest, e);
        return editor_err("Failed to create file. Please try again.");
    }

    Redirect::to(&format!(
        "/admin/appearance/editor/{}?file={}",
        url_encode_param(&theme),
        url_encode_param(&rel),
    )).into_response()
}

// ── Get Theme (copy global → site, no activation) ─────────────────────────────

#[derive(Deserialize)]
pub struct GetThemeForm {
    pub theme: String,
}

pub async fn get_theme(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<GetThemeForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    // Super admins own global themes directly — nothing to copy.
    if admin.caps.is_global_admin {
        return Redirect::to("/admin/appearance?filter=my").into_response();
    }

    let name = form.theme.trim().to_string();
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return render_appearance_list(&state, Some("Invalid theme name."), &ctx, admin.site_id, "global")
            .await.into_response();
    }

    let themes_dir = &state.config.themes_dir;
    let source = FsPath::new(themes_dir).join("global").join(&name);
    if !source.is_dir() {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return render_appearance_list(&state, Some("Global theme not found."), &ctx, admin.site_id, "global")
            .await.into_response();
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            return render_appearance_list(
                &state, Some("No site selected. Run 'synaptic-cli site init' first."),
                &ctx, admin.site_id, "global",
            ).await.into_response();
        }
    };

    let dest = FsPath::new(themes_dir).join("sites").join(site_id.to_string()).join(&name);
    if dest.exists() {
        // Already copied — just send them to their themes.
        return Redirect::to("/admin/appearance?filter=my").into_response();
    }

    let source_owned = source.to_path_buf();
    let dest_owned = dest.to_path_buf();
    match tokio::task::spawn_blocking(move || copy_dir_all(&source_owned, &dest_owned)).await {
        Ok(Ok(())) => {
            tracing::info!("get_theme: copied global theme '{}' to site {}", name, site_id);
            Redirect::to("/admin/appearance?filter=my").into_response()
        }
        Ok(Err(e)) => {
            tracing::error!("get_theme: copy failed for '{}': {}", name, e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            render_appearance_list(&state, Some("Failed to get theme. Please try again."), &ctx, admin.site_id, "global")
                .await.into_response()
        }
        Err(e) => {
            tracing::error!("get_theme: task panicked: {:?}", e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            render_appearance_list(&state, Some("Failed to get theme. Please try again."), &ctx, admin.site_id, "global")
                .await.into_response()
        }
    }
}

// ── Theme file editor ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditorQuery {
    pub file: Option<String>,
    pub saved: Option<String>,
    pub restored: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct SaveFileForm {
    pub file: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct RestoreFileForm {
    pub file: String,
}

/// Encode a string for use as a URL query-parameter value.
/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
fn days_to_ymd(mut days: i64) -> (i64, i64, i64) {
    // Shift epoch to 1 Mar 0000 for simpler leap-year arithmetic.
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn url_encode_param(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => out.push(b as char),
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Find the filesystem path of a named theme (global or site-scoped).
///
/// When `prefer_site` is true (site admins), the site-scoped copy is returned
/// first so that a copied global theme is editable by its owner. When false
/// (super admins), the global directory takes priority.
fn resolve_theme_dir(themes_dir: &str, theme_name: &str, site_id: Option<Uuid>, prefer_site: bool) -> Option<PathBuf> {
    if theme_name.is_empty() || theme_name.contains("..") || theme_name.contains('/') || theme_name.contains('\\') {
        return None;
    }
    let global = FsPath::new(themes_dir).join("global").join(theme_name);
    let site = site_id.map(|id| FsPath::new(themes_dir).join("sites").join(id.to_string()).join(theme_name));

    if prefer_site {
        if let Some(ref s) = site { if s.is_dir() { return Some(s.clone()); } }
        if global.is_dir() { return Some(global); }
    } else {
        if global.is_dir() { return Some(global); }
        if let Some(s) = site { if s.is_dir() { return Some(s); } }
    }
    None
}

/// Returns true when `path` lives inside the global themes directory.
fn is_in_global_dir(path: &FsPath, themes_dir: &str) -> bool {
    let global_dir = FsPath::new(themes_dir).join("global");
    match (path.canonicalize(), global_dir.canonicalize()) {
        (Ok(p), Ok(g)) => p.starts_with(g),
        _ => false,
    }
}

/// Resolve a relative file path within a theme dir, guarding against traversal.
fn resolve_file_in_theme(theme_dir: &FsPath, rel_path: &str) -> Option<PathBuf> {
    if rel_path.contains('\0') || rel_path.is_empty() { return None; }
    let canonical_theme = theme_dir.canonicalize().ok()?;
    let canonical_file = theme_dir.join(rel_path).canonicalize().ok()?;
    if !canonical_file.starts_with(&canonical_theme) { return None; }
    if !canonical_file.is_file() { return None; }
    Some(canonical_file)
}

/// Build the `.bak` path for a given absolute file path.
fn bak_path_for(abs: &FsPath) -> PathBuf {
    let mut p = abs.to_path_buf();
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    p.set_file_name(name);
    p
}

/// Walk theme dir, returning relative paths (excluding `.bak` and hidden entries).
fn walk_theme_files(theme_dir: &FsPath) -> Vec<String> {
    let mut files = Vec::new();
    walk_dir_inner(theme_dir, theme_dir, &mut files);
    files.sort();
    files
}

fn walk_dir_inner(base: &FsPath, current: &FsPath, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') { continue; }
        if path.is_dir() {
            walk_dir_inner(base, &path, out);
        } else {
            if name_str.ends_with(".bak") { continue; }
            if name_str == "screenshot.png" { continue; }
            if name_str == "theme.toml" { continue; }
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

pub async fn edit_file(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme): Path<String>,
    Query(q): Query<EditorQuery>,
) -> Response {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id, !admin.caps.is_global_admin) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    let is_readonly = !admin.caps.is_global_admin && is_in_global_dir(&theme_dir, &state.config.themes_dir);

    let files = walk_theme_files(&theme_dir);

    let status_flash: Option<String> = if q.saved.is_some() {
        Some("File saved.".to_string())
    } else if q.restored.is_some() {
        Some("File restored from backup.".to_string())
    } else if let Some(ref e) = q.error {
        Some(format!("Template syntax error — file not saved. {}", e))
    } else {
        None
    };

    let (selected_rel, content, has_backup, file_err) = if let Some(ref rel) = q.file {
        tracing::debug!(theme = %theme, file = %rel, "editor: load file");
        if rel.contains('\0') || rel.contains("..") {
            tracing::warn!(theme = %theme, file = %rel, "editor: rejected path traversal attempt");
            (None, String::new(), false, Some("Invalid file path."))
        } else {
            match resolve_file_in_theme(&theme_dir, rel) {
                Some(abs) => {
                    let has_bak = bak_path_for(&abs).exists();
                    tracing::debug!(theme = %theme, file = %rel, abs = ?abs, has_bak, "editor: resolved file");
                    match fs::read_to_string(&abs) {
                        Ok(c) => {
                            tracing::debug!(theme = %theme, file = %rel, bytes = c.len(), "editor: file read ok");
                            (Some(rel.clone()), c, has_bak, None)
                        }
                        Err(e) => {
                            tracing::warn!(theme = %theme, file = %rel, err = %e, "editor: read_to_string failed");
                            (Some(rel.clone()), String::new(), has_bak,
                                Some("Could not read file (may be binary)."))
                        }
                    }
                }
                None => {
                    tracing::warn!(theme = %theme, file = %rel, "editor: resolve_file_in_theme returned None");
                    (Some(rel.clone()), String::new(), false, Some("File not found."))
                },
            }
        }
    } else {
        tracing::debug!(theme = %theme, "editor: no file selected, showing picker");
        (None, String::new(), false, None)
    };

    let effective_flash: Option<String> = file_err.map(|s| s.to_string()).or(status_flash);

    let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
        let abs = theme_dir.join(f);
        let has_bak = bak_path_for(&abs).exists();
        let edited_at = if has_bak {
            fs::metadata(&abs)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    // Format as "DD Mon YYYY HH:MM"
                    let secs_i = secs as i64;
                    let days_since_epoch = secs_i / 86400;
                    let time_of_day = secs_i % 86400;
                    let hh = time_of_day / 3600;
                    let mm = (time_of_day % 3600) / 60;
                    // Compute date from days since 1970-01-01
                    let (y, mo, d) = days_to_ymd(days_since_epoch);
                    let month = ["Jan","Feb","Mar","Apr","May","Jun",
                                 "Jul","Aug","Sep","Oct","Nov","Dec"]
                        [(mo - 1) as usize];
                    format!("{:02} {} {} {:02}:{:02}", d, month, y, hh, mm)
                })
        } else {
            None
        };
        admin::pages::appearance::EditorFile {
            rel_path: f.clone(),
            is_selected: selected_rel.as_deref() == Some(f.as_str()),
            has_backup: has_bak,
            edited_at,
        }
    }).collect();

    Html(admin::pages::appearance::render_theme_editor(
        &theme,
        &editor_files,
        selected_rel.as_deref(),
        &content,
        has_backup,
        effective_flash.as_deref(),
        &ctx,
        is_readonly,
    )).into_response()
}

pub async fn save_file(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme): Path<String>,
    Form(form): Form<SaveFileForm>,
) -> Response {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id, !admin.caps.is_global_admin) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    if !admin.caps.is_global_admin && is_in_global_dir(&theme_dir, &state.config.themes_dir) {
        return Redirect::to(&format!("/admin/appearance/editor/{}", url_encode_param(&theme))).into_response();
    }

    let redirect_base = format!(
        "/admin/appearance/editor/{}?file={}",
        url_encode_param(&theme), url_encode_param(&form.file)
    );

    let Some(abs_path) = resolve_file_in_theme(&theme_dir, &form.file) else {
        return Redirect::to(&redirect_base).into_response();
    };

    // Validate Tera syntax before touching disk — reject HTML files that won't parse.
    // Load the full theme into a scratch Tera instance first so that {% extends %}
    // and {% include %} references (e.g. base.html) resolve correctly during validation.
    if form.file.ends_with(".html") {
        let templates_glob = theme_dir.join("templates").join("**").join("*.html");
        let glob_str = templates_glob.to_string_lossy().to_string();
        let mut test_tera = tera::Tera::new(&glob_str).unwrap_or_default();
        // Override the file being saved with the new content so we test the edited version.
        if let Err(e) = test_tera.add_raw_template("__validate__", &form.content) {
            // Strip ANSI colour codes Tera sometimes adds to error messages.
            let msg = e.to_string();
            let clean: String = msg.chars().filter(|c| c.is_ascii() && (*c >= ' ' || *c == '\n')).collect();
            let encoded = url_encode_param(clean.trim());
            return Redirect::to(&format!("{}&error={}", redirect_base, encoded)).into_response();
        }
    }

    let bak = bak_path_for(&abs_path);
    if !bak.exists() {
        if let Err(e) = fs::copy(&abs_path, &bak) {
            tracing::warn!("theme editor: backup failed for {:?}: {e}", abs_path);
        }
    }

    if let Err(e) = fs::write(&abs_path, form.content.as_bytes()) {
        tracing::error!("theme editor: write failed for {:?}: {e}", abs_path);
        return Redirect::to(&redirect_base).into_response();
    }

    if form.file.ends_with(".html") {
        let active = state.active_theme_for_site(admin.site_id);
        if active == theme {
            if let Err(e) = state.templates.switch_theme(&theme) {
                tracing::warn!("theme editor: Tera reload after save failed: {e}");
            }
        }
    }

    Redirect::to(&format!("{}&saved=1", redirect_base)).into_response()
}

pub async fn restore_file(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme): Path<String>,
    Form(form): Form<RestoreFileForm>,
) -> Response {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id, !admin.caps.is_global_admin) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    if !admin.caps.is_global_admin && is_in_global_dir(&theme_dir, &state.config.themes_dir) {
        return Redirect::to(&format!("/admin/appearance/editor/{}", url_encode_param(&theme))).into_response();
    }

    let redirect_base = format!(
        "/admin/appearance/editor/{}?file={}",
        url_encode_param(&theme), url_encode_param(&form.file)
    );

    let Some(abs_path) = resolve_file_in_theme(&theme_dir, &form.file) else {
        return Redirect::to(&redirect_base).into_response();
    };

    let bak = bak_path_for(&abs_path);
    if !bak.exists() {
        return Redirect::to(&redirect_base).into_response();
    }

    if let Err(e) = fs::copy(&bak, &abs_path) {
        tracing::error!("theme editor: restore failed for {:?}: {e}", abs_path);
        return Redirect::to(&redirect_base).into_response();
    }

    if let Err(e) = fs::remove_file(&bak) {
        tracing::warn!("theme editor: could not delete backup {:?} after restore: {e}", bak);
    }

    if form.file.ends_with(".html") {
        let active = state.active_theme_for_site(admin.site_id);
        if active == theme {
            if let Err(e) = state.templates.switch_theme(&theme) {
                tracing::warn!("theme editor: Tera reload after restore failed: {e}");
            }
        }
    }

    Redirect::to(&format!("{}&restored=1", redirect_base)).into_response()
}
// ── Delete file ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteFileForm {
    pub file: String,
}

pub async fn delete_file(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(theme): Path<String>,
    Form(form): Form<DeleteFileForm>,
) -> Response {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id, !admin.caps.is_global_admin) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    // Guard: required templates cannot be deleted.
    let rel = form.file.trim().to_string();
    if REQUIRED_TEMPLATES.contains(&rel.as_str()) {
        let files = walk_theme_files(&theme_dir);
        let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
            admin::pages::appearance::EditorFile {
                rel_path: f.clone(),
                is_selected: f == &rel,
                has_backup: bak_path_for(&theme_dir.join(f)).exists(),
                edited_at: None,
            }
        }).collect();
        return Html(admin::pages::appearance::render_theme_editor(
            &theme, &editor_files, Some(&rel), "", false,
            Some("Required theme templates cannot be deleted."), &ctx, false,
        )).into_response();
    }

    let Some(abs_path) = resolve_file_in_theme(&theme_dir, &rel) else {
        let files = walk_theme_files(&theme_dir);
        let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
            admin::pages::appearance::EditorFile {
                rel_path: f.clone(),
                is_selected: false,
                has_backup: bak_path_for(&theme_dir.join(f)).exists(),
                edited_at: None,
            }
        }).collect();
        return Html(admin::pages::appearance::render_theme_editor(
            &theme, &editor_files, None, "", false,
            Some("File not found."), &ctx, false,
        )).into_response();
    };

    // Remove .bak file if present.
    let bak = bak_path_for(&abs_path);
    if bak.exists() {
        if let Err(e) = fs::remove_file(&bak) {
            tracing::warn!("delete_file: could not remove backup {:?}: {}", bak, e);
        }
    }

    if let Err(e) = fs::remove_file(&abs_path) {
        tracing::error!("delete_file: remove failed for {:?}: {}", abs_path, e);
        let files = walk_theme_files(&theme_dir);
        let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
            admin::pages::appearance::EditorFile {
                rel_path: f.clone(),
                is_selected: f == &rel,
                has_backup: bak_path_for(&theme_dir.join(f)).exists(),
                edited_at: None,
            }
        }).collect();
        return Html(admin::pages::appearance::render_theme_editor(
            &theme, &editor_files, Some(&rel), "", false,
            Some("Failed to delete file. Please try again."), &ctx, false,
        )).into_response();
    }

    tracing::info!("theme file deleted: theme={} file={}", theme, rel);
    Redirect::to(&format!("/admin/appearance/editor/{}", url_encode_param(&theme))).into_response()
}

/// Recursively copy a directory tree from `src` to `dst`.
pub(crate) fn copy_dir_all(src: &FsPath, dst: &FsPath) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

// ── Shared list renderer ───────────────────────────────────────────────────────

async fn render_appearance_list(
    state: &AppState,
    flash: Option<&str>,
    ctx: &admin::PageContext,
    site_id: Option<Uuid>,
    filter: &str,
) -> Html<String> {
    let themes_dir = &state.config.themes_dir;

    // Scope the active_theme query by site_id.
    // When site_id is None (super_admin with no site selected) we use an empty string
    // so no global theme is incorrectly marked "active" — deletability is determined
    // solely by in_use == 0 in that case.
    let active_theme_from_db: String = if let Some(sid) = site_id {
        sqlx::query_scalar(
            "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'active_theme'",
        )
        .bind(sid)
        .fetch_optional(&state.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to read active_theme from DB: {:?}", e);
            None
        })
        .unwrap_or_else(|| "default".to_string())
    } else {
        String::new()
    };

    let global_dir = FsPath::new(themes_dir).join("global");
    let site_dir = site_id.map(|id| FsPath::new(themes_dir).join("sites").join(id.to_string()));

    let mut themes = Vec::new();
    scan_theme_dir(&global_dir, &active_theme_from_db, "global", &mut themes);
    if let Some(ref sd) = site_dir {
        scan_theme_dir(sd, &active_theme_from_db, "site", &mut themes);
    }

    // Fetch usage counts: how many sites have each theme as their active_theme.
    let usage_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT value, COUNT(*) FROM site_settings WHERE key = 'active_theme' GROUP BY value",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let usage_counts: std::collections::HashMap<String, usize> = usage_rows
        .into_iter()
        .map(|(name, count)| (name, count as usize))
        .collect();

    // Compute can_delete and in_use_by for each theme.
    for theme in &mut themes {
        let in_use = usage_counts.get(&theme.name).copied().unwrap_or(0);
        theme.in_use_by = if theme.source == "global" { in_use } else { 0 };
        theme.can_delete = if theme.source == "global" {
            // Super admin only; not in use on any site.
            // in_use == 0 already guarantees it isn't active anywhere, so we
            // don't need a separate !theme.active check here.
            ctx.is_global_admin && in_use == 0
        } else {
            // Site theme: deletable as long as it isn't the currently active theme.
            !theme.active
        };
    }

    // For the global filter view, mark which themes the user already has a site copy of.
    if filter == "global" && !ctx.is_global_admin {
        if let Some(sid) = site_id {
            let site_dir = FsPath::new(themes_dir).join("sites").join(sid.to_string());
            for theme in &mut themes {
                if theme.source == "global" {
                    theme.has_site_copy = site_dir.join(&theme.name).is_dir();
                }
            }
        }
    }

    // Apply filter. Super admins always see everything (global themes are their domain).
    // Site admins see their own themes (source=="site") by default, or global themes
    // when explicitly requesting filter=global.
    let mut themes: Vec<ThemeInfo> = if ctx.is_global_admin {
        themes
    } else if filter == "global" {
        themes.into_iter().filter(|t| t.source == "global").collect()
    } else {
        // "my" — site themes only
        themes.into_iter().filter(|t| t.source == "site").collect()
    };

    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Html(render_with_flash(&themes, flash, ctx, filter))
}

/// Scan a theme directory and append found themes to `themes`.
/// `source` is `"global"` or `"site"`.
fn scan_theme_dir(dir: &FsPath, active_theme: &str, source: &str, themes: &mut Vec<ThemeInfo>) {
    let Ok(entries) = fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        if dir_name.starts_with('.') {
            continue;
        }
        let toml_path = path.join("theme.toml");
        if let Ok(toml_content) = fs::read_to_string(&toml_path) {
            if let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) {
                if let Some(theme_section) = parsed.get("theme").and_then(|v| v.as_table()) {
                    let name = theme_section.get("name").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
                    let version = theme_section.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let description = theme_section.get("description").and_then(|v| v.as_str()).unwrap_or("No description").to_string();
                    let author = theme_section.get("author").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                    let has_screenshot = path.join("screenshot.png").exists();
                    themes.push(ThemeInfo {
                        name: name.clone(),
                        version,
                        description,
                        author,
                        active: name == active_theme,
                        has_screenshot,
                        source: source.to_string(),
                        can_delete: false,    // computed after scanning in render_appearance_list
                        in_use_by: 0,         // computed after scanning in render_appearance_list
                        has_site_copy: false, // computed below for global filter view
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_test_theme(dir: &FsPath, name: &str) {
        let theme_dir = dir.join(name);
        std::fs::create_dir_all(theme_dir.join("templates")).unwrap();
        std::fs::write(
            theme_dir.join("theme.toml"),
            format!(
                "[theme]\nname = \"{}\"\nversion = \"1.0\"\ndescription = \"Test theme\"\nauthor = \"Tester\"\n",
                name
            ),
        )
        .unwrap();
    }

    fn unique_tmp(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("synaptic_theme_test_{}_{}", label, Uuid::new_v4()))
    }

    #[test]
    fn theme_discovery_finds_global_themes() {
        let tmp = unique_tmp("global");
        let global_dir = tmp.join("global");
        std::fs::create_dir_all(&global_dir).unwrap();
        make_test_theme(&global_dir, "myblue");

        let mut themes = Vec::new();
        scan_theme_dir(&global_dir, "default", "global", &mut themes);

        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "myblue");
        assert_eq!(themes[0].source, "global");
        assert!(!themes[0].active);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn theme_discovery_finds_site_themes() {
        let tmp = unique_tmp("site");
        let site_id = Uuid::new_v4();
        let site_dir = tmp.join("sites").join(site_id.to_string());
        std::fs::create_dir_all(&site_dir).unwrap();
        make_test_theme(&site_dir, "clienttheme");

        let mut themes = Vec::new();
        scan_theme_dir(&site_dir, "clienttheme", "site", &mut themes);

        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "clienttheme");
        assert_eq!(themes[0].source, "site");
        assert!(themes[0].active);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn theme_discovery_excludes_other_site_themes() {
        let tmp = unique_tmp("isolation");
        let site_a = Uuid::new_v4();
        let site_b = Uuid::new_v4();

        let dir_a = tmp.join("sites").join(site_a.to_string());
        let dir_b = tmp.join("sites").join(site_b.to_string());
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        make_test_theme(&dir_a, "theme_for_a");
        make_test_theme(&dir_b, "theme_for_b");

        // Site A should only see its own themes.
        let mut themes = Vec::new();
        scan_theme_dir(&dir_a, "default", "site", &mut themes);

        assert_eq!(themes.len(), 1, "site A should see exactly 1 theme");
        assert_eq!(themes[0].name, "theme_for_a");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn theme_discovery_skips_dot_dirs() {
        let tmp = unique_tmp("dotdir");
        let global_dir = tmp.join("global");
        std::fs::create_dir_all(&global_dir).unwrap();

        // Real theme.
        make_test_theme(&global_dir, "realtheme");
        // Hidden temp dir (as created during upload).
        std::fs::create_dir_all(global_dir.join(".theme_upload_tmp_12345")).unwrap();

        let mut themes = Vec::new();
        scan_theme_dir(&global_dir, "default", "global", &mut themes);

        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "realtheme");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
