//! Admin handlers for the Form Designer — CRUD over saved form definitions.

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
use crate::models::form_def::{self, CreateFormDef, FormField, FormSettings, UpdateFormDef};
use crate::models::mail_log;

use admin::pages::form_designer::{forms_list_fragment, render_editor, render_list, FieldRow, FormEditData, FormRow};

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

// ── list ─────────────────────────────────────────────────────────────────────

pub async fn list(State(state): State<AppState>, admin: AdminUser, Query(params): Query<HashMap<String, String>>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let forms = form_def::list_for_site(&state.db, site_id).await.unwrap_or_default();
    let mut rows: Vec<FormRow> = forms.into_iter().map(|f| FormRow {
        id: f.id.to_string(),
        name: f.name,
        slug: f.slug,
        field_count: f.fields.len(),
        updated_at: f.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    }).collect();

    // In-memory search + pagination — same reasoning as /admin/sites: the
    // list is small enough per site that a second SQL query isn't worth
    // the added complexity over filtering/slicing the Vec already fetched.
    let search = params.get("search").map(|s| s.trim()).unwrap_or("");
    if !search.is_empty() {
        let needle = search.to_lowercase();
        rows.retain(|r| r.name.to_lowercase().contains(&needle) || r.slug.to_lowercase().contains(&needle));
    }

    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("");
    let dir = params.get("dir").map(|s| s.as_str()).unwrap_or("");
    match sort {
        "slug"   => rows.sort_by_key(|r| r.slug.to_lowercase()),
        "fields" => rows.sort_by_key(|r| r.field_count),
        "name"   => rows.sort_by_key(|r| r.name.to_lowercase()),
        _ => {}
    }
    if !sort.is_empty() && dir == "desc" {
        rows.reverse();
    }

    const PER_PAGE: i64 = 20;
    let total = rows.len() as i64;
    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let page = params.get("page").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).clamp(1, total_pages);
    let start = ((page - 1) * PER_PAGE) as usize;
    let end = (start + PER_PAGE as usize).min(rows.len());
    let page_rows = rows.get(start..end).unwrap_or(&[]);

    if params.contains_key("partial") {
        return Html(forms_list_fragment(page_rows, page, total_pages, search, sort, dir)).into_response();
    }

    Html(render_list(page_rows, page, total_pages, search, sort, dir, &ctx, None)).into_response()
}

// ── new / edit form ─────────────────────────────────────────────────────────

pub async fn new_form(State(state): State<AppState>, admin: AdminUser) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    if let Err(e) = require_site_id(&admin) { return e; }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    Html(render_editor(&FormEditData::default(), &ctx, None)).into_response()
}

pub async fn edit_form(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Ok(Some(form)) = form_def::get_by_id(&state.db, site_id, id).await else {
        return Redirect::to("/admin/form-designer").into_response();
    };

    let data = FormEditData {
        id: Some(form.id.to_string()),
        name: form.name,
        fields: form.fields.into_iter().map(|f| FieldRow {
            label: f.label,
            name: f.name,
            field_type: f.field_type,
            required: f.required,
            options_text: f.options.into_iter().map(|(v, l)| {
                if v == l { l } else { format!("{v}|{l}") }
            }).collect::<Vec<_>>().join("\n"),
        }).collect(),
        success_message: form.settings.success_message,
        button_label: form.settings.button_label,
        include_honeypot: form.settings.include_honeypot,
        notify_email: form.settings.notify_email.unwrap_or_default(),
        confirm_submitter: form.settings.confirm_submitter,
        confirm_subject: form.settings.confirm_subject,
        confirm_body: form.settings.confirm_body,
    };

    Html(render_editor(&data, &ctx, None)).into_response()
}

// ── analytics (placeholder) ──────────────────────────────────────────────────

