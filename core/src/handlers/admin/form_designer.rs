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

use admin::pages::form_designer::{
    forms_list_fragment, render_editor, AiProviderOption, FieldRow, FormEditData, FormRow,
    ProviderOption, TranslationSummary,
};

async fn translation_editor_data(
    state: &AppState,
    site_id: Uuid,
    form_id: Uuid,
    source_updated_at: chrono::DateTime<chrono::Utc>,
) -> (
    Vec<(String, String)>,
    Vec<AiProviderOption>,
    Vec<TranslationSummary>,
) {
    let locales = crate::models::site_locale::enabled_locales_for_site(&state.db, site_id)
        .await
        .into_iter()
        .filter_map(|code| {
            crate::utils::locales::display_name(&code).map(|name| (code, name.to_string()))
        })
        .collect();
    let providers = crate::models::ai_provider::list_verified_for_site(&state.db, site_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| AiProviderOption {
            id: p.id.to_string(),
            label: p.label,
        })
        .collect();
    let translations = crate::models::embedded_translation::list_for_form(&state.db, form_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| TranslationSummary {
            locale_name: crate::utils::locales::display_name(&t.locale)
                .unwrap_or(&t.locale)
                .to_string(),
            is_stale: t.is_stale(source_updated_at) || t.parsed().is_none(),
            generated_at: t.generated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            locale: t.locale,
        })
        .collect();
    (locales, providers, translations)
}

fn require_forms_cap(admin: &AdminUser) -> Result<(), Response> {
    if !admin.caps.can_manage_forms {
        Err((StatusCode::FORBIDDEN, "Forbidden").into_response())
    } else {
        Ok(())
    }
}

fn require_site_id(admin: &AdminUser) -> Result<Uuid, Response> {
    admin
        .site_id
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No site selected.").into_response())
}

// ── list ─────────────────────────────────────────────────────────────────────

/// `/admin/form-designer` (GET, no params) now redirects to the consolidated
/// `/admin/designer?tab=forms` hub — kept as a route (rather than removed)
/// so an old bookmark/link still lands somewhere sensible. `?partial=1`
/// live-search requests still render `forms_list_fragment` directly, since
/// nothing on the hub page issues those requests today, but the function
/// stays in case a future hub redesign wants live search back.
pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    if !params.contains_key("partial") {
        return Redirect::to("/admin/designer?tab=forms").into_response();
    }

    let forms = form_def::list_for_site(&state.db, site_id)
        .await
        .unwrap_or_default();
    let blocked = crate::models::form_submission::blocked_names(&state.db, site_id).await;
    let mut rows: Vec<FormRow> = forms
        .into_iter()
        .map(|f| FormRow {
            id: f.id.to_string(),
            blocked: blocked.contains(&f.name),
            name: f.name,
            slug: f.slug,
            field_count: f.fields.len(),
            updated_at: f.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        })
        .collect();

    let search = params.get("search").map(|s| s.trim()).unwrap_or("");
    if !search.is_empty() {
        let needle = search.to_lowercase();
        rows.retain(|r| {
            r.name.to_lowercase().contains(&needle) || r.slug.to_lowercase().contains(&needle)
        });
    }

    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("");
    let dir = params.get("dir").map(|s| s.as_str()).unwrap_or("");
    match sort {
        "slug" => rows.sort_by_key(|r| r.slug.to_lowercase()),
        "fields" => rows.sort_by_key(|r| r.field_count),
        "name" => rows.sort_by_key(|r| r.name.to_lowercase()),
        _ => {}
    }
    if !sort.is_empty() && dir == "desc" {
        rows.reverse();
    }

    const PER_PAGE: i64 = 20;
    let total = rows.len() as i64;
    let total_pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let page = params
        .get("page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1)
        .clamp(1, total_pages);
    let start = ((page - 1) * PER_PAGE) as usize;
    let end = (start + PER_PAGE as usize).min(rows.len());
    let page_rows = rows.get(start..end).unwrap_or(&[]);

    Html(forms_list_fragment(
        page_rows,
        page,
        total_pages,
        search,
        sort,
        dir,
    ))
    .into_response()
}

// ── new / edit form ─────────────────────────────────────────────────────────

