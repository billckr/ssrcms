//! Admin plugin management handlers.
//! Mirrors the appearance.rs pattern: install, upload, activate, deactivate, delete.

use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use axum::extract::Form;
use serde::Deserialize;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path as FsPath, PathBuf};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::site_plugin;
use crate::plugins::manifest::PluginManifest;
use admin::pages::plugins::{PluginCard, render_with_flash};

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct PluginsQuery {
    #[serde(default)]
    pub filter: Option<String>,
}

// ── List ──────────────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<PluginsQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let filter = q.filter.as_deref().unwrap_or("my");
    render_plugins_list(&state, None, &ctx, admin.site_id, filter, admin.caps.is_global_admin)
        .await
        .into_response()
}

// ── Install (copy global → site) ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InstallPluginForm {
    pub plugin_name: String,
}

pub async fn install(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<InstallPluginForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! err {
        ($msg:expr) => {
            return render_plugins_list(&state, Some($msg), &ctx, admin.site_id, "global", admin.caps.is_global_admin)
                .await
                .into_response()
        };
    }

    // Validate plugin name.
    if form.plugin_name.contains("..") || form.plugin_name.contains('/') || form.plugin_name.contains('\\') || form.plugin_name.is_empty() {
        err!("Invalid plugin name.");
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => err!("No site selected. Run 'synap-cli site init' first."),
    };

    let plugins_dir = &state.config.plugins_dir;
    let global_src = FsPath::new(plugins_dir).join("global").join(&form.plugin_name);

    if !global_src.is_dir() {
        err!("Plugin not found in global library.");
    }

    // Path traversal guard.
    let canonical_global = match FsPath::new(plugins_dir).join("global").canonicalize() {
        Ok(p) => p,
        Err(_) => err!("Plugin directory not found."),
    };
    let canonical_src = match global_src.canonicalize() {
        Ok(p) => p,
        Err(_) => err!("Plugin not found."),
    };
    if !canonical_src.starts_with(&canonical_global) {
        tracing::warn!("install plugin path traversal attempt: {:?}", form.plugin_name);
        err!("Invalid plugin name.");
    }

    let site_dest = FsPath::new(plugins_dir)
        .join("sites")
        .join(site_id.to_string())
        .join(&form.plugin_name);

    // Only copy if not already present.
    if !site_dest.exists() {
        let src = canonical_src.clone();
        let dst = site_dest.clone();
        match tokio::task::spawn_blocking(move || copy_dir_all(&src, &dst)).await {
            Ok(Ok(())) => tracing::info!("installed plugin '{}' to site {}", form.plugin_name, site_id),
            Ok(Err(e)) => {
                tracing::error!("install plugin: copy failed: {}", e);
                err!("Failed to copy plugin files. Please try again.");
            }
            Err(e) => {
                tracing::error!("install plugin: task panicked: {:?}", e);
                err!("Failed to install plugin. Please try again.");
            }
        }
    }

    // Record in site_plugins table (idempotent).
    if let Err(e) = site_plugin::install(&state.db, site_id, &form.plugin_name).await {
        tracing::error!("install plugin: DB insert failed: {:?}", e);
        err!("Plugin copied but could not be recorded. Please try again.");
    }

    Redirect::to("/admin/plugins?filter=my").into_response()
}

// ── Upload (zip → site) ───────────────────────────────────────────────────────

