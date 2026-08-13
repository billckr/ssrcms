//! Admin navigation menu handlers.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::nav_menu;
use admin::pages::menus::{MenuEdit, MenuItemRow, MenuRow};

// ── List ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MenusListQuery {
    pub error: Option<String>,
    /// Column to sort by: "name" | "location" | "items".
    #[serde(default)]
    pub sort: String,
    /// Sort direction: "asc" or "desc".
    #[serde(default)]
    pub dir: String,
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<MenusListQuery>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let menus = nav_menu::list_by_site(&state.db, site_id)
        .await
        .unwrap_or_default();

    let mut rows = Vec::with_capacity(menus.len());
    for m in &menus {
        let item_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM nav_menu_items WHERE menu_id = $1",
        )
        .bind(m.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        rows.push(MenuRow {
            id: m.id.to_string(),
            name: m.name.clone(),
            location: m.location.clone(),
            item_count,
        });
    }

    let flash = match q.error.as_deref() {
        Some("duplicate_name") => Some("A menu with that name already exists."),
        _ => None,
    };

    Html(admin::pages::menus::render_list(&rows, &q.sort, &q.dir, &ctx, flash)).into_response()
}

// ── Create ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateMenuForm {
    pub name: String,
    #[serde(default)]
    pub location: String,
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<CreateMenuForm>,
) -> impl IntoResponse {
    let Some(site_id) = admin.site_id else {
        return Redirect::to("/admin").into_response();
    };
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let Some(name) = clean_text(&form.name, 1, 100) else {
        return Redirect::to("/admin/menus").into_response();
    };
    let location = if form.location.is_empty() { None } else { Some(form.location.as_str()) };

    match nav_menu::create(&state.db, site_id, &name, location).await {
        Ok(menu) => {
            Redirect::to(&format!("/admin/menus/{}", menu.id)).into_response()
        }
        Err(e) if is_unique_violation(&e) => {
            Redirect::to("/admin/menus?error=duplicate_name").into_response()
        }
        Err(e) => {
            tracing::error!("create menu error: {:?}", e);
            Redirect::to("/admin/menus").into_response()
        }
    }
}

// ── Edit ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditMenuQuery {
    pub error: Option<String>,
    pub saved: Option<String>,
}

pub async fn edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(q): Query<EditMenuQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let menu = match nav_menu::get_by_id(&state.db, id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    // Site isolation
    if !ctx.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    let items = nav_menu::items_for_menu(&state.db, id).await.unwrap_or_default();
    let pages = load_pages_for_site(&state, admin.site_id).await;

    // Resolve page titles for items
    let item_rows = build_item_rows(&items, &pages);

    let menu_edit = MenuEdit {
        id: menu.id.to_string(),
        name: menu.name.clone(),
        location: menu.location.clone(),
    };

    let flash = match q.error.as_deref() {
        Some("duplicate_name") => Some("A menu with that name already exists."),
        _ => match q.saved.as_deref() {
            Some("menu") => Some("Menu saved."),
            Some("item_added") => Some("Menu item added."),
            Some("item_updated") => Some("Menu item saved."),
            Some("item_deleted") => Some("Menu item deleted."),
            _ => None,
        },
    };

    Html(admin::pages::menus::render_edit(&menu_edit, &item_rows, &pages, &ctx, flash)).into_response()
}

// ── Update ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateMenuForm {
    pub name: String,
    #[serde(default)]
    pub location: String,
}

pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UpdateMenuForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    // Site isolation
    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    let Some(name) = clean_text(&form.name, 1, 100) else {
        return Redirect::to(&format!("/admin/menus/{}", id)).into_response();
    };
    let location = if form.location.is_empty() { None } else { Some(form.location.as_str()) };

    if let Err(e) = nav_menu::update(&state.db, id, &name, location).await {
        if is_unique_violation(&e) {
            return Redirect::to(&format!("/admin/menus/{}?error=duplicate_name", id)).into_response();
        }
        tracing::error!("update menu {} error: {:?}", id, e);
    }

    Redirect::to(&format!("/admin/menus/{}?saved=menu", id)).into_response()
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    if let Err(e) = nav_menu::delete(&state.db, id).await {
        tracing::error!("delete menu {} error: {:?}", id, e);
    }

    Redirect::to("/admin/menus").into_response()
}

// ── Item: Add ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddItemForm {
    pub label: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub page_id: String,
    #[serde(default)]
    pub parent_id: String,
    #[serde(default)]
    pub sort_order: String,
    #[serde(default = "default_target")]
    pub target: String,
}

fn default_target() -> String {
    "_self".to_string()
}

pub async fn add_item(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<AddItemForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    let Some(label) = clean_text(&form.label, 2, 25) else {
        return Redirect::to(&format!("/admin/menus/{}", id)).into_response();
    };
    let page_id: Option<Uuid> = form.page_id.parse().ok();
    let parent_id: Option<Uuid> = form.parent_id.parse().ok();
    let sort_order = clean_sort_order(&form.sort_order);
    let url_clean = clean_url(&form.url);
    let url = url_clean.as_deref();
    let target = clean_target(&form.target);

    if let Err(e) = nav_menu::create_item(
        &state.db, id, parent_id, sort_order,
        &label, url, page_id, target,
    ).await {
        tracing::error!("add nav item error: {:?}", e);
    }

    Redirect::to(&format!("/admin/menus/{}?saved=item_added", id)).into_response()
}

// ── Item: Edit ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditItemForm {
    pub label: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub page_id: String,
    #[serde(default)]
    pub parent_id: String,
    #[serde(default)]
    pub sort_order: String,
    #[serde(default = "default_target")]
    pub target: String,
}

