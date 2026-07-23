//! The in-browser theme file editor: browsing, creating, editing, restoring,
//! and deleting individual template/CSS/JS files within a theme. Split out of
//! appearance.rs, which also owns the theme list/activate/delete/screenshot
//! handlers and the library-wide operations (upload, create, get, publish).

use axum::{
    extract::{Path, Query, State, Form},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

use super::appearance::{render_appearance_list, url_encode_param, REQUIRED_TEMPLATES};

#[derive(Deserialize)]
pub struct NewFileForm {
    pub filename: String,
    pub ext: String,
    pub source: Option<String>,
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

    let source = form.source.as_deref().unwrap_or("site");
    let Some(theme_dir) = resolve_theme_dir_by_source(&state.config.themes_dir, &state.config.sites_dir, &theme, Some(source), admin.site_id) else {
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
            &theme, &editor_files, None, "", false, Some(msg), &ctx, false, source,
        )).into_response()
    };

    if !admin.caps.is_global_admin && (is_in_global_dir(&theme_dir, &state.config.themes_dir) || is_in_private_dir(&theme_dir, &state.config.themes_dir)) {
        return editor_err("Global themes cannot be modified. Copy this theme to your site first.");
    }

    // name = bare name the user typed (e.g. "partials/header" or "custom")
    // ext  = dropdown selection: ".html", ".css", ".js", or ".xml"
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
        ".xml"  => ("templates", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
        _       => return editor_err("Invalid file type. Choose .html, .css, .js, or .xml."),
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
        "/admin/appearance/editor/{}?source={}&file={}",
        url_encode_param(&theme),
        source,
        url_encode_param(&rel),
    )).into_response()
}