pub async fn analytics(
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
        return Redirect::to("/admin/form-designer").into_response();
    };

    let (total_sent, succeeded, failed) = mail_log::counts_for_form(&state.db, id).await.unwrap_or((0, 0, 0));
    let recent = mail_log::list_for_form(&state.db, id, 50).await.unwrap_or_default();

    let mut recent: Vec<admin::pages::form_designer::MailLogRow> = recent.into_iter().map(|r| admin::pages::form_designer::MailLogRow {
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

    let data = admin::pages::form_designer::FormAnalyticsData {
        id: id.to_string(),
        form_name: form.name,
        total_sent,
        succeeded,
        failed,
        recent,
    };

    if params.contains_key("partial") {
        let search_qs = if search.is_empty() { String::new() } else { format!("&search={}", admin::html_escape(search)) };
        return Html(admin::pages::form_designer::render_analytics_table(&data, sort, dir, &search_qs)).into_response();
    }

    Html(admin::pages::form_designer::render_analytics(&data, sort, dir, search, &ctx)).into_response()
}

// ── create / update ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveFormForm {
    pub name: String,
    pub fields_json: String,
    pub success_message: String,
    pub button_label: String,
    pub include_honeypot: Option<String>,
    pub notify_email: Option<String>,
    pub confirm_submitter: Option<String>,
    pub confirm_subject: String,
    pub confirm_body: String,
}

/// Raw shape of one field as JSON-encoded by the editor's submit handler —
/// `options` is a plain array of `[value, label]` pairs.
#[derive(Deserialize)]
struct RawField {
    label: String,
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    required: bool,
    #[serde(default)]
    options: Vec<(String, String)>,
}

fn parse_fields(raw: &str) -> Vec<FormField> {
    serde_json::from_str::<Vec<RawField>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(|f| FormField {
            label: f.label,
            name: f.name,
            field_type: f.field_type,
            required: f.required,
            options: f.options,
        })
        .collect()
}

fn settings_from_form(form: &SaveFormForm) -> FormSettings {
    FormSettings {
        success_message: if form.success_message.trim().is_empty() {
            "Thank you for your submission!".to_string()
        } else {
            form.success_message.clone()
        },
        button_label: if form.button_label.trim().is_empty() {
            "Submit".to_string()
        } else {
            form.button_label.clone()
        },
        include_honeypot: form.include_honeypot.as_deref() == Some("true"),
        notify_email: form.notify_email.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        confirm_submitter: form.confirm_submitter.as_deref() == Some("true"),
        confirm_subject: if form.confirm_subject.trim().is_empty() {
            "We've received your submission".to_string()
        } else {
            form.confirm_subject.clone()
        },
        confirm_body: if form.confirm_body.trim().is_empty() {
            "Thanks for reaching out! We've received your submission and will follow up soon.".to_string()
        } else {
            form.confirm_body.clone()
        },
    }
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    axum::Form(form): axum::Form<SaveFormForm>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let fields = parse_fields(&form.fields_json);
    let settings = settings_from_form(&form);

    if let Err(e) = form_def::create(&state.db, CreateFormDef { site_id, name: form.name, fields, settings }).await {
        tracing::error!("form_designer::create failed: {e}");
    }
    Redirect::to("/admin/form-designer").into_response()
}

pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Form(form): axum::Form<SaveFormForm>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    let fields = parse_fields(&form.fields_json);
    let settings = settings_from_form(&form);

    if let Err(e) = form_def::update(&state.db, site_id, id, UpdateFormDef { name: form.name, fields, settings }).await {
        tracing::error!("form_designer::update failed: {e}");
    }

    Redirect::to(&format!("/admin/form-designer/{id}")).into_response()
}

pub async fn delete(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    if let Err(e) = require_forms_cap(&admin) { return e; }
    let site_id = match require_site_id(&admin) { Ok(id) => id, Err(e) => return e };

    if let Err(e) = form_def::delete(&state.db, site_id, id).await {
        tracing::error!("form_designer::delete failed: {e}");
    }

    Redirect::to("/admin/form-designer").into_response()
}