pub async fn edit_item(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((menu_id, item_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<EditItemForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, menu_id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    let Some(label) = clean_text(&form.label, 2, 25) else {
        return Redirect::to(&format!("/admin/menus/{}", menu_id)).into_response();
    };
    let page_id: Option<Uuid> = form.page_id.parse().ok();
    let parent_id: Option<Uuid> = form.parent_id.parse().ok();
    let sort_order = clean_sort_order(&form.sort_order);
    let url_clean = clean_url(&form.url);
    let url = url_clean.as_deref();
    let target = clean_target(&form.target);

    if let Err(e) = nav_menu::update_item(
        &state.db, item_id, parent_id, sort_order,
        &label, url, page_id, target,
    ).await {
        tracing::error!("edit nav item {} error: {:?}", item_id, e);
    }

    Redirect::to(&format!("/admin/menus/{}?saved=item_updated", menu_id)).into_response()
}

// ── Item: Delete ──────────────────────────────────────────────────────────────

pub async fn delete_item(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((menu_id, item_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return Redirect::to("/admin").into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, menu_id).await {
        Ok(m) => m,
        Err(_) => return Redirect::to("/admin/menus").into_response(),
    };

    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return Redirect::to("/admin/menus").into_response();
    }

    if let Err(e) = nav_menu::delete_item(&state.db, item_id).await {
        tracing::error!("delete nav item {} error: {:?}", item_id, e);
    }

    Redirect::to(&format!("/admin/menus/{}?saved=item_deleted", menu_id)).into_response()
}

// ── Item: Reorder (drag-and-drop) ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReorderItemsBody {
    pub order: Vec<Uuid>,
}

/// Reassigns sort_order for one sibling group at a time (the top-level items, or a single
/// parent's children) based on the order submitted by the drag-and-drop UI.
pub async fn reorder_items(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReorderItemsBody>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_appearance {
        return StatusCode::FORBIDDEN.into_response();
    }

    let menu = match nav_menu::get_by_id(&state.db, id).await {
        Ok(m) => m,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    if !admin.caps.is_global_admin && admin.site_id != Some(menu.site_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    if body.order.len() > 500 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    if let Err(e) = nav_menu::reorder_items(&state.db, id, &body.order).await {
        tracing::error!("reorder nav items for menu {} error: {:?}", id, e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::OK.into_response()
}

fn is_unique_violation(err: &crate::errors::AppError) -> bool {
    matches!(
        err,
        crate::errors::AppError::Database(sqlx::Error::Database(db_err)) if db_err.is_unique_violation()
    )
}

// ── Sanitisation ─────────────────────────────────────────────────────────────

/// Trim and cap a text field. Returns `None` when the result is empty.
fn clean_text(s: &str, min_len: usize, max_len: usize) -> Option<String> {
    let s = s.trim();
    if s.chars().count() < min_len { return None; }
    Some(s.chars().take(max_len).collect())
}

/// Whether `rest` (the part of a URL after `http://`/`https://`) starts with
/// a real host containing a TLD, e.g. "example.com" or "example.com/path" —
/// rejects a bare word like "k" with no dot, same standard a person reading
/// the URL would expect.
fn has_valid_host(rest: &str) -> bool {
    let host = rest.split('/').next().unwrap_or("");
    match host.find('.') {
        Some(pos) => pos > 0 && pos < host.len() - 1,
        None => false,
    }
}

/// Validate and normalise a menu item URL.
/// Accepts relative paths (`/…`, root `/` included) and absolute `http(s)://`
/// URLs with a real host + TLD (e.g. `https://example.com`) and `mailto:`
/// URLs with something after the scheme. A bare `"https://"` or a host with
/// no TLD like `"https://k"` is rejected, not silently accepted as a broken
/// link. Rejects `javascript:`, `data:`, and any other scheme.
/// Returns `None` for empty (or invalid) input, meaning "no custom URL".
fn clean_url(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() { return None; }
    let lower = s.to_ascii_lowercase();
    let valid = if lower.starts_with('/') {
        true
    } else if let Some(rest) = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://")) {
        has_valid_host(rest)
    } else if let Some(rest) = lower.strip_prefix("mailto:") {
        !rest.is_empty()
    } else {
        false
    };
    if !valid { return None; }
    Some(s.chars().take(500).collect())
}

/// Clamp sort_order to a sane range.
fn clean_sort_order(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0).clamp(0, 9999)
}

/// Only allow the two valid target values.
fn clean_target(s: &str) -> &str {
    if s == "_blank" { "_blank" } else { "_self" }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn load_pages_for_site(state: &AppState, site_id: Option<Uuid>) -> Vec<(Uuid, String)> {
    crate::models::post::get_published_pages_by_site(&state.db, site_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id, p.title))
        .collect()
}

fn build_item_rows(
    items: &[nav_menu::NavMenuItem],
    pages: &[(Uuid, String)],
) -> Vec<MenuItemRow> {
    let page_map: std::collections::HashMap<Uuid, &str> = pages.iter()
        .map(|(id, title)| (*id, title.as_str()))
        .collect();

    items.iter().map(|i| {
        let page_title = i.page_id
            .and_then(|pid| page_map.get(&pid).copied())
            .map(|s| s.to_string());

        MenuItemRow {
            id: i.id.to_string(),
            menu_id: i.menu_id.to_string(),
            parent_id: i.parent_id.map(|id| id.to_string()),
            sort_order: i.sort_order,
            label: i.label.clone(),
            url: i.url.clone(),
            page_id: i.page_id.map(|id| id.to_string()),
            page_title,
            target: i.target.clone(),
        }
    }).collect()
}