// ── Theme file editor ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditorQuery {
    pub file: Option<String>,
    pub saved: Option<String>,
    pub restored: Option<String>,
    pub error: Option<String>,
    /// Which directory the theme lives in: "site", "global", or "private".
    /// Set by the Edit button on each theme card; threads through all editor
    /// operations so saves always target the correct copy of the theme.
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct SaveFileForm {
    pub file: String,
    pub content: String,
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct RestoreFileForm {
    pub file: String,
    pub source: Option<String>,
}

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

/// Find the filesystem path of a named theme (global or site-scoped).
///
/// Resolve a theme directory using an explicit source hint ("site", "global",
/// or "private"). Called by every editor handler so writes always land in the
/// correct copy of the theme rather than whichever copy happens to win in the
/// generic prefer_site search.
///
/// - `"site"` → `sites/<site_id>/themes/<name>/`
/// - `"global"` → `themes/global/<name>/`
/// - `"private"` → `themes/private/<name>/`
/// - `None` or unknown → falls back to site copy first, then global, then private
fn resolve_theme_dir_by_source(
    themes_dir: &str,
    sites_dir: &str,
    theme_name: &str,
    source: Option<&str>,
    site_id: Option<Uuid>,
) -> Option<PathBuf> {
    if theme_name.is_empty() || theme_name.contains("..") || theme_name.contains('/') || theme_name.contains('\\') {
        return None;
    }
    let dir = match source.unwrap_or("site") {
        "global" => FsPath::new(themes_dir).join("global").join(theme_name),
        "private" => FsPath::new(themes_dir).join("private").join(theme_name),
        _ => {
            // "site" or fallback — use the site-specific copy if one exists,
            // then global, then private (super admins may have no site copy yet)
            if let Some(id) = site_id {
                let s = FsPath::new(sites_dir).join(id.to_string()).join("themes").join(theme_name);
                if s.is_dir() { return Some(s); }
            }
            let g = FsPath::new(themes_dir).join("global").join(theme_name);
            if g.is_dir() { return Some(g); }
            FsPath::new(themes_dir).join("private").join(theme_name)
        }
    };
    if dir.is_dir() { Some(dir) } else { None }
}

/// Returns true when `path` lives inside the global themes directory.
fn is_in_global_dir(path: &FsPath, themes_dir: &str) -> bool {
    let global_dir = FsPath::new(themes_dir).join("global");
    match (path.canonicalize(), global_dir.canonicalize()) {
        (Ok(p), Ok(g)) => p.starts_with(g),
        _ => false,
    }
}

/// Returns true when `path` lives inside the private themes directory.
fn is_in_private_dir(path: &FsPath, themes_dir: &str) -> bool {
    let private_dir = FsPath::new(themes_dir).join("private");
    match (path.canonicalize(), private_dir.canonicalize()) {
        (Ok(p), Ok(d)) => p.starts_with(d),
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
            // Only show editable theme files — allowlist of extensions.
            // Everything else (images, zips, .bak, Zone.Identifier, theme.toml, etc.) is excluded.
            let editable = name_str.ends_with(".html")
                || name_str.ends_with(".css")
                || name_str.ends_with(".js")
                || name_str.ends_with(".xml");
            if !editable { continue; }
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

    let source = q.source.as_deref().unwrap_or("site");
    let Some(theme_dir) = resolve_theme_dir_by_source(&state.config.themes_dir, &state.config.sites_dir, &theme, Some(source), admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    let is_readonly = !admin.caps.is_global_admin && (is_in_global_dir(&theme_dir, &state.config.themes_dir) || is_in_private_dir(&theme_dir, &state.config.themes_dir));

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
        source,
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

    let source = form.source.as_deref().unwrap_or("site");
    let Some(theme_dir) = resolve_theme_dir_by_source(&state.config.themes_dir, &state.config.sites_dir, &theme, Some(source), admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    if !admin.caps.is_global_admin && (is_in_global_dir(&theme_dir, &state.config.themes_dir) || is_in_private_dir(&theme_dir, &state.config.themes_dir)) {
        return Redirect::to(&format!("/admin/appearance/editor/{}?source={}", url_encode_param(&theme), source)).into_response();
    }

    let redirect_base = format!(
        "/admin/appearance/editor/{}?source={}&file={}",
        url_encode_param(&theme), source, url_encode_param(&form.file)
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

    // Skip write entirely if content hasn't changed — prevents a spurious .bak
    // being created and the file being marked as modified when nothing was edited.
    if let Ok(existing) = fs::read_to_string(&abs_path) {
        if existing == form.content {
            return Redirect::to(&format!("{}&saved=1", redirect_base)).into_response();
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
        // Invalidate the cached Tera instance for this (theme, site) pair so it is
        // reloaded from disk on the next request — picking up the edit from the
        // correct copy of the theme.
        state.templates.invalidate_theme(&theme, admin.site_id);
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

    let source = form.source.as_deref().unwrap_or("site");
    let Some(theme_dir) = resolve_theme_dir_by_source(&state.config.themes_dir, &state.config.sites_dir, &theme, Some(source), admin.site_id) else {
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my")
            .await.into_response();
    };

    if !admin.caps.is_global_admin && (is_in_global_dir(&theme_dir, &state.config.themes_dir) || is_in_private_dir(&theme_dir, &state.config.themes_dir)) {
        return Redirect::to(&format!("/admin/appearance/editor/{}?source={}", url_encode_param(&theme), source)).into_response();
    }

    let redirect_base = format!(
        "/admin/appearance/editor/{}?source={}&file={}",
        url_encode_param(&theme), source, url_encode_param(&form.file)
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
        state.templates.invalidate_theme(&theme, admin.site_id);
    }

    Redirect::to(&format!("{}&restored=1", redirect_base)).into_response()
}

// ── Delete file ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteFileForm {
    pub file: String,
    pub source: Option<String>,
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

    let source = form.source.as_deref().unwrap_or("site");
    let Some(theme_dir) = resolve_theme_dir_by_source(&state.config.themes_dir, &state.config.sites_dir, &theme, Some(source), admin.site_id) else {
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
            Some("Required theme templates cannot be deleted."), &ctx, false, source,
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
            Some("File not found."), &ctx, false, source,
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
            Some("Failed to delete file. Please try again."), &ctx, false, source,
        )).into_response();
    }

    tracing::info!("theme file deleted: theme={} file={}", theme, rel);
    Redirect::to(&format!("/admin/appearance/editor/{}?source={}", url_encode_param(&theme), source)).into_response()
}
