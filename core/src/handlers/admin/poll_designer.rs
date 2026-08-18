//! Admin handlers for the Poll Designer — CRUD over saved poll definitions.
//! Mirrors `form_designer.rs`'s structure closely.

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
use crate::models::poll_def::{self, CreatePollDef, PollOption, PollSettings, UpdatePollDef, VoteProtection};

use admin::pages::poll_designer::{polls_list_fragment, render_editor, PollEditData, PollOptionRow, PollRow};

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

// ── list (partial, for the Designer hub's live search) ─────────────────────

pub async fn list(State(state): State<AppState>, admin: AdminUser, Query(params): Query<HashMap<String, String>>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    if !params.contains_key("partial") {
        return Redirect::to("/admin/designer?tab=polls").into_response();
    }

    let polls = poll_def::list_for_site(&state.db, site_id).await.unwrap_or_default();
    let rows: Vec<PollRow> = polls.into_iter().map(|p| PollRow {
        id: p.id.to_string(),
        name: p.name,
        slug: p.slug,
        option_count: p.options.len(),
        total_votes: p.total_votes,
        updated_at: p.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    }).collect();

    let search = params.get("search").map(|s| s.trim()).unwrap_or("");
    Html(polls_list_fragment(&rows, search)).into_response()
}

// ── new / edit ───────────────────────────────────────────────────────────────

pub async fn new_poll(State(state): State<AppState>, admin: AdminUser) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    if let Err(e) = require_site_id(&admin) { return e; }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    Html(render_editor(&PollEditData::default(), &ctx, None)).into_response()
}

pub async fn edit_poll(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Ok(Some(poll)) = poll_def::get_by_id(&state.db, site_id, id).await else {
        return Redirect::to("/admin/designer?tab=polls").into_response();
    };

    let data = PollEditData {
        id: Some(poll.id.to_string()),
        name: poll.name,
        question: poll.question,
        options: poll.options.into_iter().map(|o| PollOptionRow { key: o.key, label: o.label }).collect(),
        success_message: poll.settings.success_message,
        button_label: poll.settings.button_label,
        vote_protection: poll.settings.vote_protection.as_str().to_string(),
    };

    Html(render_editor(&data, &ctx, None)).into_response()
}

// ── create / update / delete ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SavePollForm {
    pub name: String,
    pub question: String,
    pub options_json: String,
    pub success_message: String,
    pub button_label: String,
    pub vote_protection: String,
}

/// Raw shape of one option as JSON-encoded by the editor's submit handler.
#[derive(Deserialize)]
struct RawOption {
    key: String,
    label: String,
}

fn parse_options(raw: &str) -> Vec<PollOption> {
    serde_json::from_str::<Vec<RawOption>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(|o| PollOption { key: o.key, label: o.label })
        .collect()
}

/// Mirrors the JS-side checks in the editor (name/question required + sane
/// length, at least two options) so a submission that bypasses the browser
/// (JS disabled, direct POST) can't write incomplete or oversized data.
fn validate_poll(name: &str, question: &str, options: &[PollOption]) -> Option<String> {
    let name_len = name.trim().chars().count();
    if name_len < 5 || name_len > 255 {
        return Some("Poll name must be between 5 and 255 characters.".to_string());
    }
    let question_len = question.trim().chars().count();
    if question_len < 5 || question_len > 255 {
        return Some("Question must be between 5 and 255 characters.".to_string());
    }
    if options.len() < 2 {
        return Some("A poll needs at least two options.".to_string());
    }
    for o in options {
        let label_len = o.label.trim().chars().count();
        if label_len < 1 || label_len > 255 {
            return Some("Each option label must be between 1 and 255 characters.".to_string());
        }
    }
    None
}

fn settings_from_form(form: &SavePollForm) -> PollSettings {
    PollSettings {
        success_message: if form.success_message.trim().is_empty() {
            "Thanks for voting!".to_string()
        } else {
            form.success_message.clone()
        },
        button_label: if form.button_label.trim().is_empty() {
            "Vote".to_string()
        } else {
            form.button_label.clone()
        },
        vote_protection: VoteProtection::from_str(&form.vote_protection).unwrap_or_default(),
    }
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    axum::Form(form): axum::Form<SavePollForm>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let options = parse_options(&form.options_json);
    let settings = settings_from_form(&form);

    if let Some(msg) = validate_poll(&form.name, &form.question, &options) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let data = PollEditData {
            id: None,
            name: form.name,
            question: form.question,
            options: options.into_iter().map(|o| PollOptionRow { key: o.key, label: o.label }).collect(),
            success_message: settings.success_message,
            button_label: settings.button_label,
            vote_protection: settings.vote_protection.as_str().to_string(),
        };
        return Html(render_editor(&data, &ctx, Some(&msg))).into_response();
    }

    if let Err(e) = poll_def::create(&state.db, CreatePollDef { site_id, name: form.name, question: form.question, options, settings }).await {
        tracing::error!("poll_designer::create failed: {e}");
    }
    Redirect::to("/admin/designer?tab=polls").into_response()
}

pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Form(form): axum::Form<SavePollForm>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let options = parse_options(&form.options_json);
    let settings = settings_from_form(&form);

    if let Some(msg) = validate_poll(&form.name, &form.question, &options) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let data = PollEditData {
            id: Some(id.to_string()),
            name: form.name,
            question: form.question,
            options: options.into_iter().map(|o| PollOptionRow { key: o.key, label: o.label }).collect(),
            success_message: settings.success_message,
            button_label: settings.button_label,
            vote_protection: settings.vote_protection.as_str().to_string(),
        };
        return Html(render_editor(&data, &ctx, Some(&msg))).into_response();
    }

    if let Err(e) = poll_def::update(&state.db, site_id, id, UpdatePollDef { name: form.name, question: form.question, options, settings }).await {
        tracing::error!("poll_designer::update failed: {e}");
    }

    Redirect::to(&format!("/admin/designer/polls/{id}")).into_response()
}

pub async fn delete(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    if let Err(e) = poll_def::delete(&state.db, site_id, id).await {
        tracing::error!("poll_designer::delete failed: {e}");
    }

    Redirect::to("/admin/designer?tab=polls").into_response()
}
