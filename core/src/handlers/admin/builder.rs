//! Visual page builder handlers.
//!
//! Route structure:
//!   /admin/builder                        — project list for site
//!   /admin/builder/new                    — create project form
//!   /admin/builder/:project_id            — page list within a project
//!   /admin/builder/:project_id/pages/new  — new page editor
//!   /admin/builder/:project_id/pages/:id  — edit page editor
//!   /admin/builder/save                   — JSON API: save page
//!   /admin/builder/load/:id               — JSON API: load page
//!   /admin/builder/:project_id/activate   — set project active
//!   /admin/builder/:project_id/delete     — delete project
//!   /admin/builder/:project_id/pages/:id/set-homepage  — set page as homepage
//!   /admin/builder/:project_id/pages/:id/delete        — delete page

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
    Form,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::{builder_project, page_composition};

// ── Project list ──────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let projects = builder_project::list_by_site(&state.db, site_id)
        .await
        .unwrap_or_default();

    let mut rows: Vec<admin::pages::builder::ProjectRow> = Vec::with_capacity(projects.len());
    for p in &projects {
        let count = builder_project::page_count(&state.db, p.id).await;
        rows.push(admin::pages::builder::ProjectRow {
            id: p.id.to_string(),
            name: p.name.clone(),
            description: p.description.clone(),
            is_active: p.is_active,
            page_count: count,
            updated_at: p.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        });
    }

    Html(admin::pages::builder::render_project_list(&rows, &ctx)).into_response()
}

// ── Create project ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProjectForm {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_project(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<CreateProjectForm>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let name = form.name.trim().to_string();
    if name.is_empty() || name.len() > 35 {
        return Redirect::to("/admin/builder").into_response();
    }

    let description = form.description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .map(|d| d.chars().take(100).collect::<String>());

    match builder_project::create(
        &state.db,
        site_id,
        &name,
        description.as_deref(),
        Some(admin.user.id),
    )
    .await
    {
        Ok(project) => Redirect::to(&format!("/admin/builder/{}", project.id)).into_response(),
        Err(e) => {
            tracing::error!("create project error: {e}");
            Redirect::to("/admin/builder").into_response()
        }
    }
}

// ── Page list within a project ────────────────────────────────────────────────

pub async fn project_pages(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(project_id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let project = match builder_project::get_by_id(&state.db, project_id, site_id).await {
        Ok(Some(p)) => p,
        _ => return Redirect::to("/admin/builder").into_response(),
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let pages = page_composition::list_by_project(&state.db, project_id)
        .await
        .unwrap_or_default();

    let rows: Vec<admin::pages::builder::PageRow> = pages
        .iter()
        .map(|p| admin::pages::builder::PageRow {
            id: p.id.to_string(),
            name: p.name.clone(),
            slug: p.slug.clone(),
            page_type: p.page_type.clone(),
            is_homepage: p.is_homepage,
            updated_at: p.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    Html(admin::pages::builder::render_page_list(
        &admin::pages::builder::ProjectRow {
            id: project.id.to_string(),
            name: project.name.clone(),
            description: project.description.clone(),
            is_active: project.is_active,
            page_count: rows.len() as i64,
            updated_at: project.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        },
        &rows,
        &ctx,
    ))
    .into_response()
}

// ── New page form + create ────────────────────────────────────────────────────

/// GET /admin/builder/:project_id/pages/new — show the new page form.
pub async fn new_page_form(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(project_id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let project = match builder_project::get_by_id(&state.db, project_id, site_id).await {
        Ok(Some(p)) => p,
        _ => return Redirect::to("/admin/builder").into_response(),
    };

    // Check if a homepage already exists for this project
    let has_homepage = page_composition::list_by_project(&state.db, project_id)
        .await
        .unwrap_or_default()
        .iter()
        .any(|p| p.is_homepage);

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    Html(admin::pages::builder::render_new_page_form(
        &admin::pages::builder::ProjectRow {
            id: project.id.to_string(),
            name: project.name.clone(),
            description: project.description.clone(),
            is_active: project.is_active,
            page_count: 0,
            updated_at: String::new(),
        },
        has_homepage,
        &ctx,
    )).into_response()
}

#[derive(Deserialize)]
pub struct CreatePageForm {
    pub name: String,
    pub page_type: String,  // "homepage" | "page"
    pub slug: Option<String>,
}

/// POST /admin/builder/:project_id/pages/new — create the page and open editor.
pub async fn create_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(project_id): Path<Uuid>,
    Form(form): Form<CreatePageForm>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    if builder_project::get_by_id(&state.db, project_id, site_id).await.ok().flatten().is_none() {
        return Redirect::to("/admin/builder").into_response();
    }

    let name = form.name.trim().to_string();
    if name.is_empty() || name.len() > 100 {
        return Redirect::to(&format!("/admin/builder/{project_id}/pages/new")).into_response();
    }

    let is_homepage = form.page_type == "homepage";

    // Normalise slug: strip leading slash, lowercase, only allow safe chars
    let slug = if is_homepage {
        None
    } else {
        let raw = form.slug.as_deref().unwrap_or("").trim().to_lowercase();
        let raw = raw.trim_start_matches('/');
        let clean: String = raw.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let clean = clean.trim_matches('-').to_string();
        if clean.is_empty() {
            return Redirect::to(&format!("/admin/builder/{project_id}/pages/new")).into_response();
        }
        Some(clean)
    };

    // If setting as homepage, clear existing homepage first
    if is_homepage {
        let _ = page_composition::deactivate_homepage(&state.db, project_id).await;
    }

    match page_composition::create(
        &state.db,
        site_id,
        project_id,
        &name,
        &form.page_type,
        slug.as_deref(),
        is_homepage,
        Some(admin.user.id),
    )
    .await
    {
        Ok(page) => Redirect::to(
            &format!("/admin/builder/{project_id}/pages/{}", page.id)
        ).into_response(),
        Err(e) => {
            tracing::error!("create page error: {e}");
            Redirect::to(&format!("/admin/builder/{project_id}/pages/new")).into_response()
        }
    }
}

// ── Editor shell ──────────────────────────────────────────────────────────────

pub async fn edit_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    if builder_project::get_by_id(&state.db, project_id, site_id).await.ok().flatten().is_none() {
        return Redirect::to("/admin/builder").into_response();
    }

    let page = match page_composition::get_by_id(&state.db, page_id).await {
        Ok(Some(p)) if p.project_id == Some(project_id) => p,
        _ => return Redirect::to(&format!("/admin/builder/{project_id}")).into_response(),
    };

    let project = match builder_project::get_by_id(&state.db, project_id, site_id).await {
        Ok(Some(p)) => p,
        _ => return Redirect::to("/admin/builder").into_response(),
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let site_label = ctx.current_site.clone();

    Html(admin::pages::builder::render_editor(
        Some(page_id), &page.name, project_id, site_id, &project.name, &site_label, &ctx,
    )).into_response()
}

// ── JSON API ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveRequest {
    pub composition_id: Option<String>,
    pub project_id: String,
    pub site_id: String,
    pub name: String,
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct SaveResponse {
    pub id: String,
    pub project_id: String,
}

pub async fn save(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<SaveRequest>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "No site").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let project_id = match Uuid::parse_str(&body.project_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid project_id").into_response(),
    };

    // Verify project belongs to this site
    if builder_project::get_by_id(&state.db, project_id, site_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, "Project not found").into_response();
    }

    let page_id = match body.composition_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "Missing composition_id").into_response(),
    };

    let name = if body.name.trim().is_empty() { "Untitled".to_string() } else { body.name.trim().to_string() };

    match page_composition::save_composition(
        &state.db,
        page_id,
        site_id,
        &name,
        body.data,
    )
    .await
    {
        Ok(comp) => Json(SaveResponse {
            id: comp.id.to_string(),
            project_id: project_id.to_string(),
        }).into_response(),
        Err(e) => {
            tracing::error!("builder save error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Save failed").into_response()
        }
    }
}