pub async fn new_form(State(state): State<AppState>, admin: AdminUser) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let providers = crate::models::email_provider::list_verified_for_site(&state.db, site_id)
        .await
        .unwrap_or_default();
    let data = FormEditData {
        provider_options: providers
            .into_iter()
            .map(|p| ProviderOption {
                id: p.id.to_string(),
                label: format!("{} - {}", p.label, p.provider_type),
            })
            .collect(),
        site_id: site_id.to_string(),
        ai_translation_enabled: state.app_settings.read().unwrap().ai_translation_enabled,
        ..FormEditData::default()
    };

    Html(render_editor(&data, &ctx, None)).into_response()
}

pub async fn edit_form(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let Ok(Some(form)) = form_def::get_by_id(&state.db, site_id, id).await else {
        return Redirect::to("/admin/form-designer").into_response();
    };

    let providers = crate::models::email_provider::list_verified_for_site(&state.db, site_id)
        .await
        .unwrap_or_default();
    let (translation_locales, ai_provider_options, translations) =
        translation_editor_data(&state, site_id, form.id, form.updated_at).await;

    let data = FormEditData {
        id: Some(form.id.to_string()),
        name: form.name,
        fields: form
            .fields
            .into_iter()
            .map(|f| FieldRow {
                label: f.label,
                name: f.name,
                field_type: f.field_type,
                required: f.required,
                options_text: f
                    .options
                    .into_iter()
                    .map(|(v, l)| if v == l { l } else { format!("{v}|{l}") })
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
            .collect(),
        success_message: form.settings.success_message,
        button_label: form.settings.button_label,
        include_honeypot: form.settings.include_honeypot,
        notify_email: form.settings.notify_email.unwrap_or_default(),
        confirm_submitter: form.settings.confirm_submitter,
        confirm_subject: form.settings.confirm_subject,
        confirm_body: form.settings.confirm_body,
        no_mail: form.settings.no_mail,
        email_provider_id: form
            .email_provider_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        provider_options: providers
            .into_iter()
            .map(|p| ProviderOption {
                id: p.id.to_string(),
                label: format!("{} - {}", p.label, p.provider_type),
            })
            .collect(),
        site_id: site_id.to_string(),
        ai_translation_enabled: state.app_settings.read().unwrap().ai_translation_enabled,
        translation_locales,
        ai_provider_options,
        translations,
    };

    Html(render_editor(&data, &ctx, None)).into_response()
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
    pub no_mail: Option<String>,
    #[serde(default)]
    pub email_provider_id: Option<String>,
    #[serde(default)]
    pub active_tab: Option<String>,
}

/// Query-string suffix to keep the same Form Settings tab active after a
/// save reloads the page, e.g. editing Mail Settings shouldn't bounce back
/// to General Settings just because it's first in the list.
fn tab_suffix(form: &SaveFormForm) -> String {
    match form.active_tab.as_deref() {
        Some(tab) if tab == "mail" || tab == "preview" => format!("?tab={tab}"),
        _ => String::new(),
    }
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

/// Mirrors `admin::pages::form_designer::is_visual_only` — separator/note
/// fields don't submit a value, so "Field name" doesn't apply to them.
fn is_visual_only(field_type: &str) -> bool {
    matches!(field_type, "separator" | "note")
}

/// Mirrors the JS-side checks in the editor (name required + sane length,
/// at least one field, each field's label/name filled in) so a submission
/// that bypasses the browser (JS disabled, direct POST) can't write
/// incomplete or oversized data.
fn validate_form(name: &str, fields: &[FormField]) -> Option<String> {
    let name_len = name.trim().chars().count();
    if name_len < 5 || name_len > 255 {
        return Some("Form name must be between 5 and 255 characters.".to_string());
    }
    if fields.is_empty() {
        return Some("A form needs at least one field.".to_string());
    }
    for f in fields {
        if is_visual_only(&f.field_type) {
            if f.field_type == "note" && f.label.trim().is_empty() {
                return Some("Note text can't be empty.".to_string());
            }
            if f.label.trim().chars().count() > 255 {
                return Some("Field label must be 255 characters or fewer.".to_string());
            }
            continue;
        }
        let label_len = f.label.trim().chars().count();
        if label_len < 1 || label_len > 255 {
            return Some("Each field label must be between 1 and 255 characters.".to_string());
        }
        let name_len = f.name.trim().chars().count();
        if name_len < 1 || name_len > 100 {
            return Some("Each field name must be between 1 and 100 characters.".to_string());
        }
    }
    None
}

fn parse_provider_id(form: &SaveFormForm) -> Option<Uuid> {
    form.email_provider_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
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
        notify_email: form
            .notify_email
            .as_deref()
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
            "Thanks for reaching out! We've received your submission and will follow up soon."
                .to_string()
        } else {
            form.confirm_body.clone()
        },
        no_mail: form.no_mail.as_deref() == Some("true"),
    }
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    axum::Form(form): axum::Form<SaveFormForm>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    let fields = parse_fields(&form.fields_json);
    let email_provider_id = parse_provider_id(&form);
    let settings = settings_from_form(&form);

    if let Some(msg) = validate_form(&form.name, &fields) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let providers = crate::models::email_provider::list_verified_for_site(&state.db, site_id)
            .await
            .unwrap_or_default();
        let data = FormEditData {
            id: None,
            name: form.name,
            fields: fields
                .into_iter()
                .map(|f| FieldRow {
                    label: f.label,
                    name: f.name,
                    field_type: f.field_type,
                    required: f.required,
                    options_text: f
                        .options
                        .into_iter()
                        .map(|(v, l)| if v == l { l } else { format!("{v}|{l}") })
                        .collect::<Vec<_>>()
                        .join("\n"),
                })
                .collect(),
            success_message: settings.success_message,
            button_label: settings.button_label,
            include_honeypot: settings.include_honeypot,
            notify_email: settings.notify_email.unwrap_or_default(),
            confirm_submitter: settings.confirm_submitter,
            confirm_subject: settings.confirm_subject,
            confirm_body: settings.confirm_body,
            no_mail: settings.no_mail,
            email_provider_id: email_provider_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            provider_options: providers
                .into_iter()
                .map(|p| ProviderOption {
                    id: p.id.to_string(),
                    label: format!("{} - {}", p.label, p.provider_type),
                })
                .collect(),
            site_id: site_id.to_string(),
            ai_translation_enabled: state.app_settings.read().unwrap().ai_translation_enabled,
            translation_locales: Vec::new(),
            ai_provider_options: Vec::new(),
            translations: Vec::new(),
        };
        return Html(render_editor(&data, &ctx, Some(&msg))).into_response();
    }

    if let Err(e) = form_def::create(
        &state.db,
        CreateFormDef {
            site_id,
            name: form.name,
            fields,
            settings,
            email_provider_id,
        },
    )
    .await
    {
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
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    let fields = parse_fields(&form.fields_json);
    let email_provider_id = parse_provider_id(&form);
    let settings = settings_from_form(&form);

    if let Some(msg) = validate_form(&form.name, &fields) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let providers = crate::models::email_provider::list_verified_for_site(&state.db, site_id)
            .await
            .unwrap_or_default();
        let data = FormEditData {
            id: Some(id.to_string()),
            name: form.name,
            fields: fields
                .into_iter()
                .map(|f| FieldRow {
                    label: f.label,
                    name: f.name,
                    field_type: f.field_type,
                    required: f.required,
                    options_text: f
                        .options
                        .into_iter()
                        .map(|(v, l)| if v == l { l } else { format!("{v}|{l}") })
                        .collect::<Vec<_>>()
                        .join("\n"),
                })
                .collect(),
            success_message: settings.success_message,
            button_label: settings.button_label,
            include_honeypot: settings.include_honeypot,
            notify_email: settings.notify_email.unwrap_or_default(),
            confirm_submitter: settings.confirm_submitter,
            confirm_subject: settings.confirm_subject,
            confirm_body: settings.confirm_body,
            no_mail: settings.no_mail,
            email_provider_id: email_provider_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            provider_options: providers
                .into_iter()
                .map(|p| ProviderOption {
                    id: p.id.to_string(),
                    label: format!("{} - {}", p.label, p.provider_type),
                })
                .collect(),
            site_id: site_id.to_string(),
            ai_translation_enabled: state.app_settings.read().unwrap().ai_translation_enabled,
            translation_locales: Vec::new(),
            ai_provider_options: Vec::new(),
            translations: Vec::new(),
        };
        return Html(render_editor(&data, &ctx, Some(&msg))).into_response();
    }

    let suffix = tab_suffix(&form);
    if let Err(e) = form_def::update(
        &state.db,
        site_id,
        id,
        UpdateFormDef {
            name: form.name,
            fields,
            settings,
            email_provider_id,
        },
    )
    .await
    {
        tracing::error!("form_designer::update failed: {e}");
    }

    Redirect::to(&format!("/admin/form-designer/{id}{suffix}")).into_response()
}

