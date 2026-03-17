//! Visual page builder handlers.
//!
//! Serves the Puck editor shell and provides JSON API endpoints for saving,
//! loading, activating, and deleting page compositions.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::page_composition;

// ── Editor shell ──────────────────────────────────────────────────────────────

/// GET /admin/builder
/// Lists all compositions for the current site.
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

    let comps = page_composition::list_by_site(&state.db, site_id)
        .await
        .unwrap_or_default();

    let rows: Vec<admin::pages::builder::CompositionRow> = comps
        .iter()
        .map(|c| admin::pages::builder::CompositionRow {
            id: c.id.to_string(),
            name: c.name.clone(),
            is_homepage: c.is_homepage,
            updated_at: c.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    Html(admin::pages::builder::render_list(&rows, &ctx)).into_response()
}

/// GET /admin/builder/new
/// Serves the Puck editor shell for a new composition.
pub async fn new_editor(
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

    Html(admin::pages::builder::render_editor(
        None,
        "",
        site_id,
        &ctx,
    ))
    .into_response()
}

/// GET /admin/builder/edit/:id
/// Serves the Puck editor shell for an existing composition.
pub async fn edit_editor(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let comp = match page_composition::get_by_id(&state.db, id).await {
        Ok(Some(c)) if c.site_id == site_id => c,
        _ => return Redirect::to("/admin/builder").into_response(),
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    Html(admin::pages::builder::render_editor(
        Some(id),
        &comp.name,
        site_id,
        &ctx,
    ))
    .into_response()
}

// ── JSON API ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveRequest {
    pub composition_id: Option<String>,
    pub site_id: String,
    pub name: String,
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct SaveResponse {
    pub id: String,
}

/// POST /admin/builder/save
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

    let existing_id = body
        .composition_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());

    let name = if body.name.trim().is_empty() {
        "Untitled".to_string()
    } else {
        body.name.trim().to_string()
    };

    match page_composition::upsert(
        &state.db,
        existing_id,
        site_id,
        &name,
        body.data,
        Some(admin.user.id),
    )
    .await
    {
        Ok(comp) => Json(SaveResponse { id: comp.id.to_string() }).into_response(),
        Err(e) => {
            tracing::error!("builder save error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Save failed").into_response()
        }
    }
}

/// GET /admin/builder/load/:id
pub async fn load(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return (StatusCode::FORBIDDEN, "No site").into_response();
    };

    match page_composition::get_by_id(&state.db, id).await {
        Ok(Some(comp)) if comp.site_id == site_id => {
            Json(comp.composition).into_response()
        }
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("builder load error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// POST /admin/builder/activate/:id
pub async fn activate(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }

    if let Err(e) = page_composition::activate_homepage(&state.db, id, site_id).await {
        tracing::error!("builder activate error: {e}");
    }

    Redirect::to("/admin/builder").into_response()
}

/// POST /admin/builder/deactivate
pub async fn deactivate(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }

    if let Err(e) = page_composition::deactivate_homepage(&state.db, site_id).await {
        tracing::error!("builder deactivate error: {e}");
    }

    Redirect::to("/admin/builder").into_response()
}

/// POST /admin/builder/delete/:id
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin/builder").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin/builder").into_response();
    }

    if let Err(e) = page_composition::delete(&state.db, id, site_id).await {
        tracing::error!("builder delete error: {e}");
    }

    Redirect::to("/admin/builder").into_response()
}
