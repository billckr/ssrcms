//! GET /admin/activity-log — read-only view over the audit_log table, plus
//! CSV export and clear-all.

use axum::{
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::activity_log::{humanize_action, role_display, ActivityLogRow};

const PER_PAGE: i64 = 50;

/// Resolves the admin's own scope (`None` for a global admin) and the
/// validated `?site=` selection (only ever narrowing within that scope — a
/// site admin can't type another site's uuid into the query string to
/// escape it), then combines them into the final scope every query below
/// filters by: `Some(&[selected])` if one site is picked, else the admin's
/// own scope.
async fn resolve_scope(state: &AppState, admin: &AdminUser, site: Option<&str>) -> (Option<Vec<Uuid>>, Option<Uuid>, Option<Vec<Uuid>>) {
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
    let selected_site_id = site.and_then(|s| s.parse::<Uuid>().ok())
        .filter(|sid| admin_scope.as_ref().is_none_or(|scope| scope.contains(sid)));
    let scope = match selected_site_id {
        Some(sid) => Some(vec![sid]),
        None => admin_scope.clone(),
    };
    (admin_scope, selected_site_id, scope)
}

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

    let (admin_scope, selected_site_id, scope) = resolve_scope(&state, &admin, query.site.as_deref()).await;
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

/// GET /admin/activity-log/export — CSV dump of every entry within the
/// caller's scope (see `resolve_scope`; a site admin only ever gets their
/// own sites' rows). Ignores the search box — this is a full backup of
/// what's in scope, not "export my current filter" — but does honor a
/// `?site=` selection so a super admin can export just one site's history.
pub async fn export_csv(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<ActivityLogQuery>,
) -> Response {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let (_, _, scope) = resolve_scope(&state, &admin, query.site.as_deref()).await;
    let all_sites = crate::models::site::list(&state.db).await.unwrap_or_default();
    let hostnames: HashMap<Uuid, String> = all_sites.iter().map(|s| (s.id, s.hostname.clone())).collect();

    let entries = crate::models::audit_log::list_for_export(&state.db, scope.as_deref())
        .await
        .unwrap_or_default();

    let mut csv = String::from("when,who,role,action,target_type,target,site\n");
    for e in &entries {
        let site_label = e.site_id.and_then(|sid| hostnames.get(&sid)).cloned().unwrap_or_default();
        csv.push_str(&csv_escape(&e.created_at.format("%Y-%m-%d %H:%M UTC").to_string()));
        csv.push(',');
        csv.push_str(&csv_escape(&e.actor_email));
        csv.push(',');
        csv.push_str(&csv_escape(role_display(&e.actor_role)));
        csv.push(',');
        csv.push_str(&csv_escape(&humanize_action(&e.action)));
        csv.push(',');
        csv.push_str(&csv_escape(&e.target_type));
        csv.push(',');
        csv.push_str(&csv_escape(&e.target_label));
        csv.push(',');
        csv.push_str(&csv_escape(&site_label));
        csv.push('\n');
    }

    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"activity-log.csv\"".to_string()),
        ],
        csv,
    ).into_response()
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// POST /admin/activity-log/clear — deletes every entry within the caller's
/// scope (see `resolve_scope`; a site admin can only ever clear their own
/// sites' rows, and honors a `?site=` selection the same way the list view
/// and export do, so a super admin can clear just one site's history).
pub async fn clear(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<ActivityLogQuery>,
) -> Response {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let (_, _, scope) = resolve_scope(&state, &admin, query.site.as_deref()).await;
    let cleared = match crate::models::audit_log::delete_scoped(&state.db, scope.as_deref()).await {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("activity_log::clear failed: {e}");
            0
        }
    };

    // Recorded after the delete, so the log isn't left silently empty —
    // this becomes the sole surviving entry describing who cleared it.
    super::audit(&state, &admin, "activity_log.cleared", "audit_log", None, &format!("{cleared} entries"), admin.site_id).await;

    let redirect = match query.site.as_deref() {
        Some(site) => format!("/admin/activity-log?site={site}"),
        None => "/admin/activity-log".to_string(),
    };
    Redirect::to(&redirect).into_response()
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