pub async fn load(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(page_id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "No site").into_response();
    };

    match page_composition::get_by_id(&state.db, page_id).await {
        Ok(Some(comp)) if comp.site_id == site_id => Json(comp.draft_composition).into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("builder load error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// POST /admin/builder/publish — promote draft to live.
pub async fn publish(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<SaveRequest>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "No site").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let project_id = match Uuid::parse_str(&body.project_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid project_id").into_response(),
    };

    if builder_project::get_by_id(&state.db, project_id, site_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, "Project not found").into_response();
    }

    let page_id = match body.composition_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "Missing composition_id").into_response(),
    };

    let name = if body.name.trim().is_empty() { "Untitled".to_string() } else { body.name.trim().to_string() };

    match page_composition::publish_composition(&state.db, page_id, site_id, &name, body.data).await {
        Ok(comp) => Json(SaveResponse {
            id: comp.id.to_string(),
            project_id: project_id.to_string(),
        }).into_response(),
        Err(e) => {
            tracing::error!("builder publish error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Publish failed").into_response()
        }
    }
}

// ── Project actions ───────────────────────────────────────────────────────────

pub async fn activate_project(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(project_id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }
    if let Err(e) = builder_project::activate(&state.db, project_id, site_id).await {
        tracing::error!("activate project error: {e}");
    }
    Redirect::to("/admin/builder").into_response()
}

pub async fn deactivate_project(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }
    if let Err(e) = builder_project::deactivate(&state.db, site_id).await {
        tracing::error!("deactivate project error: {e}");
    }
    Redirect::to("/admin/builder").into_response()
}

pub async fn delete_project(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(project_id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }
    if let Err(e) = builder_project::delete(&state.db, project_id, site_id).await {
        tracing::error!("delete project error: {e}");
    }
    Redirect::to("/admin/builder").into_response()
}

// ── Page actions ──────────────────────────────────────────────────────────────

pub async fn set_homepage(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }
    // Verify project ownership
    if builder_project::get_by_id(&state.db, project_id, site_id).await.ok().flatten().is_none() {
        return Redirect::to("/admin/builder").into_response();
    }
    if let Err(e) = page_composition::activate_homepage(&state.db, page_id, project_id).await {
        tracing::error!("set homepage error: {e}");
    }
    Redirect::to(&format!("/admin/builder/{project_id}")).into_response()
}

pub async fn delete_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }
    if let Err(e) = page_composition::delete(&state.db, page_id, site_id).await {
        tracing::error!("delete page error: {e}");
    }
    Redirect::to(&format!("/admin/builder/{project_id}")).into_response()
}
