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

    // Resolve site hostnames/hierarchy up front — needed for both the site
    // filter dropdown and the per-row site label.
    let all_sites = crate::models::site::list(&state.db).await.unwrap_or_default();
    let hostnames: HashMap<Uuid, String> = all_sites.iter().map(|s| (s.id, s.hostname.clone())).collect();

    // A site admin's allowed set is every site they hold any site_users row
    // on (they may control more than one, e.g. a site plus its child sites
    // they created — see Site::parent_site_id). Never widened by ?site=
    // below; it only ever narrows within this set.
    let admin_scope: Option<Vec<Uuid>> = if admin.caps.is_global_admin {
        None
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

    // ?site= narrows to one site — but only if it's actually a site this
    // admin is allowed to see (a site_admin can't type another site's uuid
    // into the query string to escape their scope).
    let selected_site_id = query.site.as_deref()
        .and_then(|s| s.parse::<Uuid>().ok())
        .filter(|sid| admin_scope.as_ref().is_none_or(|scope| scope.contains(sid)));

    let scope: Option<Vec<Uuid>> = match selected_site_id {
        Some(sid) => Some(vec![sid]),
        None => admin_scope.clone(),
    };
    let scope_slice = scope.as_deref();

    let (entries, total) = tokio::join!(
        crate::models::audit_log::list_filtered(&state.db, scope_slice, search, sort, dir, PER_PAGE, offset),
        crate::models::audit_log::count_filtered(&state.db, scope_slice, search),
    );
    let entries = entries.unwrap_or_default();
    let total = total.unwrap_or(0);

    let rows: Vec<ActivityLogRow> = entries.iter().map(|e| ActivityLogRow {
        created_at: e.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        actor_label: format!("{} ({})", e.actor_email, role_display(&e.actor_role)),
        action_label: humanize_action(&e.action),
        target_type: e.target_type.clone(),
        target_label: e.target_label.clone(),
        site_label: e.site_id.and_then(|sid| hostnames.get(&sid)).cloned().unwrap_or_else(|| "—".to_string()),
    }).collect();

    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);

    // Build the filter dropdown's options, grouped so a child site is listed
    // right under its parent (indented) rather than alphabetically
    // interleaved with unrelated sites — only sites in admin_scope are
    // offered to a site admin.
    let site_options = build_site_options(&all_sites, admin_scope.as_deref());
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

/// Builds the site-filter dropdown's options, grouped by hierarchy: each
/// top-level site (`parent_site_id: None`) is followed immediately by its
/// own child sites, indented — rather than interleaved alphabetically with
/// unrelated sites. When `allowed` is `Some`, only those site ids are
/// included (a site admin's scope); `None` includes every site (global admin).
fn build_site_options(all_sites: &[crate::models::site::Site], allowed: Option<&[Uuid]>) -> Vec<(String, String)> {
    let visible: Vec<&crate::models::site::Site> = all_sites.iter()
        .filter(|s| allowed.is_none_or(|ids| ids.contains(&s.id)))
        .collect();

    let mut top_level: Vec<&crate::models::site::Site> = visible.iter()
        .copied()
        .filter(|s| s.parent_site_id.is_none())
        .collect();
    top_level.sort_by(|a, b| a.hostname.cmp(&b.hostname));

    let mut options = Vec::with_capacity(visible.len());
    for parent in &top_level {
        options.push((parent.id.to_string(), parent.hostname.clone()));
        let mut children: Vec<&crate::models::site::Site> = visible.iter()
            .copied()
            .filter(|s| s.parent_site_id == Some(parent.id))
            .collect();
        children.sort_by(|a, b| a.hostname.cmp(&b.hostname));
        for child in children {
            options.push((child.id.to_string(), format!("\u{2003}\u{2514} {}", child.hostname)));
        }
    }

    // Defensive: a child whose parent isn't in `visible` (e.g. a site admin
    // was removed from the parent site but still controls the child they
    // created) — list it top-level rather than dropping it silently.
    let listed: std::collections::HashSet<Uuid> = options.iter()
        .filter_map(|(id, _)| id.parse::<Uuid>().ok())
        .collect();
    let mut orphans: Vec<&crate::models::site::Site> = visible.iter()
        .copied()
        .filter(|s| !listed.contains(&s.id))
        .collect();
    orphans.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    for site in orphans {
        options.push((site.id.to_string(), site.hostname.clone()));
    }

    options
}