pub async fn upload(
    State(state): State<AppState>,
    admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let max_bytes = (state.config.max_upload_mb as usize)
        .saturating_mul(1024 * 1024)
        .max(25 * 1024 * 1024);

    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            match field.bytes().await {
                Ok(b) if b.len() <= max_bytes => zip_bytes = Some(b.to_vec()),
                Ok(_) => {
                    let msg = format!("Upload too large. Maximum is {} MB.", state.config.max_upload_mb);
                    return render_plugins_list(&state, Some(&msg), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                        .await
                        .into_response();
                }
                Err(e) => {
                    tracing::error!("plugin upload: read field error: {:?}", e);
                    return render_plugins_list(&state, Some("Failed to read uploaded file."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                        .await
                        .into_response();
                }
            }
        }
    }

    let zip_bytes = match zip_bytes {
        Some(b) => b,
        None => return render_plugins_list(&state, Some("No file received."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response(),
    };

    let site_id = match admin.site_id {
        Some(id) => id,
        None => return render_plugins_list(&state, Some("No site selected."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response(),
    };

    let plugins_dir = state.config.plugins_dir.clone();
    let target_dir = format!("{}/sites/{}", plugins_dir, site_id);

    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        tracing::error!("plugin upload: create dir failed: {}", e);
        return render_plugins_list(&state, Some("Failed to prepare plugin directory."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response();
    }

    let result = tokio::task::spawn_blocking(move || {
        extract_and_install_plugin(&zip_bytes, &target_dir)
    })
    .await;

    match result {
        Ok(Ok(plugin_name)) => {
            // Record in DB (idempotent).
            if let Err(e) = site_plugin::install(&state.db, site_id, &plugin_name).await {
                tracing::error!("plugin upload: DB insert failed: {:?}", e);
                return render_plugins_list(&state, Some("Plugin installed but could not be recorded."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                    .await
                    .into_response();
            }

            // Register plugin templates in the Tera engine.
            let plugin_dir = FsPath::new(&state.config.plugins_dir)
                .join("sites")
                .join(site_id.to_string())
                .join(&plugin_name);
            register_plugin_templates(&plugin_dir, &state.templates);

            tracing::info!("plugin '{}' uploaded and installed for site {}", plugin_name, site_id);
            render_plugins_list(&state, Some(&format!("Plugin '{}' installed.", plugin_name)), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                .await
                .into_response()
        }
        Ok(Err(msg)) => {
            tracing::warn!("plugin upload rejected: {}", msg);
            render_plugins_list(&state, Some(&msg), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                .await
                .into_response()
        }
        Err(e) => {
            tracing::error!("plugin upload task panicked: {:?}", e);
            render_plugins_list(&state, Some("Installation failed. Please try again."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                .await
                .into_response()
        }
    }
}

// ── Activate ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PluginNameForm {
    pub plugin_name: String,
}

pub async fn activate(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PluginNameForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let site_id = match admin.site_id {
        Some(id) => id,
        None => return render_plugins_list(&state, Some("No site selected."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response(),
    };

    if let Err(e) = site_plugin::activate(&state.db, site_id, &form.plugin_name).await {
        tracing::error!("activate plugin '{}': {:?}", form.plugin_name, e);
        return render_plugins_list(&state, Some("Failed to activate plugin."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response();
    }

    Redirect::to("/admin/plugins?filter=my").into_response()
}

// ── Deactivate ────────────────────────────────────────────────────────────────

pub async fn deactivate(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PluginNameForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let site_id = match admin.site_id {
        Some(id) => id,
        None => return render_plugins_list(&state, Some("No site selected."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response(),
    };

    if let Err(e) = site_plugin::deactivate(&state.db, site_id, &form.plugin_name).await {
        tracing::error!("deactivate plugin '{}': {:?}", form.plugin_name, e);
        return render_plugins_list(&state, Some("Failed to deactivate plugin."), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
            .await
            .into_response();
    }

    Redirect::to("/admin/plugins?filter=my").into_response()
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PluginNameForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_plugins {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! err {
        ($msg:expr) => {
            return render_plugins_list(&state, Some($msg), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
                .await
                .into_response()
        };
    }

    if form.plugin_name.contains("..") || form.plugin_name.contains('/') || form.plugin_name.contains('\\') || form.plugin_name.is_empty() {
        err!("Invalid plugin name.");
    }

    let site_id = match admin.site_id {
        Some(id) => id,
        None => err!("No site selected."),
    };

    // Guard: cannot delete an active plugin.
    let is_active = site_plugin::is_active(&state.db, site_id, &form.plugin_name)
        .await
        .unwrap_or(false);
    if is_active {
        err!("Cannot delete an active plugin. Deactivate it first.");
    }

    // Path traversal guard.
    let site_plugins_dir = FsPath::new(&state.config.plugins_dir)
        .join("sites")
        .join(site_id.to_string());
    let plugin_path = site_plugins_dir.join(&form.plugin_name);

    let canonical_parent = match site_plugins_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => err!("Plugin directory not found."),
    };
    let canonical_plugin = match plugin_path.canonicalize() {
        Ok(p) => p,
        Err(_) => err!("Plugin not found."),
    };
    if canonical_plugin.parent() != Some(canonical_parent.as_path()) {
        tracing::warn!("delete plugin path traversal: {:?}", form.plugin_name);
        err!("Invalid plugin name.");
    }

    // Remove directory.
    if let Err(e) = fs::remove_dir_all(&canonical_plugin) {
        tracing::error!("delete plugin '{}': {:?}", form.plugin_name, e);
        err!("Failed to delete plugin files.");
    }

    // Remove DB record.
    if let Err(e) = site_plugin::delete(&state.db, site_id, &form.plugin_name).await {
        tracing::error!("delete plugin '{}': DB error: {:?}", form.plugin_name, e);
    }

    tracing::info!("plugin '{}' deleted from site {}", form.plugin_name, site_id);
    render_plugins_list(&state, Some(&format!("Plugin '{}' deleted.", form.plugin_name)), &ctx, admin.site_id, "my", admin.caps.is_global_admin)
        .await
        .into_response()
}

// ── Shared render helper ──────────────────────────────────────────────────────

async fn render_plugins_list(
    state: &AppState,
    flash: Option<&str>,
    ctx: &admin::PageContext,
    site_id: Option<uuid::Uuid>,
    filter: &str,
    is_global_admin: bool,
) -> impl IntoResponse {
    let plugins_dir = &state.config.plugins_dir;

    match filter {
        "global" => {
            // Show plugins available in plugins/global/, marking which are installed for this site.
            let installed: Vec<String> = if let Some(sid) = site_id {
                site_plugin::list_for_site(&state.db, sid)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.plugin_name)
                    .collect()
            } else {
                Vec::new()
            };

            let mut cards = Vec::new();
            let global_dir = FsPath::new(plugins_dir).join("global");
            if let Ok(entries) = fs::read_dir(&global_dir) {
                let mut names: Vec<_> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .collect();
                names.sort_by_key(|e| e.file_name());
                for entry in names {
                    let plugin_dir = entry.path();
                    let manifest_path = plugin_dir.join("plugin.toml");
                    if let Ok(manifest) = PluginManifest::from_file(&manifest_path) {
                        let name = manifest.plugin.name.clone();
                        let is_installed = installed.contains(&name);
                        cards.push(PluginCard {
                            name: name.clone(),
                            version: manifest.plugin.version.clone(),
                            plugin_type: manifest.plugin.plugin_type.clone(),
                            author: manifest.plugin.author.clone(),
                            description: manifest.plugin.description.clone(),
                            source: "global".to_string(),
                            is_active: false,
                            is_installed,
                            hooks: manifest.hooks.keys().cloned().collect(),
                        });
                    }
                }
            }
            Html(render_with_flash(&cards, flash, ctx, "global", is_global_admin)).into_response()
        }
        _ => {
            // "my" — show plugins installed for this site with active state.
            let installed_records: Vec<crate::models::site_plugin::SitePlugin> =
                if let Some(sid) = site_id {
                    site_plugin::list_for_site(&state.db, sid)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

            let mut cards = Vec::new();
            if let Some(sid) = site_id {
                let site_dir = FsPath::new(plugins_dir)
                    .join("sites")
                    .join(sid.to_string());
                if let Ok(entries) = fs::read_dir(&site_dir) {
                    let mut names: Vec<_> = entries
                        .flatten()
                        .filter(|e| e.path().is_dir())
                        .collect();
                    names.sort_by_key(|e| e.file_name());
                    for entry in names {
                        let plugin_dir = entry.path();
                        let manifest_path = plugin_dir.join("plugin.toml");
                        if let Ok(manifest) = PluginManifest::from_file(&manifest_path) {
                            let name = manifest.plugin.name.clone();
                            let is_active = installed_records
                                .iter()
                                .any(|r| r.plugin_name == name && r.active);
                            let mut hook_names: Vec<String> = manifest.hooks.keys().cloned().collect();
                            hook_names.sort();
                            cards.push(PluginCard {
                                name: name.clone(),
                                version: manifest.plugin.version.clone(),
                                plugin_type: manifest.plugin.plugin_type.clone(),
                                author: manifest.plugin.author.clone(),
                                description: manifest.plugin.description.clone(),
                                source: "site".to_string(),
                                is_active,
                                is_installed: true,
                                hooks: hook_names,
                            });
                        }
                    }
                }
            }
            Html(render_with_flash(&cards, flash, ctx, "my", is_global_admin)).into_response()
        }
    }
}

// ── Plugin template registration helper ──────────────────────────────────────

fn register_plugin_templates(plugin_dir: &FsPath, templates: &crate::templates::TemplateEngine) {
    // Note: the Rust `glob` crate does not support brace expansion ({html,xml}),
    // so we run two separate glob passes.
    for ext in &["html", "xml"] {
        let glob_pattern = format!("{}/**/*.{}", plugin_dir.display(), ext);
        if let Ok(paths) = glob::glob(&glob_pattern) {
            for path in paths.flatten() {
                let rel = match path.strip_prefix(plugin_dir) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let template_name = rel.to_string_lossy().replace('\\', "/");
                let source = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if let Err(e) = templates.add_raw_template(&template_name, &source) {
                    tracing::warn!("could not register plugin template '{}': {}", template_name, e);
                }
            }
        }
    }
}

// ── Zip extraction ────────────────────────────────────────────────────────────

/// Extract a plugin zip into target_dir.
/// Validates that plugin.toml exists and has a valid [plugin] name and type.
/// Returns the plugin name on success.
fn extract_and_install_plugin(zip_bytes: &[u8], target_dir: &str) -> Result<String, String> {
    use std::io::Cursor;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "File does not appear to be a valid zip archive.".to_string())?;

    let prefix = find_plugin_prefix(&mut archive)?;

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
        if relative.is_empty() { continue; }
        if relative.contains("..") || relative.starts_with('/') || relative.starts_with('\\') {
            return Err("Zip contains invalid paths.".to_string());
        }
        let dest = PathBuf::from(&tmp_path).join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| format!("Failed to create dir: {}", e))?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| format!("Failed to read entry: {}", e))?;
            fs::write(&dest, &buf).map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    // Validate plugin.toml.
    let toml_path = PathBuf::from(&tmp_path).join("plugin.toml");
    let toml_content = fs::read_to_string(&toml_path)
        .map_err(|_| "plugin.toml not found. Is this a valid Synaptic Signals plugin?".to_string())?;
    let parsed: toml::Table = toml::from_str(&toml_content)
        .map_err(|_| "plugin.toml is not valid TOML.".to_string())?;

    let plugin_name = parsed
        .get("plugin")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .ok_or("plugin.toml is missing [plugin] name field.".to_string())?
        .to_string();

    if plugin_name.is_empty() || plugin_name.contains('/') || plugin_name.contains('\\') || plugin_name.contains("..") {
        return Err("plugin.toml contains an invalid plugin name.".to_string());
    }

    // Validate plugin type if present.
    if let Some(plugin_type) = parsed
        .get("plugin")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("type"))
        .and_then(|v| v.as_str())
    {
        match plugin_type {
            "tera" => {} // OK
            "wasm" => {
                // WASM plugins must include a .wasm file.
                let has_wasm = std::fs::read_dir(&tmp_path)
                    .map(|entries| {
                        entries
                            .flatten()
                            .any(|e| e.path().extension().map(|x| x == "wasm").unwrap_or(false))
                    })
                    .unwrap_or(false);
                if !has_wasm {
                    let _ = fs::remove_dir_all(&tmp_path);
                    return Err("WASM plugin must include a .wasm file.".to_string());
                }
            }
            other => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(format!("Unknown plugin type '{}'. Expected 'tera' or 'wasm'.", other));
            }
        }
    }

    // Move to final location.
    let final_path = PathBuf::from(target_dir).join(&plugin_name);
    if final_path.exists() {
        fs::remove_dir_all(&final_path).map_err(|e| format!("Failed to replace existing plugin: {}", e))?;
    }
    fs::rename(&tmp_path, &final_path).map_err(|e| format!("Failed to install plugin: {}", e))?;

    Ok(plugin_name)
}

fn find_plugin_prefix(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Result<String, String> {
    let mut nested: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| format!("Failed to read zip: {}", e))?;
        let name = entry.name().to_string();
        if name == "plugin.toml" {
            return Ok(String::new());
        }
        if name.ends_with("/plugin.toml") && name.matches('/').count() == 1 && nested.is_none() {
            let prefix = name[..name.len() - "plugin.toml".len()].to_string();
            nested = Some(prefix);
        }
    }
    nested.ok_or("plugin.toml not found in zip. Is this a valid Synaptic Signals plugin?".to_string())
}

fn tempdir_in(dir: &str) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let tmp_name = format!(".plugin_upload_tmp_{}", ts);
    let tmp_path = PathBuf::from(dir).join(&tmp_name);
    fs::create_dir_all(&tmp_path).map_err(|e| format!("Failed to create temp directory: {}", e))?;
    tmp_path.to_str()
        .map(|s| s.to_string())
        .ok_or("Temp path is not valid UTF-8.".to_string())
}

/// Recursively copy a directory. Mirrors the pattern from appearance.rs.
pub fn copy_dir_all(src: &FsPath, dst: &FsPath) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}
