//! Admin handlers for viewing a poll's tallied results and raw vote log,
//! plus CSV export and a reset action. Sibling to `forms.rs`'s submissions
//! viewer.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::{poll_def, poll_vote};

use admin::pages::poll_results::{render_results, ResultOption, VoteRow};

const VOTES_PER_PAGE: i64 = 50;

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

#[derive(Deserialize)]
pub struct ResultsQuery {
    pub page: Option<i64>,
}

pub async fn view(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>, Query(q): Query<ResultsQuery>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Ok(Some(poll)) = poll_def::get_by_id(&state.db, site_id, id).await else {
        return Redirect::to("/admin/designer?tab=polls").into_response();
    };

    let tally = poll_vote::tally(&state.db, id).await.unwrap_or_default();
    let total_votes: i64 = tally.iter().map(|(_, c)| c).sum();
    let options: Vec<ResultOption> = poll.options.iter().map(|o| {
        let votes = tally.iter().find(|(k, _)| k == &o.key).map(|(_, c)| *c).unwrap_or(0);
        let percent = if total_votes > 0 { ((votes as f64 / total_votes as f64) * 100.0).round() as u32 } else { 0 };
        ResultOption { label: o.label.clone(), votes, percent }
    }).collect();

    let page = q.page.unwrap_or(1).max(1);
    let count = poll_vote::count_for_poll(&state.db, site_id, id).await.unwrap_or(0);
    let total_pages = ((count + VOTES_PER_PAGE - 1) / VOTES_PER_PAGE).max(1);
    let offset = (page - 1) * VOTES_PER_PAGE;
    let raw_votes = poll_vote::list_votes(&state.db, site_id, id, VOTES_PER_PAGE, offset).await.unwrap_or_default();
    let vote_rows: Vec<VoteRow> = raw_votes.iter().map(|v| {
        let option_label = poll.options.iter().find(|o| o.key == v.option_key).map(|o| o.label.clone()).unwrap_or_else(|| v.option_key.clone());
        VoteRow {
            id: v.id.to_string(),
            option_label,
            ip_address: v.ip_address.clone().unwrap_or_default(),
            voted_at: v.voted_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        }
    }).collect();

    Html(render_results(&poll.id.to_string(), &poll.name, &options, total_votes, &vote_rows, page, total_pages, &ctx, None)).into_response()
}

pub async fn export_csv(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let Ok(Some(poll)) = poll_def::get_by_id(&state.db, site_id, id).await else {
        return (StatusCode::NOT_FOUND, "Poll not found.").into_response();
    };
    let votes = poll_vote::list_all_votes(&state.db, site_id, id).await.unwrap_or_default();

    let mut csv = String::from("option,ip_address,voted_at\n");
    for v in &votes {
        let option_label = poll.options.iter().find(|o| o.key == v.option_key).map(|o| o.label.clone()).unwrap_or_else(|| v.option_key.clone());
        csv.push_str(&format!(
            "{},{},{}\n",
            csv_escape(&option_label),
            csv_escape(v.ip_address.as_deref().unwrap_or("")),
            v.voted_at.to_rfc3339(),
        ));
    }

    (
        [
            (header::CONTENT_TYPE, "text/csv".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}-votes.csv\"", poll.slug)),
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

pub async fn reset(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    if let Err(e) = poll_vote::delete_all(&state.db, site_id, id).await {
        tracing::error!("poll_results::reset failed: {e}");
    }

    Redirect::to(&format!("/admin/designer/polls/{id}/results")).into_response()
}