#[derive(Deserialize)]
pub struct TranslateFormRequest {
    pub locale: String,
    pub provider_id: Uuid,
}

pub async fn translate(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Form(request): axum::Form<TranslateFormRequest>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };
    if !state.app_settings.read().unwrap().ai_translation_enabled {
        return (StatusCode::FORBIDDEN, "AI Translation is disabled.").into_response();
    }
    let Ok(Some(form)) = form_def::get_by_id(&state.db, site_id, id).await else {
        return (StatusCode::NOT_FOUND, "Form not found.").into_response();
    };
    let enabled = crate::models::site_locale::enabled_locales_for_site(&state.db, site_id).await;
    let Some(locale_name) = crate::utils::locales::display_name(&request.locale) else {
        return (StatusCode::BAD_REQUEST, "Unknown language.").into_response();
    };
    if !enabled.iter().any(|locale| locale == &request.locale) {
        return (
            StatusCode::BAD_REQUEST,
            "Language is not enabled for this site.",
        )
            .into_response();
    }
    let provider = match crate::models::ai_provider::get_by_id(&state.db, request.provider_id).await
    {
        Ok(Some(row)) if row.site_id == site_id && row.verified => row,
        _ => return (StatusCode::BAD_REQUEST, "Verified AI provider not found.").into_response(),
    };
    let Some(config) =
        crate::models::ai_provider::decrypt_config(&state.config.secret_key, &provider)
    else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to decrypt AI provider config.",
        )
            .into_response();
    };

    let attempt_id = Uuid::new_v4();
    let started_at = std::time::Instant::now();
    tracing::info!(target: "ai_translation", event="translation_attempt_started", operation="form_translation", %attempt_id, %site_id, form_id=%id, locale=%request.locale, provider_id=%provider.id, provider_type=config.provider_type(), model=config.model_name(), admin_user_id=%admin.user.id, "AI form translation attempt started");
    match crate::translate::translate_form(&config, &form, locale_name).await {
        Ok(payload) => {
            if let Err(error) = crate::models::embedded_translation::upsert_form(
                &state.db,
                id,
                &request.locale,
                &payload,
                form.updated_at,
            )
            .await
            {
                tracing::error!(target: "ai_translation", event="translation_attempt_finished", outcome="failure", stage="persistence", operation="form_translation", %attempt_id, %site_id, form_id=%id, locale=%request.locale, provider_id=%provider.id, provider_type=config.provider_type(), model=config.model_name(), admin_user_id=%admin.user.id, duration_ms=started_at.elapsed().as_millis() as u64, error=%error, "AI form translation attempt failed");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Translation could not be saved.",
                )
                    .into_response();
            }
            tracing::info!(target: "ai_translation", event="translation_attempt_finished", outcome="success", stage="complete", operation="form_translation", %attempt_id, %site_id, form_id=%id, locale=%request.locale, provider_id=%provider.id, provider_type=config.provider_type(), model=config.model_name(), admin_user_id=%admin.user.id, duration_ms=started_at.elapsed().as_millis() as u64, "AI form translation attempt succeeded");
            Redirect::to(&format!("/admin/form-designer/{id}?tab=translations")).into_response()
        }
        Err(error) => {
            tracing::error!(target: "ai_translation", event="translation_attempt_finished", outcome="failure", stage="provider_or_response", operation="form_translation", %attempt_id, %site_id, form_id=%id, locale=%request.locale, provider_id=%provider.id, provider_type=config.provider_type(), model=config.model_name(), admin_user_id=%admin.user.id, duration_ms=started_at.elapsed().as_millis() as u64, error=%error, "AI form translation attempt failed");
            (
                StatusCode::BAD_GATEWAY,
                format!("Translation failed: {error}"),
            )
                .into_response()
        }
    }
}

pub async fn delete_translation(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, locale)): Path<(Uuid, String)>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };
    if !matches!(
        form_def::get_by_id(&state.db, site_id, id).await,
        Ok(Some(_))
    ) {
        return (StatusCode::NOT_FOUND, "Form not found.").into_response();
    }
    if let Err(error) =
        crate::models::embedded_translation::delete_form(&state.db, id, &locale).await
    {
        tracing::error!(form_id=%id, %locale, %error, "failed to delete form translation");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Translation could not be deleted.",
        )
            .into_response();
    }
    Redirect::to(&format!("/admin/form-designer/{id}?tab=translations")).into_response()
}

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(e) = require_forms_cap(&admin) {
        return e;
    }
    let site_id = match require_site_id(&admin) {
        Ok(id) => id,
        Err(e) => return e,
    };

    if let Err(e) = form_def::delete(&state.db, site_id, id).await {
        tracing::error!("form_designer::delete failed: {e}");
    }

    Redirect::to("/admin/form-designer").into_response()
}
