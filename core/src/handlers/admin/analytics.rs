//! GET /admin/analytics — tabbed overview (General, Forms). Forms tab
//! replaces the old standalone /admin/form-data-analytics list page; its
//! per-form submission/detail routes are untouched.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::{form_def, form_submission, mail_log};
use admin::pages::forms::FormSummaryRow;

fn require_forms_cap(admin: &AdminUser) -> Result<(), Response> {
    if !admin.caps.can_manage_forms {
        Err((StatusCode::FORBIDDEN, "Forbidden").into_response())
    } else {
        Ok(())
    }
}

fn require_site_id(admin: &AdminUser) -> Result<Uuid, Response> {
    admin.site_id.ok_or_else(|| (StatusCode::BAD_REQUEST, "No site selected.").into_response())
}

#[derive(Deserialize, Default)]
pub struct AnalyticsQuery {
    #[serde(default)]
    pub tab: String,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub dir: String,
    /// A form definition's UUID — when present, the Forms tab shows only
    /// that one form instead of the full site list. Set by the "Analytics"
    /// link in the Form Designer editor, so clicking it lands on this one
    /// form rather than the whole table.
    #[serde(default)]
    pub form: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<AnalyticsQuery>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let tab = if q.tab == "forms" { "forms" } else { "general" };

    // Only the Forms tab needs this data — skip the queries entirely on General.
    let (rows, flash): (Vec<FormSummaryRow>, Option<&str>) = if tab != "forms" {
        (vec![], None)
    } else {
        match form_submission::list_forms(&state.db, site_id).await {
            Ok(summaries) => {
                let blocked = form_submission::blocked_names(&state.db, site_id).await;
                // Submissions and definitions are linked only by slug (no FK) —
                // a form deleted in Form Designer leaves its collected data
                // behind. Flag those rows (definition_exists = false, id =
                // None) rather than let them look identical to a form that's
                // still editable/re-embeddable — Edit/Analytics/Delete all
                // need a real form id, so they only render when one exists.
                let defined: std::collections::HashMap<String, uuid::Uuid> = form_def::list_for_site(&state.db, site_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|f| (f.slug, f.id))
                    .collect();
                let mut rows: Vec<FormSummaryRow> = summaries.into_iter().map(|s| {
                    let is_blocked = blocked.contains(&s.form_name);
                    let id = defined.get(&s.form_name).map(|id| id.to_string());
                    FormSummaryRow {
                        form_name: s.form_name,
                        submission_count: s.submission_count,
                        last_submitted_at: s.last_submitted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                        unread_count: s.unread_count,
                        blocked: is_blocked,
                        definition_exists: id.is_some(),
                        id,
                    }
                }).collect();
                if let Some(form_id) = q.form.as_deref() {
                    rows.retain(|r| r.id.as_deref() == Some(form_id));
                }
                (rows, None)
            }
            Err(e) => {
                tracing::error!("analytics forms tab: list_forms error: {:?}", e);
                (vec![], Some("Failed to load forms."))
            }
        }
    };

    let form_filter = q.form.as_deref().map(|id| {
        let name = rows.iter().find(|r| r.id.as_deref() == Some(id)).map(|r| r.form_name.as_str()).unwrap_or(id);
        (id, name)
    });

    Html(admin::pages::analytics::render(tab, &rows, &q.sort, &q.dir, flash, form_filter, &ctx)).into_response()
}

// ── per-form mail delivery log — GET /admin/analytics/form/{id} (moved from
// the old /admin/form-analytics/{id}, itself part of Form Designer) ────────

pub async fn form_detail(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Ok(Some(form)) = form_def::get_by_id(&state.db, site_id, id).await else {
        return Redirect::to("/admin/analytics?tab=forms").into_response();
    };

    let tab = match params.get("tab").map(|s| s.as_str()) {
        Some("results") => "results",
        Some("submissions") => "submissions",
        _ => "stats",
    };

    // Only the Submissions tab needs this — skip the queries otherwise.
    let submissions = if tab == "submissions" {
        const PER_PAGE: i64 = 20;
        let _ = form_submission::mark_all_read(&state.db, site_id, &form.slug).await;
        let total: i64 = form_submission::count_for_form(&state.db, site_id, &form.slug).await.unwrap_or(0);
        let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
        let page = params.get("page").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).clamp(1, total_pages);
        let offset = (page - 1) * PER_PAGE;

        let all_data = form_submission::list_all_data_for_form(&state.db, site_id, &form.slug).await.unwrap_or_default();
        let columns = super::forms::collect_columns(&all_data.iter().collect::<Vec<_>>());

        let rows: Vec<admin::pages::forms::SubmissionRow> = form_submission::list_submissions(&state.db, site_id, &form.slug, PER_PAGE, offset)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| admin::pages::forms::SubmissionRow {
                id: s.id.to_string(),
                data: s.data,
                ip_address: s.ip_address,
                read_at: s.read_at.map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string()),
                submitted_at: s.submitted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            })
            .collect();

        Some(admin::pages::analytics::SubmissionsTabData { rows, columns, page, total_pages })
    } else {
        None
    };

    let (total_sent, succeeded, failed) = mail_log::counts_for_form(&state.db, id).await.unwrap_or((0, 0, 0));
    let recent = mail_log::list_for_form(&state.db, id, 50).await.unwrap_or_default();

    let mut recent: Vec<admin::pages::analytics::MailLogRow> = recent.into_iter().map(|r| admin::pages::analytics::MailLogRow {
        to_email: r.to_email,
        subject: r.subject,
        success: r.success,
        mailgun_message_id: r.mailgun_message_id,
        error: r.error,
        sent_at: r.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    }).collect();

    let search = params.get("search").map(|s| s.trim()).unwrap_or("");
    if !search.is_empty() {
        let needle = search.to_lowercase();
        recent.retain(|r| {
            r.to_email.to_lowercase().contains(&needle)
                || r.subject.to_lowercase().contains(&needle)
                || r.mailgun_message_id.as_deref().unwrap_or("").to_lowercase().contains(&needle)
                || r.error.as_deref().unwrap_or("").to_lowercase().contains(&needle)
        });
    }

    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("");
    let dir = params.get("dir").map(|s| s.as_str()).unwrap_or("");
    match sort {
        "to"      => recent.sort_by(|a, b| a.to_email.to_lowercase().cmp(&b.to_email.to_lowercase())),
        "subject" => recent.sort_by(|a, b| a.subject.to_lowercase().cmp(&b.subject.to_lowercase())),
        "status"  => recent.sort_by(|a, b| a.success.cmp(&b.success)),
        "sent"    => recent.sort_by(|a, b| a.sent_at.cmp(&b.sent_at)),
        _ => {}
    }
    // Sorts above are ascending by default; reverse for dir=desc. No sort
    // param at all leaves the list in its DB order (newest-first).
    if !sort.is_empty() && dir == "desc" {
        recent.reverse();
    }

    let data = admin::pages::analytics::FormAnalyticsData {
        id: id.to_string(),
        form_name: form.name,
        form_slug: form.slug,
        total_submissions: form.total_submissions,
        total_sent,
        succeeded,
        failed,
        recent,
        submissions,
    };

    if params.contains_key("partial") {
        let search_qs = if search.is_empty() { String::new() } else { format!("&search={}", admin::html_escape(search)) };
        return Html(admin::pages::analytics::render_analytics_table(&data, sort, dir, &search_qs)).into_response();
    }

    Html(admin::pages::analytics::render_analytics(&data, tab, sort, dir, search, &ctx)).into_response()
}
