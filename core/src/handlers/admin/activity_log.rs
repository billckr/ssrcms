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

    // Global admins see everything by default, or one site when ?site= is
    // set. Site admins are always scoped to every site they belong to —
    // they may own more than one.
    let selected_site_id = if admin.caps.is_global_admin {
        query.site.as_deref().and_then(|s| s.parse::<Uuid>().ok())
    } else {
        None
    };

    let (entries, total) = if admin.caps.is_global_admin {
        match selected_site_id {
            Some(sid) => (
                crate::models::audit_log::list_for_sites(&state.db, &[sid], PER_PAGE, offset).await.unwrap_or_default(),
                crate::models::audit_log::count_for_sites(&state.db, &[sid]).await.unwrap_or(0),
            ),
            None => (
                crate::models::audit_log::list_all(&state.db, PER_PAGE, offset).await.unwrap_or_default(),
                crate::models::audit_log::count_all(&state.db).await.unwrap_or(0),
            ),
        }
    } else {
        let site_ids: Vec<Uuid> = crate::models::site_user::list_for_user(&state.db, admin.user.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(site, _)| site.id)
            .collect();
        (
            crate::models::audit_log::list_for_sites(&state.db, &site_ids, PER_PAGE, offset).await.unwrap_or_default(),
            crate::models::audit_log::count_for_sites(&state.db, &site_ids).await.unwrap_or(0),
        )
    };

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

    Html(admin::pages::activity_log::render_list(
        &rows,
        page,
        total_pages,
        &site_options,
        &selected_site_str,
        None,
        &ctx,
    )).into_response()
}
