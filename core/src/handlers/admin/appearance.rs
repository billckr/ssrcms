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
use admin::pages::appearance::{ThemeInfo, render_with_flash};

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

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    render_appearance_list(&state, None, &ctx, admin.site_id).await.into_response()
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
        return render_appearance_list(&state, Some("Invalid theme name."), &ctx, admin.site_id).await.into_response();
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
            return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id).await.into_response();
        }
    } else {
        tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id).await.into_response();
    };

    // Path traversal guard: theme must stay within global/ or sites/<id>/.
    let canonical_theme = match theme_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id).await.into_response(),
    };
    let canonical_global = global_dir.canonicalize().unwrap_or_default();
    let canonical_site = site_dir
        .as_ref()
        .and_then(|sd| sd.canonicalize().ok())
        .unwrap_or_default();
    if !canonical_theme.starts_with(&canonical_global) && !canonical_theme.starts_with(&canonical_site) {
        tracing::warn!("activate path traversal attempt: theme_name={:?}", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id).await.into_response();
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            tracing::warn!("theme activate: no site selected, cannot save per-site setting");
            return render_appearance_list(&state, Some("No site selected. Run 'synaptic-cli site init' first."), &ctx, admin.site_id).await.into_response();
        }
    };

    if let Err(e) = set_site_setting(&state.db, site_id, "active_theme", &form.theme).await {
        tracing::error!("failed to save active_theme to DB: {:?}", e);
        return render_appearance_list(&state, Some("Failed to activate theme. Please try again."), &ctx, admin.site_id).await.into_response();
    }

    if let Err(e) = state.templates.switch_theme(&form.theme) {
        tracing::error!("failed to switch theme to '{}': {:?}", form.theme, e);
        return render_appearance_list(&state, Some("Theme files could not be loaded. Please try again."), &ctx, admin.site_id).await.into_response();
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
            return render_appearance_list(&state, Some($msg), &ctx, admin.site_id)
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
    render_appearance_list(&state, Some(&format!("Theme '{}' deleted.", form.theme)), &ctx, admin.site_id)
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
                    return render_appearance_list(&state, Some("Upload too large. Maximum size is 50 MB."), &ctx, admin.site_id)
                        .await
                        .into_response();
                }
                Err(e) => {
                    tracing::error!("failed to read theme zip field: {:?}", e);
                    return render_appearance_list(&state, Some("Failed to read uploaded file. Please try again."), &ctx, admin.site_id)
                        .await
                        .into_response();
                }
            }
        }
    }

    let zip_bytes = match zip_bytes {
        Some(b) => b,
        None => return render_appearance_list(&state, Some("No file received."), &ctx, admin.site_id).await.into_response(),
    };

    // Route the upload to the correct subdirectory.
    // Super admins upload to themes/global/; site admins upload to themes/sites/<site_id>/.
    let themes_parent = state.config.themes_dir.clone();
    let target_dir = if admin.caps.is_global_admin {
        format!("{}/global", themes_parent)
    } else if let Some(sid) = admin.site_id {
        format!("{}/sites/{}", themes_parent, sid)
    } else {
        return render_appearance_list(&state, Some("No site selected. Cannot install theme."), &ctx, admin.site_id)
            .await
            .into_response();
    };

    // Ensure target directory exists.
    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        tracing::error!("failed to create theme target dir '{}': {}", target_dir, e);
        return render_appearance_list(&state, Some("Failed to prepare theme directory."), &ctx, admin.site_id)
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
                return render_appearance_list(&state, Some("Theme installed but could not be reloaded. Please restart the server."), &ctx, admin.site_id)
                    .await
                    .into_response();
            }
            tracing::info!("reloaded active theme '{}' after installing '{}'", active, theme_name);

            render_appearance_list(&state, Some(&format!("Theme '{}' installed successfully.", theme_name)), &ctx, admin.site_id)
                .await
                .into_response()
        }
        Ok(Err(msg)) => {
            tracing::warn!("theme upload rejected: {}", msg);
            render_appearance_list(&state, Some(&msg), &ctx, admin.site_id).await.into_response()
        }
        Err(e) => {
            tracing::error!("theme upload task panicked: {:?}", e);
            render_appearance_list(&state, Some("Installation failed. Please try again."), &ctx, admin.site_id)
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
fn find_theme_prefix(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Result<String, String> {
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip: {}", e))?;
        let name = entry.name();
        if name == "theme.toml" {
            return Ok(String::new());
        }
        if name.ends_with("/theme.toml") {
            let prefix = &name[..name.len() - "theme.toml".len()];
            return Ok(prefix.to_string());
        }
    }
    Err("theme.toml not found in zip. Is this a valid Synaptic Signals theme?".to_string())
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
// ── Theme file editor ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditorQuery {
    pub file: Option<String>,
    pub saved: Option<String>,
    pub restored: Option<String>,
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
fn resolve_theme_dir(themes_dir: &str, theme_name: &str, site_id: Option<Uuid>) -> Option<PathBuf> {
    if theme_name.is_empty() || theme_name.contains("..") || theme_name.contains('/') || theme_name.contains('\\') {
        return None;
    }
    let global = FsPath::new(themes_dir).join("global").join(theme_name);
    if global.is_dir() { return Some(global); }
    if let Some(id) = site_id {
        let site = FsPath::new(themes_dir).join("sites").join(id.to_string()).join(theme_name);
        if site.is_dir() { return Some(site); }
    }
    None
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

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id)
            .await.into_response();
    };

    let files = walk_theme_files(&theme_dir);

    let status_flash: Option<&str> = if q.saved.is_some() {
        Some("File saved.")
    } else if q.restored.is_some() {
        Some("File restored from backup.")
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

    let effective_flash = file_err.or(status_flash);

    let editor_files: Vec<admin::pages::appearance::EditorFile> = files.iter().map(|f| {
        let has_bak = bak_path_for(&theme_dir.join(f)).exists();
        admin::pages::appearance::EditorFile {
            rel_path: f.clone(),
            is_selected: selected_rel.as_deref() == Some(f.as_str()),
            has_backup: has_bak,
        }
    }).collect();

    Html(admin::pages::appearance::render_theme_editor(
        &theme,
        &editor_files,
        selected_rel.as_deref(),
        &content,
        has_backup,
        effective_flash,
        &ctx,
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

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id)
            .await.into_response();
    };

    let redirect_base = format!(
        "/admin/appearance/editor/{}?file={}",
        url_encode_param(&theme), url_encode_param(&form.file)
    );

    let Some(abs_path) = resolve_file_in_theme(&theme_dir, &form.file) else {
        return Redirect::to(&redirect_base).into_response();
    };

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

    let Some(theme_dir) = resolve_theme_dir(&state.config.themes_dir, &theme, admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id)
            .await.into_response();
    };

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
// ── Shared list renderer ───────────────────────────────────────────────────────

async fn render_appearance_list(
    state: &AppState,
    flash: Option<&str>,
    ctx: &admin::PageContext,
    site_id: Option<Uuid>,
) -> Html<String> {
    let themes_dir = &state.config.themes_dir;

    // Step 11 fix: scope the active_theme query by site_id.
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
        "default".to_string()
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
            // Super admin only; not active anywhere.
            ctx.is_global_admin && !theme.active && in_use == 0
        } else {
            // Site theme: either admin type can delete, as long as it's not active.
            !theme.active
        };
    }

    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Html(render_with_flash(&themes, flash, ctx))
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
                        can_delete: false, // computed after scanning in render_appearance_list
                        in_use_by: 0,      // computed after scanning in render_appearance_list
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
