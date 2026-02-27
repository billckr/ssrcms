//! Admin handlers for form submissions.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::form_submission;

use admin::pages::forms::{FormSummaryRow, SubmissionRow};

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

// ── list all forms ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct FormsFilter {
    pub filter: Option<String>,
}

pub async fn list_forms(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<FormsFilter>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    match form_submission::list_forms(&state.db, site_id).await {
        Ok(summaries) => {
            let blocked = form_submission::blocked_names(&state.db, site_id).await;
            let mut rows: Vec<FormSummaryRow> = summaries.into_iter().map(|s| {
                let is_blocked = blocked.contains(&s.form_name);
                FormSummaryRow {
                    form_name: s.form_name,
                    submission_count: s.submission_count,
                    last_submitted_at: s.last_submitted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                    unread_count: s.unread_count,
                    blocked: is_blocked,
                }
            }).collect();

            // Collect distinct names for the dropdown (before filtering).
            let all_names: Vec<String> = rows.iter().map(|r| r.form_name.clone()).collect();

            // Apply filter if set and non-empty.
            let active_filter = params.filter.as_deref().unwrap_or("").trim().to_string();
            if !active_filter.is_empty() {
                rows.retain(|r| r.form_name == active_filter);
            }

            Html(admin::pages::forms::render_forms_list(
                &rows, &all_names, &active_filter, None, &ctx,
            )).into_response()
        }
        Err(e) => {
            tracing::error!("list_forms error: {:?}", e);
            Html(admin::pages::forms::render_forms_list(
                &[], &[], "", Some("Failed to load forms."), &ctx,
            )).into_response()
        }
    }
}

// ── view a single form's submissions ─────────────────────────────────────────

pub async fn view_form(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = require_forms_cap(&admin) { return r; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(r) => return r };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Mark as read in the background (fire-and-forget; errors are non-fatal)
    let _ = form_submission::mark_all_read(&state.db, site_id, &name).await;

    match form_submission::list_submissions(&state.db, site_id, &name, 500, 0).await {
        Ok(subs) => {
            // Derive column set from all JSONB keys in natural insertion order
            let columns = collect_columns(&subs.iter().map(|s| &s.data).collect::<Vec<_>>());

            let rows: Vec<SubmissionRow> = subs.into_iter().map(|s| SubmissionRow {
                id: s.id.to_string(),
                data: s.data,
                ip_address: s.ip_address,
                read_at: s.read_at.map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string()),
                submitted_at: s.submitted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            }).collect();

            Html(admin::pages::forms::render_form_detail(&name, &rows, &columns, None, &ctx)).into_response()
        }
        Err(e) => {
            tracing::error!("view_form '{}' error: {:?}", name, e);
            Html(admin::pages::forms::render_form_detail(&name, &[], &[], Some("Failed to load submissions."), &ctx)).into_response()
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
    Redirect::to(&format!("/admin/forms/{}", name)).into_response()
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
    Redirect::to("/admin/forms").into_response()
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
fn collect_columns(values: &[&serde_json::Value]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut cols = Vec::new();
    for v in values {
        if let serde_json::Value::Object(map) = v {
            for key in map.keys() {
                if seen.insert(key.clone()) {
                    cols.push(key.clone());
                }
            }
        }
    }
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
    Redirect::to("/admin/forms").into_response()
}
