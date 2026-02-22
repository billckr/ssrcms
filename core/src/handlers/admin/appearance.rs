use axum::{
    body::Body,
    extract::{Multipart, Path, State, Form},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path as FsPath, PathBuf};

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
    _admin: AdminUser,
) -> Html<String> {
    render_appearance_list(&state, None).await
}

// ── Activate ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ActivateForm {
    pub theme: String,
}

pub async fn activate(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<ActivateForm>,
) -> impl IntoResponse {
    let themes_dir = &state.config.themes_dir;
    let theme_path = FsPath::new(themes_dir).join(&form.theme);

    if !theme_path.is_dir() {
        tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
        return render_appearance_list(&state, Some("Theme not found.")).await.into_response();
    }

    if let Err(e) = set_site_setting(&state.db, "active_theme", &form.theme).await {
        tracing::error!("failed to save active_theme to DB: {:?}", e);
        return render_appearance_list(&state, Some("Failed to activate theme. Please try again.")).await.into_response();
    }

    if let Err(e) = state.templates.switch_theme(&form.theme) {
        tracing::error!("failed to switch theme to '{}': {:?}", form.theme, e);
        return render_appearance_list(&state, Some("Theme files could not be loaded. Please try again.")).await.into_response();
    }

    *state.active_theme.write().unwrap() = form.theme.clone();

    Redirect::to("/admin/appearance").into_response()
}

// ── Screenshot ─────────────────────────────────────────────────────────────────

pub async fn screenshot(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(theme_name): Path<String>,
) -> Response {
    let themes_dir = FsPath::new(&state.config.themes_dir);

    // Path traversal guard: canonicalize the target and verify it stays inside themes_dir.
    let candidate = themes_dir.join(&theme_name).join("screenshot.png");
    let canonical_themes = match themes_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let canonical_candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if !canonical_candidate.starts_with(&canonical_themes) {
        tracing::warn!("screenshot path traversal attempt: theme_name={:?}", theme_name);
        return StatusCode::NOT_FOUND.into_response();
    }

    match fs::read(&canonical_candidate) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── Zip upload ─────────────────────────────────────────────────────────────────

pub async fn upload_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Collect the zip bytes from the multipart field named "file".
    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            match field.bytes().await {
                Ok(b) if b.len() <= MAX_ZIP_BYTES => zip_bytes = Some(b.to_vec()),
                Ok(_) => {
                    return render_appearance_list(&state, Some("Upload too large. Maximum size is 50 MB."))
                        .await
                        .into_response();
                }
                Err(e) => {
                    tracing::error!("failed to read theme zip field: {:?}", e);
                    return render_appearance_list(&state, Some("Failed to read uploaded file. Please try again."))
                        .await
                        .into_response();
                }
            }
        }
    }

    let zip_bytes = match zip_bytes {
        Some(b) => b,
        None => return render_appearance_list(&state, Some("No file received.")).await.into_response(),
    };

    let themes_dir = state.config.themes_dir.clone();

    // Run zip extraction on a blocking thread (zip crate is synchronous).
    let result = tokio::task::spawn_blocking(move || {
        extract_and_install_theme(&zip_bytes, &themes_dir)
    })
    .await;

    match result {
        Ok(Ok(theme_name)) => {
            tracing::info!("theme '{}' installed successfully", theme_name);

            // Always reload the active theme in Tera after a successful upload.
            // If the uploaded theme IS the active theme this picks up the new
            // files immediately. If not, it is a harmless reload of the
            // already-loaded theme. This avoids any name-comparison edge cases.
            let active = state.active_theme.read().unwrap().clone();
            if let Err(e) = state.templates.switch_theme(&active) {
                tracing::error!("theme '{}' installed but Tera reload of '{}' failed: {:?}", theme_name, active, e);
                return render_appearance_list(&state, Some("Theme installed but could not be reloaded. Please restart the server."))
                    .await
                    .into_response();
            }
            tracing::info!("reloaded active theme '{}' after installing '{}'", active, theme_name);

            render_appearance_list(&state, Some(&format!("Theme '{}' installed successfully.", theme_name)))
                .await
                .into_response()
        }
        Ok(Err(msg)) => {
            tracing::warn!("theme upload rejected: {}", msg);
            render_appearance_list(&state, Some(&msg)).await.into_response()
        }
        Err(e) => {
            tracing::error!("theme upload task panicked: {:?}", e);
            render_appearance_list(&state, Some("Installation failed. Please try again."))
                .await
                .into_response()
        }
    }
}

/// Extract a theme zip into a temp directory, validate structure, then move it to themes_dir.
/// Returns the theme name on success, or a user-facing error string on failure.
fn extract_and_install_theme(zip_bytes: &[u8], themes_dir: &str) -> Result<String, String> {
    use std::io::Cursor;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "File does not appear to be a valid zip archive.".to_string())?;

    // Detect top-level directory prefix (many zips wrap content in a folder).
    // We look for the entry that contains theme.toml to find the prefix.
    let prefix = find_theme_prefix(&mut archive)?;

    // Extract into a temp directory.
    let tmp_dir = tempdir_in(themes_dir)?;
    let tmp_path = tmp_dir.clone();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let raw_name = entry.name().to_string();

        // Strip the detected prefix to get the relative path within the theme.
        let relative = if prefix.is_empty() {
            raw_name.clone()
        } else {
            match raw_name.strip_prefix(&prefix) {
                Some(r) => r.to_string(),
                None => continue, // skip the prefix dir entry itself
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

    // Move temp dir to themes/<name>, replacing any existing theme of the same name.
    let final_path = PathBuf::from(themes_dir).join(&theme_name);
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
fn tempdir_in(themes_dir: &str) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let tmp_name = format!(".theme_upload_tmp_{}", ts);
    let tmp_path = PathBuf::from(themes_dir).join(&tmp_name);
    fs::create_dir_all(&tmp_path)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    tmp_path.to_str()
        .map(|s| s.to_string())
        .ok_or("Temp path is not valid UTF-8.".to_string())
}

// ── Shared list renderer ───────────────────────────────────────────────────────

async fn render_appearance_list(state: &AppState, flash: Option<&str>) -> Html<String> {
    let themes_dir = &state.config.themes_dir;

    let active_theme_from_db: String = sqlx::query_scalar("SELECT value FROM site_settings WHERE key = 'active_theme'")
        .fetch_optional(&state.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to read active_theme from DB: {:?}", e);
            None
        })
        .unwrap_or_else(|| "default".to_string());

    let mut themes = Vec::new();

    if let Ok(entries) = fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Skip hidden temp dirs created during upload.
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
                            active: name == active_theme_from_db,
                            has_screenshot,
                        });
                    }
                }
            }
        }
    }

    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Html(render_with_flash(&themes, flash))
}
