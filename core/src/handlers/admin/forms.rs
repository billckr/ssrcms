//! Admin handlers for form submissions.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::{form_def, form_submission};

use admin::pages::forms::SubmissionRow;

// ── helpers ───────────────────────────────────────────────────────────────────

fn require_forms_cap(admin: &AdminUser) -> Result<(), Response> {
    if !admin.caps.can_manage_forms {
        Err((StatusCode::FORBIDDEN, "Forbidden").into_response())
    } else {
        Ok(())
    }
}

fn require_site_id(admin: &AdminUser) -> Result<uuid::Uuid, Response> {
    admin.site_id.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "No site selected.").into_response()
    })
}

// ── view a single form's submissions ─────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct ViewFormQuery {
    pub page: Option<i64>,
}

const SUBMISSIONS_PER_PAGE: i64 = 20;

pub async fn view_form(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
    Query(q): Query<ViewFormQuery>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Submissions now live on the Submissions tab of the form's own
    // Analytics page — redirect there whenever a form definition still
    // exists for this slug. Only an orphaned row (definition deleted, so
    // there's no /admin/analytics/form/{id} to send it to) still renders
    // here directly.
    if let Ok(Some(form)) = form_def::get_by_slug(&state.db, site_id, &name).await {
        let page_qs = q.page.map(|p| format!("&page={p}")).unwrap_or_default();
        return Redirect::to(&format!("/admin/analytics/form/{}?tab=submissions{page_qs}", form.id)).into_response();
    }

    // Mark as read in the background (fire-and-forget; errors are non-fatal)
    let _ = form_submission::mark_all_read(&state.db, site_id, &name).await;

    let total: i64 = form_submission::count_for_form(&state.db, site_id, &name).await.unwrap_or(0);
    let total_pages = ((total + SUBMISSIONS_PER_PAGE - 1) / SUBMISSIONS_PER_PAGE).max(1);
    let page = q.page.unwrap_or(1).clamp(1, total_pages);
    let offset = (page - 1) * SUBMISSIONS_PER_PAGE;

    // Column set is derived from every submission ever made to this form,
    // not just the current page — otherwise a field only present on an
    // older page would silently disappear from the displayed rows.
    let all_data = form_submission::list_all_data_for_form(&state.db, site_id, &name).await.unwrap_or_default();
    let columns = collect_columns(&all_data.iter().collect::<Vec<_>>());

    match form_submission::list_submissions(&state.db, site_id, &name, SUBMISSIONS_PER_PAGE, offset).await {
        Ok(subs) => {
            let rows: Vec<SubmissionRow> = subs.into_iter().map(|s| SubmissionRow {
                id: s.id.to_string(),
                data: s.data,
                ip_address: s.ip_address,
                read_at: s.read_at.map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string()),
                submitted_at: s.submitted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            }).collect();

            Html(admin::pages::forms::render_form_detail(&name, &rows, &columns, page, total_pages, None, &ctx)).into_response()
        }
        Err(e) => {
            tracing::error!("view_form '{}' error: {:?}", name, e);
            Html(admin::pages::forms::render_form_detail(&name, &[], &[], 1, 1, Some("Failed to load submissions."), &ctx)).into_response()
        }
    }
}

// ── delete a single submission ────────────────────────────────────────────────

pub async fn delete_submission(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((name, id)): Path<(String, uuid::Uuid)>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    if let Err(e) = form_submission::delete(&state.db, site_id, id).await {
        tracing::error!("delete_submission error: {:?}", e);
    }
    Redirect::to(&submissions_return_url(&state, site_id, &name).await).into_response()
}

// ── delete all submissions for a form ────────────────────────────────────────

pub async fn delete_all(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    if let Err(e) = form_submission::delete_all(&state.db, site_id, &name).await {
        tracing::error!("delete_all '{}' error: {:?}", name, e);
    }
    Redirect::to(&submissions_return_url(&state, site_id, &name).await).into_response()
}

/// Where to send the admin back to after a submissions-list action (single
/// delete, delete-all): the form's Submissions tab if a definition still
/// exists for this slug, else the old standalone page (orphaned data).
async fn submissions_return_url(state: &AppState, site_id: uuid::Uuid, name: &str) -> String {
    match form_def::get_by_slug(&state.db, site_id, name).await {
        Ok(Some(form)) => format!("/admin/analytics/form/{}?tab=submissions", form.id),
        _ => format!("/admin/form-data-analytics/{name}"),
    }
}

// ── export CSV ────────────────────────────────────────────────────────────────

pub async fn export_csv(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    match form_submission::list_submissions(&state.db, site_id, &name, 10_000, 0).await {
        Err(e) => {
            tracing::error!("export_csv '{}' error: {:?}", name, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Export failed").into_response()
        }
        Ok(subs) => {
            let columns = collect_columns(&subs.iter().map(|s| &s.data).collect::<Vec<_>>());

            let mut csv = String::new();

            // Header row
            for (i, col) in columns.iter().enumerate() {
                if i > 0 { csv.push(','); }
                csv.push_str(&csv_escape(col));
            }
            csv.push_str(",submitted_at,ip_address\n");

            // Data rows
            for s in &subs {
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 { csv.push(','); }
                    let val = s.data.get(col)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    csv.push_str(&csv_escape(val));
                }
                let ts = s.submitted_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let ip = s.ip_address.as_deref().unwrap_or("");
                csv.push(',');
                csv.push_str(&csv_escape(&ts));
                csv.push(',');
                csv.push_str(&csv_escape(ip));
                csv.push('\n');
            }

            let filename = format!("form-{}.csv", name);
            (
                [
                    (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                    (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
                ],
                csv,
            ).into_response()
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build an ordered, deduplicated column list from a set of JSONB objects.
/// Prioritizes: name, email, subject, message; then all others.
pub(super) fn collect_columns(values: &[&serde_json::Value]) -> Vec<String> {
    let mut all_keys = std::collections::HashSet::new();

    // Collect all unique keys from all values
    for v in values {
        if let serde_json::Value::Object(map) = v {
            for key in map.keys() {
                all_keys.insert(key.clone());
            }
        }
    }

    // Define priority order
    let priority = ["name", "email", "subject", "message"];
    let mut cols = Vec::new();

    // Add priority columns first (if they exist)
    for p in &priority {
        if all_keys.contains(*p) {
            cols.push(p.to_string());
            all_keys.remove(*p);
        }
    }

    // Add remaining columns (sorted for consistency)
    let mut remaining: Vec<_> = all_keys.into_iter().collect();
    remaining.sort();
    cols.extend(remaining);

    cols
}

/// RFC 4180 CSV field escaping.
fn csv_escape(s: &str) -> String {
    if s.contains('"') || s.contains(',') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ── block / unblock a form ────────────────────────────────────────────────────

pub async fn toggle_block(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    if form_submission::is_blocked(&state.db, site_id, &name).await {
        let _ = form_submission::unblock(&state.db, site_id, &name).await;
    } else {
        let _ = form_submission::block(&state.db, site_id, &name).await;
    }
    Redirect::to("/admin/analytics?tab=forms").into_response()
}
