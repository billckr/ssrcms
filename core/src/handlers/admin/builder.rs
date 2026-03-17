//! Visual page builder admin handlers.
//!
//! Workflow:
//!   1. GET  /admin/appearance/builder          → list compositions
//!   2. GET  /admin/appearance/builder/new      → show "name your theme" form
//!   3. POST /admin/appearance/builder/new      → create theme dir + DB record, redirect to editor
//!   4. GET  /admin/appearance/builder/{id}/edit → builder canvas
//!   5. POST /admin/appearance/builder/{id}/save → update composition JSON (fetch API)
//!   6. POST /admin/appearance/builder/preview   → render without saving (fetch API → iframe)
//!   7. POST /admin/appearance/builder/{id}/delete → delete DB record
//!
//! Activation/deactivation is handled by the existing Appearance system — the
//! composition is tied to the theme directory that was created during step 3.

use std::path::{Path as FsPath, PathBuf};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form, Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::page_composition::{self, CompositionJson};
use crate::templates::composer;
use crate::templates::context::{ContextBuilder, RequestContext, SessionContext};
use admin::pages::builder::{BuilderEditorData, CompositionRow};

// ── List ─────────────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let compositions = page_composition::list(&state.db, site_id)
        .await
        .unwrap_or_default();

    let active_theme = state.active_theme_for_site(Some(site_id));

    let rows: Vec<CompositionRow> = compositions
        .iter()
        .map(|c| {
            let is_active = c.theme_name.as_deref()
                .map(|tn| tn == active_theme)
                .unwrap_or(false);
            CompositionRow {
                id: c.id.to_string(),
                name: c.name.clone(),
                layout: c.layout.clone(),
                theme_name: c.theme_name.clone().unwrap_or_default(),
                is_active,
                updated_at: c.updated_at.format("%b %-d, %Y").to_string(),
            }
        })
        .collect();

    let flash = q.get("error").map(|s| s.as_str());
    Html(admin::pages::builder::render_list(&rows, flash, &ctx)).into_response()
}

// ── New — name form ───────────────────────────────────────────────────────────

pub async fn new_form(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::builder::render_new_theme_form(None, &ctx)).into_response()
}

