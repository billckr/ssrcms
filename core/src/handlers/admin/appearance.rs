//! Theme list, activation, deletion, and screenshots — plus the helpers shared
//! across the appearance handler modules:
//! - `appearance_upload.rs` — zip install and "create from default"
//! - `appearance_publish.rs` — copying between library tiers (get/publish)
//! - `appearance_editor.rs` — the in-browser template file editor

use axum::{
    body::Body,
    extract::{Path, Query, State, Form},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use uuid::Uuid;

use crate::app_state::{AppState, set_site_setting};
use crate::middleware::admin_auth::AdminUser;
use admin::pages::appearance::{ThemeInfo, render_with_flash};

/// Required template files every valid theme must provide.
pub(crate) const REQUIRED_TEMPLATES: &[&str] = &[
    "templates/base.html",
    "templates/index.html",
    "templates/single.html",
    "templates/page.html",
    "templates/archive.html",
    "templates/search.html",
    "templates/404.html",
];

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

    let sites_dir   = &state.config.sites_dir;
    let global_dir  = FsPath::new(themes_dir).join("global");
    let private_dir = FsPath::new(themes_dir).join("private");
    let site_dir    = admin.site_id.map(|id| FsPath::new(sites_dir).join(id.to_string()).join("themes"));

    // Resolve which directory the theme lives in.
    let theme_path = if global_dir.join(&form.theme).is_dir() {
        global_dir.join(&form.theme)
    } else if admin.caps.is_global_admin && private_dir.join(&form.theme).is_dir() {
        // Only super_admin may activate private themes.
        private_dir.join(&form.theme)
    } else if let Some(ref sd) = site_dir {
        if sd.join(&form.theme).is_dir() {
            sd.join(&form.theme)
        } else {
            tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
            return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
        }
    } else {
        tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
    };

    // Path traversal guard: theme must live within global/, private/, or sites/<id>/.
    let canonical_theme = match theme_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response(),
    };
    let canonical_global  = global_dir.canonicalize().unwrap_or_default();
    let canonical_private = private_dir.canonicalize().unwrap_or_default();
    let canonical_site = site_dir
        .as_ref()
        .and_then(|sd| sd.canonicalize().ok())
        .unwrap_or_default();
    let in_allowed_dir = canonical_theme.starts_with(&canonical_global)
        || (admin.caps.is_global_admin && canonical_theme.starts_with(&canonical_private))
        || canonical_theme.starts_with(&canonical_site);
    if !in_allowed_dir {
        tracing::warn!("activate path traversal attempt: theme_name={:?}", form.theme);
        return render_appearance_list(&state, Some("Theme not found."), &ctx, admin.site_id, "my").await.into_response();
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => {
            tracing::warn!("theme activate: no site selected, cannot save per-site setting");
            return render_appearance_list(&state, Some("No site selected."), &ctx, admin.site_id, "my").await.into_response();
        }
    };

    // Copy global or private themes to the site folder before activating so
    // the theme shows up in "My Themes" for both site_admin and super_admin.
    // Skip the copy if a site-scoped copy already exists.
    let is_from_global  = canonical_theme.starts_with(&canonical_global);
    let is_from_private = admin.caps.is_global_admin && canonical_theme.starts_with(&canonical_private);
    if is_from_global || is_from_private {
        let site_copy = FsPath::new(&state.config.sites_dir)
            .join(site_id.to_string())
            .join("themes")
            .join(&form.theme);
        if !site_copy.exists() {
            let src = canonical_theme.clone();
            let dst = site_copy.clone();
            match tokio::task::spawn_blocking(move || copy_dir_all(&src, &dst)).await {
                Ok(Ok(())) => tracing::info!("auto-copied theme '{}' to site {}", form.theme, site_id),
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
    /// "site", "global", or "private" — from the hidden field in the card form.
    pub source: Option<String>,
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
    let sites_dir  = &state.config.sites_dir;
    let global_path  = FsPath::new(themes_dir).join("global").join(&form.theme);
    let private_path = FsPath::new(themes_dir).join("private").join(&form.theme);
    let site_path    = admin.site_id
        .map(|id| FsPath::new(sites_dir).join(id.to_string()).join("themes").join(&form.theme));

    // Determine where the theme lives using the explicit source hint from the form.
    // Falling back to filesystem discovery is a security risk — if source is
    // "site" but the global copy is found first, we'd delete the wrong directory.
    let (theme_path, theme_source) = match form.source.as_deref().unwrap_or("site") {
        "global" => {
            if global_path.is_dir() { (global_path, "global") } else { err!("Theme not found."); }
        }
        "private" => {
            if private_path.is_dir() { (private_path, "private") } else { err!("Theme not found."); }
        }
        _ => {
            // "site" — only look in the site-scoped folder.
            match site_path {
                Some(ref sp) if sp.is_dir() => (sp.clone(), "site"),
                _ => err!("Theme not found."),
            }
        }
    };

    // Authorization: only super_admin may delete global or private themes.
    if (theme_source == "global" || theme_source == "private") && !admin.caps.is_global_admin {
        err!("Only super admins can delete this theme.");
    }

    // Active theme guard (server-side).
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
    if theme_source == "global" {
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
    let expected_parent = match theme_source {
        "global" => match FsPath::new(themes_dir).join("global").canonicalize() {
            Ok(p) => p,
            Err(_) => err!("Theme not found."),
        },
        "private" => match FsPath::new(themes_dir).join("private").canonicalize() {
            Ok(p) => p,
            Err(_) => err!("Theme not found."),
        },
        _ => {
            let sid = admin.site_id.unwrap();
            match FsPath::new(&state.config.sites_dir).join(sid.to_string()).join("themes").canonicalize() {
                Ok(p) => p,
                Err(_) => err!("Theme not found."),
            }
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
    let themes_dir  = FsPath::new(&state.config.themes_dir);
    let global_dir  = themes_dir.join("global");
    let private_dir = themes_dir.join("private");
    let site_dir    = admin.site_id.map(|id| FsPath::new(&state.config.sites_dir).join(id.to_string()).join("themes"));

    // Try global, then private (super_admin only), then site dir.
    let mut dirs_to_search: Vec<PathBuf> = vec![global_dir];
    if admin.caps.is_global_admin {
        dirs_to_search.push(private_dir);
    }
    if let Some(sd) = site_dir {
        dirs_to_search.push(sd);
    }

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

/// Encode a string for use as a URL query-parameter value.
pub(crate) fn url_encode_param(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => out.push(b as char),
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
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

pub(crate) async fn render_appearance_list(
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

    let global_dir  = FsPath::new(themes_dir).join("global");
    let private_dir = FsPath::new(themes_dir).join("private");
    let site_dir    = site_id.map(|id| FsPath::new(&state.config.sites_dir).join(id.to_string()).join("themes"));

    let mut themes = Vec::new();
    scan_theme_dir(&global_dir, &active_theme_from_db, "global", &mut themes);
    // Private themes are only loaded for super_admin.
    if ctx.is_global_admin {
        scan_theme_dir(&private_dir, &active_theme_from_db, "private", &mut themes);
    }
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
        theme.can_delete = match theme.source.as_str() {
            "global" => ctx.is_global_admin && in_use == 0,
            "private" => ctx.is_global_admin && !theme.active,
            _ => !theme.active, // site theme
        };
    }

    // For the global/private filter views, mark which themes already have a
    // site-scoped copy. Applies to all users so "Get Theme" / "In My Themes"
    // renders correctly for super_admin and site_admin alike.
    if filter == "global" || filter == "private" {
        if let Some(sid) = site_id {
            let site_dir = FsPath::new(&state.config.sites_dir).join(sid.to_string()).join("themes");
            for theme in &mut themes {
                theme.has_site_copy = site_dir.join(&theme.name).is_dir();
            }
        }
    }

    // For the private filter view, mark which private themes already have a
    // global copy so the "Make Global" button can show a confirmation.
    if filter == "private" {
        for theme in &mut themes {
            if theme.source == "private" {
                theme.has_global_copy = FsPath::new(themes_dir).join("global").join(&theme.name).is_dir();
            }
        }
    }

    // Mark site copies of private themes so the Private badge stays visible
    // in My Themes even after the theme has been copied out of themes/private/.
    if filter == "my" || filter.is_empty() {
        let private_dir = FsPath::new(themes_dir).join("private");
        for theme in &mut themes {
            if theme.source == "site" && private_dir.join(&theme.name).is_dir() {
                theme.is_private_origin = true;
            }
        }
    }

    // Apply filter.
    // Both super_admin and site_admin: "my" = site-scoped copies only.
    // Activating from Global/Private auto-copies to the site folder first,
    // so the activated theme will always appear here.
    // super_admin extras: "global" = global only, "private" = private only.
    let mut themes: Vec<ThemeInfo> = if ctx.is_global_admin {
        match filter {
            "global"  => themes.into_iter().filter(|t| t.source == "global").collect(),
            "private" => themes.into_iter().filter(|t| t.source == "private").collect(),
            _ => themes.into_iter().filter(|t| t.source == "site").collect(),
        }
    } else if filter == "global" {
        themes.into_iter().filter(|t| t.source == "global").collect()
    } else {
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
                    // folder name is the key used everywhere (URLs, DB, activations).
                    // display_name is the human label from theme.toml — may differ in casing.
                    let display_name = theme_section.get("name").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
                    let version = theme_section.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let description = theme_section.get("description").and_then(|v| v.as_str()).unwrap_or("No description").to_string();
                    let author = theme_section.get("author").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                    let has_screenshot = path.join("screenshot.png").exists();
                    themes.push(ThemeInfo {
                        name: dir_name.clone(),
                        display_name,
                        version,
                        description,
                        author,
                        active: dir_name == active_theme,
                        has_screenshot,
                        source: source.to_string(),
                        can_delete: false,         // computed after scanning in render_appearance_list
                        in_use_by: 0,              // computed after scanning in render_appearance_list
                        has_site_copy: false,      // computed below for global/private filter view
                        is_private_origin: source == "private", // also set for site copies below
                        has_global_copy: false,    // computed below for private filter view
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
