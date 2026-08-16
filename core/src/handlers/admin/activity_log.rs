//! GET /admin/activity-log — read-only view over the audit_log table.

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::activity_log::{humanize_action, role_display, ActivityLogRow};

const PER_PAGE: i64 = 50;

#[derive(Deserialize)]
pub struct ActivityLogQuery {
    pub page: Option<i64>,
    pub site: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    pub partial: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<ActivityLogQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PER_PAGE;
    let search = query.search.as_deref().unwrap_or("").trim();
    let sort = query.sort.as_deref().unwrap_or("");
    let dir = query.dir.as_deref().unwrap_or("");

    // Global admins see everything by default, or one site when ?site= is
    // set. Site admins are always scoped to every site they belong to —
    // they may own more than one.
    let selected_site_id = if admin.caps.is_global_admin {
        query.site.as_deref().and_then(|s| s.parse::<Uuid>().ok())
    } else {
        None
    };

    let scope: Option<Vec<Uuid>> = if admin.caps.is_global_admin {
        selected_site_id.map(|sid| vec![sid])
    } else {
        Some(
            crate::models::site_user::list_for_user(&state.db, admin.user.id)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|(site, _)| site.id)
                .collect(),
        )
    };
    let scope_slice = scope.as_deref();

    let (entries, total) = tokio::join!(
        crate::models::audit_log::list_filtered(&state.db, scope_slice, search, sort, dir, PER_PAGE, offset),
        crate::models::audit_log::count_filtered(&state.db, scope_slice, search),
    );
    let entries = entries.unwrap_or_default();
    let total = total.unwrap_or(0);

    // Resolve site hostnames for display — one query for every site that
    // appears in this page of results, not one per row.
    let all_sites = crate::models::site::list(&state.db).await.unwrap_or_default();
    let hostnames: HashMap<Uuid, String> = all_sites.iter().map(|s| (s.id, s.hostname.clone())).collect();

    let rows: Vec<ActivityLogRow> = entries.iter().map(|e| ActivityLogRow {
        created_at: e.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        actor_label: format!("{} ({})", e.actor_email, role_display(&e.actor_role)),
        action_label: humanize_action(&e.action),
        target_type: e.target_type.clone(),
        target_label: e.target_label.clone(),
        site_label: e.site_id.and_then(|sid| hostnames.get(&sid)).cloned().unwrap_or_else(|| "—".to_string()),
    }).collect();

    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);

    let site_options: Vec<(String, String)> = if admin.caps.is_global_admin {
        all_sites.iter().map(|s| (s.id.to_string(), s.hostname.clone())).collect()
    } else {
        vec![]
    };
    let selected_site_str = selected_site_id.map(|s| s.to_string()).unwrap_or_default();

    if query.partial.is_some() {
        return Html(admin::pages::activity_log::list_fragment(
            &rows,
            page,
            total_pages,
            &selected_site_str,
            search,
            sort,
            dir,
        )).into_response();
    }

    Html(admin::pages::activity_log::render_list(
        &rows,
        page,
        total_pages,
        &site_options,
        &selected_site_str,
        search,
        sort,
        dir,
        None,
        &ctx,
    )).into_response()
}