// ── New — create theme + composition ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct NewThemeForm {
    pub name: String,
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<NewThemeForm>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! form_err {
        ($msg:expr) => {
            return Html(admin::pages::builder::render_new_theme_form(Some($msg), &ctx)).into_response()
        };
    }

    // Validate name
    let name = form.name.trim().to_string();
    if name.is_empty() { form_err!("Theme name is required."); }
    if name.len() > 64  { form_err!("Theme name must be 64 characters or less."); }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.starts_with('.') {
        form_err!("Theme name must not contain slashes, backslashes, '..', or start with a dot.");
    }

    // Folder name: slugified version of the human name
    let folder_name = slug::slugify(&name);
    if folder_name.is_empty() { form_err!("Theme name must contain at least one alphanumeric character."); }

    // Target dir: sites/{site_id}/themes/{folder_name}/
    let target_dir: PathBuf = FsPath::new(&state.config.sites_dir)
        .join(site_id.to_string())
        .join("themes")
        .join(&folder_name);

    if target_dir.exists() {
        form_err!("A theme with that name already exists. Choose a different name.");
    }

    // Copy the global default theme as the base (standalone theme files only —
    // blocks/ and layouts/ are NOT part of the global theme).
    let global_default = FsPath::new(&state.config.themes_dir).join("global").join("default");
    if !global_default.is_dir() {
        form_err!("The global 'default' theme was not found. Cannot create theme.");
    }

    if let Err(e) = super::appearance::copy_dir_all(&global_default, &target_dir) {
        tracing::error!("builder create_theme: copy failed for '{}': {}", folder_name, e);
        let _ = std::fs::remove_dir_all(&target_dir);
        form_err!("Failed to copy theme files. Please try again.");
    }

    // Copy builder-specific block and layout templates from themes/builder/.
    // These are separate from the global theme so themes are never polluted.
    let builder_dir = FsPath::new(&state.config.themes_dir).join("builder");
    let tpl_dest = target_dir.join("templates");
    for sub in &["blocks", "layouts"] {
        let src = builder_dir.join(sub);
        let dst = tpl_dest.join(sub);
        if src.is_dir() {
            if let Err(e) = super::appearance::copy_dir_all(&src, &dst) {
                tracing::warn!("builder create_theme: could not copy {} templates: {}", sub, e);
            }
        }
    }

    // Write a theme.toml with the user-supplied name so it shows up correctly in My Themes
    let toml_content = format!(
        "[theme]\nname = \"{display_name}\"\nversion = \"1.0.0\"\ndescription = \"Visual composer theme\"\nauthor = \"\"\napi_version = \"1\"\n\n[nav_locations]\nprimary = \"Primary Navigation\"\nfooter  = \"Footer Links\"\n",
        display_name = name.replace('"', "\\\""),
    );
    if let Err(e) = std::fs::write(target_dir.join("theme.toml"), toml_content.as_bytes()) {
        tracing::error!("builder create_theme: theme.toml write failed: {}", e);
        let _ = std::fs::remove_dir_all(&target_dir);
        form_err!("Failed to write theme files. Please try again.");
    }

    // Create the composition DB record
    let slug = slug::slugify(&name);
    let comp = match page_composition::create(
        &state.db,
        site_id,
        &name,
        &slug,
        "single-column",
        &folder_name,
        json!({ "zones": {} }),
        Some(admin.user.id),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("builder create composition DB error: {:?}", e);
            let _ = std::fs::remove_dir_all(&target_dir);
            form_err!("Failed to save composition. Please try again.");
        }
    };

    Redirect::to(&format!("/admin/appearance/builder/{}/edit?created=1", comp.id)).into_response()
}

// ── Edit ──────────────────────────────────────────────────────────────────────

pub async fn edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let comp = match page_composition::get(&state.db, id, site_id).await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/admin/appearance/builder").into_response(),
    };

    let composition_json = {
        let raw = serde_json::to_string(&comp.composition)
            .unwrap_or_else(|_| r#"{"zones":{}}"#.to_string());
        raw.replace("</", r"<\/")
    };

    let flash = if q.get("created").map(|v| v == "1").unwrap_or(false) {
        Some("Theme created. Build your layout, then activate it from Appearance.")
    } else if q.get("saved").map(|v| v == "1").unwrap_or(false) {
        Some("Composition saved.")
    } else {
        None
    };

    let active_theme = state.active_theme_for_site(Some(site_id));
    let is_active = comp.theme_name.as_deref().map(|tn| tn == active_theme).unwrap_or(false);

    let data = BuilderEditorData {
        id: comp.id.to_string(),
        name: comp.name.clone(),
        layout: comp.layout.clone(),
        theme_name: comp.theme_name.clone().unwrap_or_default(),
        composition_json,
        is_active,
    };

    Html(admin::pages::builder::render_editor(&data, flash, &ctx)).into_response()
}

// ── Save (JSON API) ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SavePayload {
    pub name: String,
    pub layout: String,
    pub composition: CompositionJson,
}

pub async fn save(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<SavePayload>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, Json(json!({ "ok": false, "error": "No site" }))).into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Json(json!({ "ok": false, "error": "Forbidden" }))).into_response();
    }

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": "Name is required" }))).into_response();
    }

    if !["single-column", "left-sidebar", "right-sidebar"].contains(&payload.layout.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": "Invalid layout" }))).into_response();
    }

    let sanitized = sanitize_composition(payload.composition);
    let comp_value = match serde_json::to_value(&sanitized) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e.to_string() }))).into_response(),
    };

    match page_composition::update(&state.db, id, site_id, &name, &payload.layout, comp_value).await {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!("builder save error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": "Save failed" }))).into_response()
        }
    }
}

// ── Preview (JSON API) ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PreviewPayload {
    pub layout: String,
    pub composition: CompositionJson,
    #[serde(default)]
    pub theme_name: Option<String>,
}

pub async fn preview(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(payload): Json<PreviewPayload>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, Html("Forbidden".to_string())).into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, Html("Forbidden".to_string())).into_response();
    }

    if !["single-column", "left-sidebar", "right-sidebar"].contains(&payload.layout.as_str()) {
        return (StatusCode::BAD_REQUEST, Html("Invalid layout".to_string())).into_response();
    }

    let sanitized = sanitize_composition(payload.composition);
    let comp_value = match serde_json::to_value(&sanitized) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Html(e.to_string())).into_response(),
    };

    // Use the composition's theme for preview; fall back to active theme
    let theme = payload.theme_name
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| state.active_theme_for_site(Some(site_id)));

    let comp = crate::models::page_composition::PageComposition {
        id: Uuid::nil(),
        site_id,
        name: "Preview".to_string(),
        slug: "preview".to_string(),
        layout: payload.layout,
        composition: comp_value,
        theme_name: Some(theme.clone()),
        created_by: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let base_url = state.get_site_by_id(site_id)
        .map(|(_, s)| s.base_url.clone())
        .unwrap_or_else(|| state.settings.base_url.clone());

    let site_ctx = match crate::handlers::home::build_site_context(&state, Some(site_id), &base_url).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("builder preview: site context error: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Context error".to_string())).into_response();
        }
    };

    let nav = crate::models::nav_menu::build_nav_context(&state.db, site_id, "/").await;
    let base_ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}/", base_url),
            path: "/".to_string(),
            query: std::collections::HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav,
    }
    .into_tera_context();

    match composer::render_composition(&comp, &state.templates, Some(site_id), &theme, &base_ctx) {
        Ok(html) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        ).into_response(),
        Err(e) => {
            tracing::warn!("builder preview render error: {:?}", e);
            Html(format!("<p style='color:red;padding:1rem'>Preview error: {}</p>", e)).into_response()
        }
    }
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN.into_response()).into_response();
    }

    // Fetch the record first so we know the theme folder and can guard against deleting the active theme.
    let comp = match page_composition::get(&state.db, id, site_id).await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/admin/appearance/builder").into_response(),
    };

    // Block deletion while the composition's theme is the currently active theme.
    let active_theme = state.active_theme_for_site(Some(site_id));
    if comp.theme_name.as_deref() == Some(active_theme.as_str()) {
        return Redirect::to(
            "/admin/appearance/builder?error=Cannot+delete+the+active+theme.+Activate+a+different+theme+first."
        ).into_response();
    }

    let theme_dir: Option<PathBuf> = comp.theme_name.map(|tn| {
        FsPath::new(&state.config.sites_dir)
            .join(site_id.to_string())
            .join("themes")
            .join(tn)
    });

    if let Err(e) = page_composition::delete(&state.db, id, site_id).await {
        tracing::error!("delete composition error: {:?}", e);
    }

    // Remove the theme directory from disk after the DB record is gone.
    if let Some(dir) = theme_dir {
        if dir.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!("builder delete: could not remove theme dir {:?}: {}", dir, e);
            } else {
                tracing::info!("builder delete: removed theme dir {:?}", dir);
            }
        }
    }

    Redirect::to("/admin/appearance/builder").into_response()
}

// ── Sanitization ──────────────────────────────────────────────────────────────

fn sanitize_block_html(html: &str) -> String {
    ammonia::Builder::default().clean(html).to_string()
}

fn sanitize_composition(mut comp: CompositionJson) -> CompositionJson {
    for blocks in comp.zones.values_mut() {
        for block in blocks.iter_mut() {
            if block.block_type == "text-block" {
                if let Some(content) = block.config.get("content") {
                    if let Some(s) = content.as_str() {
                        let clean = sanitize_block_html(s);
                        block.config.insert("content".to_string(), serde_json::Value::String(clean));
                    }
                }
            }
        }
    }
    comp
}
