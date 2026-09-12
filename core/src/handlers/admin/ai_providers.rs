//! Admin handlers for a site's configured AI translation providers (AI
//! Translation tab on Site Settings), including live model discovery.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Form, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::ai_provider::{self, AiProviderConfig};

use super::sites::require_site_manager;

#[derive(Deserialize, Default)]
pub struct AiProviderForm {
    pub label: String,
    pub provider_type: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub anthropic_model_name: String,
    #[serde(default)]
    pub anthropic_model_choice: String,
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default)]
    pub deepseek_model_name: String,
    #[serde(default)]
    pub deepseek_model_choice: String,
    #[serde(default)]
    pub openai_compatible_base_url: String,
    #[serde(default)]
    pub openai_compatible_api_key: String,
    #[serde(default)]
    pub openai_compatible_model_name: String,
    #[serde(default)]
    pub openai_compatible_model_choice: String,
}

fn selected_model<'a>(choice: &'a str, custom: &'a str, saved: Option<&'a str>) -> &'a str {
    if !custom.trim().is_empty() {
        custom.trim()
    } else if !choice.trim().is_empty() && choice != "__custom__" {
        choice.trim()
    } else {
        saved.unwrap_or("").trim()
    }
}

/// Build a config while optionally merging blank edit fields with the saved
/// encrypted config. `require_model=false` is used only for discovery, where
/// credentials must be valid but no model has been selected yet.
fn config_from_form(
    form: &AiProviderForm,
    existing: Option<&AiProviderConfig>,
    require_model: bool,
) -> Result<AiProviderConfig, &'static str> {
    match form.provider_type.as_str() {
        "anthropic" => {
            let (saved_key, saved_model) = match existing {
                Some(AiProviderConfig::Anthropic {
                    api_key,
                    model_name,
                }) => (Some(api_key.as_str()), Some(model_name.as_str())),
                Some(_) => return Err("The provider type cannot be changed."),
                None => (None, None),
            };
            let api_key = if form.anthropic_api_key.trim().is_empty() {
                saved_key.unwrap_or("")
            } else {
                form.anthropic_api_key.trim()
            };
            let model_name = selected_model(
                &form.anthropic_model_choice,
                &form.anthropic_model_name,
                saved_model,
            );
            if api_key.is_empty() {
                return Err("Enter an API key before loading models.");
            }
            if require_model && model_name.is_empty() {
                return Err("Load and select a model, or enter a custom model ID.");
            }
            Ok(AiProviderConfig::Anthropic {
                api_key: api_key.to_string(),
                model_name: model_name.to_string(),
            })
        }
        "deepseek" => {
            let (saved_key, saved_model) = match existing {
                Some(AiProviderConfig::Deepseek {
                    api_key,
                    model_name,
                }) => (Some(api_key.as_str()), Some(model_name.as_str())),
                Some(_) => return Err("The provider type cannot be changed."),
                None => (None, None),
            };
            let api_key = if form.deepseek_api_key.trim().is_empty() {
                saved_key.unwrap_or("")
            } else {
                form.deepseek_api_key.trim()
            };
            let model_name = selected_model(
                &form.deepseek_model_choice,
                &form.deepseek_model_name,
                saved_model,
            );
            if api_key.is_empty() {
                return Err("Enter an API key before loading models.");
            }
            if require_model && model_name.is_empty() {
                return Err("Load and select a model, or enter a custom model ID.");
            }
            Ok(AiProviderConfig::Deepseek {
                api_key: api_key.to_string(),
                model_name: model_name.to_string(),
            })
        }
        "openai_compatible" => {
            let (saved_url, saved_key, saved_model) = match existing {
                Some(AiProviderConfig::OpenaiCompatible {
                    base_url,
                    api_key,
                    model_name,
                }) => (
                    Some(base_url.as_str()),
                    Some(api_key.as_str()),
                    Some(model_name.as_str()),
                ),
                Some(_) => return Err("The provider type cannot be changed."),
                None => (None, None, None),
            };
            let base_url = if form.openai_compatible_base_url.trim().is_empty() {
                saved_url.unwrap_or("")
            } else {
                form.openai_compatible_base_url.trim()
            };
            let api_key = if form.openai_compatible_api_key.trim().is_empty() {
                saved_key.unwrap_or("")
            } else {
                form.openai_compatible_api_key.trim()
            };
            let model_name = selected_model(
                &form.openai_compatible_model_choice,
                &form.openai_compatible_model_name,
                saved_model,
            );
            if base_url.is_empty() {
                return Err("Enter a base URL before loading models.");
            }
            if require_model && model_name.is_empty() {
                return Err("Load and select a model, or enter a custom model ID.");
            }
            Ok(AiProviderConfig::OpenaiCompatible {
                base_url: base_url.to_string(),
                api_key: api_key.to_string(),
                model_name: model_name.to_string(),
            })
        }
        _ => Err("Unknown provider type."),
    }
}

/// True when the installation-wide AI Translation switch
/// (`AppSettings::ai_translation_enabled`, set by a super admin at
/// /admin/settings) is on. Every handler in this module checks this itself,
/// first thing, before any site lookup — hiding the AI Translation tab in
/// the site settings UI (`admin::pages::sites::render_settings`) is not
/// enforcement on its own, since a signed-in site admin (or anyone who knows
/// the URL shape) could otherwise still reach these routes directly while
/// the tab is hidden.
fn ai_translation_enabled(state: &AppState) -> bool {
    state.app_settings.read().unwrap().ai_translation_enabled
}

const AI_TRANSLATION_DISABLED_MSG: &str = "AI Translation is disabled for this installation.";

/// The `openai_compatible` provider type accepts an admin-entered base URL
/// with no restriction on scheme or destination — deliberately, so it can
/// point at a self-hosted model server (Ollama, LM Studio) on localhost or
/// the local network. That makes it a real SSRF primitive: whoever can
/// configure or test one can make this server issue requests to any URL it
/// can reach. On a single-owner install the super admin already controls
/// that network, so it's not a boundary crossing; on a multi-tenant install
/// a site-scoped admin does not control the underlying box, so this option
/// is restricted to global admins only — same reasoning as `provision_ssl`
/// in `handlers::admin::sites`.
const LOCAL_MODEL_FORBIDDEN_MSG: &str =
    "Local/self-hosted model providers (custom base URL) require super admin access.";

fn is_unique_violation(err: &crate::errors::AppError) -> bool {
    matches!(
        err,
        crate::errors::AppError::Database(sqlx::Error::Database(db_err)) if db_err.is_unique_violation()
    )
}

fn flash_redirect(site_id: Uuid, msg: &str) -> Redirect {
    let msg = crate::handlers::admin::themes::url_encode_param(msg);
    Redirect::to(&format!(
        "/admin/sites/{}/settings?flash={}&tab=ai-translation",
        site_id, msg
    ))
}

/// POST /admin/sites/{id}/ai-providers — add a new provider.
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<AiProviderForm>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return (StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG).into_response();
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if form.label.trim().is_empty() {
        return flash_redirect(id, "Enter a label for this provider.").into_response();
    }
    match ai_provider::label_exists_for_site(&state.db, id, form.label.trim(), None).await {
        Ok(true) => {
            return flash_redirect(id, "A provider with that label already exists.").into_response()
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(
                "failed to check AI provider label uniqueness for site {}: {:?}",
                id,
                e
            );
            return flash_redirect(id, "Failed to save provider.").into_response();
        }
    }
    let config = match config_from_form(&form, None, true) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };
    if config.requires_global_admin() && !admin.caps.is_global_admin {
        return (StatusCode::FORBIDDEN, LOCAL_MODEL_FORBIDDEN_MSG).into_response();
    }

    let row = match ai_provider::create(
        &state.db,
        id,
        form.label.trim(),
        &config,
        &state.config.secret_key,
    )
    .await
    {
        Ok(row) => row,
        // Belt-and-suspenders against the `label_exists_for_site` check
        // above racing a second near-simultaneous submission (e.g. a slow
        // provider verification making a double-click look like nothing
        // happened) — the DB-level unique constraint (see migrations/
        // 0007_ai_provider_label_unique.sql) is what actually closes the
        // race; this just keeps the error message friendly when it fires.
        Err(e) if is_unique_violation(&e) => {
            return flash_redirect(id, "A provider with that label already exists.")
                .into_response();
        }
        Err(e) => {
            tracing::error!("failed to create AI provider for site {}: {:?}", id, e);
            return flash_redirect(id, "Failed to save provider.").into_response();
        }
    };

    match crate::translate::test_provider(id, &config).await {
        Ok(()) => {
            if let Err(e) = ai_provider::mark_verified(&state.db, row.id).await {
                tracing::error!("failed to mark AI provider {} verified: {:?}", row.id, e);
            }
            Redirect::to(&format!(
                "/admin/sites/{}/settings?flash=Provider added and verified.&tab=ai-translation",
                id
            ))
            .into_response()
        }
        Err(e) => {
            tracing::warn!(
                "verification failed for new AI provider {}: {:?}",
                row.id,
                e
            );
            flash_redirect(
                id,
                &format!("Provider added, but verification failed: {e}. Use the Test icon to retry once fixed."),
            )
            .into_response()
        }
    }
}

/// POST /admin/sites/{id}/ai-providers/{provider_id} — update an existing
/// provider's label/configuration. Blank credential and non-secret fields
/// retain their stored values, because credentials are never sent back to the
/// browser. Resets `verified` to false (see `ai_provider::update`).
pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<AiProviderForm>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return (StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG).into_response();
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if form.label.trim().is_empty() {
        return flash_redirect(id, "Enter a label for this provider.").into_response();
    }
    match ai_provider::label_exists_for_site(&state.db, id, form.label.trim(), Some(provider_id))
        .await
    {
        Ok(true) => {
            return flash_redirect(id, "A provider with that label already exists.").into_response()
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(
                "failed to check AI provider label uniqueness for site {}: {:?}",
                id,
                e
            );
            return flash_redirect(id, "Failed to save provider.").into_response();
        }
    }
    let row = match ai_provider::get_by_id(&state.db, provider_id).await {
        Ok(Some(row)) if row.site_id == id => row,
        _ => return flash_redirect(id, "Provider not found.").into_response(),
    };
    let Some(existing) = ai_provider::decrypt_config(&state.config.secret_key, &row) else {
        return flash_redirect(id, "Failed to decrypt provider config.").into_response();
    };
    let config = match config_from_form(&form, Some(&existing), true) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };
    if config.requires_global_admin() && !admin.caps.is_global_admin {
        return (StatusCode::FORBIDDEN, LOCAL_MODEL_FORBIDDEN_MSG).into_response();
    }

    match ai_provider::update(&state.db, provider_id, id, form.label.trim(), &config, &state.config.secret_key).await {
        Ok(Some(_)) => match crate::translate::test_provider(id, &config).await {
            Ok(()) => {
                if let Err(e) = ai_provider::mark_verified(&state.db, provider_id).await {
                    tracing::error!(
                        "failed to mark AI provider {} verified: {:?}",
                        provider_id,
                        e
                    );
                }
                Redirect::to(&format!(
                    "/admin/sites/{}/settings?flash=Provider updated and verified.&tab=ai-translation",
                    id
                ))
                .into_response()
            }
            Err(e) => {
                tracing::warn!(
                    "verification failed for updated AI provider {}: {:?}",
                    provider_id,
                    e
                );
                flash_redirect(
                    id,
                    &format!("Provider updated, but verification failed: {e}. Use the Test icon to retry once fixed."),
                )
                .into_response()
            }
        },
        Ok(None) => flash_redirect(id, "Provider not found.").into_response(),
        // Same race as `create` — see its own comment above `is_unique_violation`.
        Err(e) if is_unique_violation(&e) => {
            flash_redirect(id, "A provider with that label already exists.").into_response()
        }
        Err(e) => {
            tracing::error!("failed to update AI provider {}: {:?}", provider_id, e);
            flash_redirect(id, "Failed to save provider.").into_response()
        }
    }
}

fn models_error(status: StatusCode, message: impl Into<String>) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

async fn discover_with_config(site_id: Uuid, config: AiProviderConfig) -> axum::response::Response {
    match crate::translate::discover_models(site_id, &config).await {
        Ok(models) if models.is_empty() => models_error(
            StatusCode::BAD_GATEWAY,
            "The provider returned no models. You can still enter a custom model ID.",
        ),
        Ok(models) => Json(serde_json::json!({ "models": models })).into_response(),
        Err(e) => {
            tracing::warn!("AI model discovery failed: {:?}", e);
            models_error(StatusCode::BAD_GATEWAY, e.to_string())
        }
    }
}

/// POST /admin/sites/{id}/ai-providers/models — discover models using the
/// credentials currently entered in the unsaved Add Provider form.
pub async fn discover_new_models(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<AiProviderForm>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return models_error(StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG);
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(site) => site,
        Err(_) => return models_error(StatusCode::NOT_FOUND, "Site not found."),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return models_error(StatusCode::FORBIDDEN, "Forbidden");
    }
    match config_from_form(&form, None, false) {
        Ok(config) if config.requires_global_admin() && !admin.caps.is_global_admin => {
            models_error(StatusCode::FORBIDDEN, LOCAL_MODEL_FORBIDDEN_MSG)
        }
        Ok(config) => discover_with_config(id, config).await,
        Err(message) => models_error(StatusCode::BAD_REQUEST, message),
    }
}

/// POST /admin/sites/{id}/ai-providers/{provider_id}/models — discover
/// models while retaining any credential/base URL left blank in an Edit form.
pub async fn discover_saved_models(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<AiProviderForm>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return models_error(StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG);
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(site) => site,
        Err(_) => return models_error(StatusCode::NOT_FOUND, "Site not found."),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return models_error(StatusCode::FORBIDDEN, "Forbidden");
    }
    let row = match ai_provider::get_by_id(&state.db, provider_id).await {
        Ok(Some(row)) if row.site_id == id => row,
        _ => return models_error(StatusCode::NOT_FOUND, "Provider not found."),
    };
    let Some(existing) = ai_provider::decrypt_config(&state.config.secret_key, &row) else {
        return models_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to decrypt provider config.",
        );
    };
    match config_from_form(&form, Some(&existing), false) {
        Ok(config) if config.requires_global_admin() && !admin.caps.is_global_admin => {
            models_error(StatusCode::FORBIDDEN, LOCAL_MODEL_FORBIDDEN_MSG)
        }
        Ok(config) => discover_with_config(id, config).await,
        Err(message) => models_error(StatusCode::BAD_REQUEST, message),
    }
}

/// POST /admin/sites/{id}/ai-providers/{provider_id}/delete
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return (StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG).into_response();
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if let Err(e) = ai_provider::delete(&state.db, provider_id, id).await {
        tracing::error!("failed to delete AI provider {}: {:?}", provider_id, e);
    }

    Redirect::to(&format!(
        "/admin/sites/{}/settings?flash=Provider deleted.&tab=ai-translation",
        id
    ))
    .into_response()
}

/// POST /admin/sites/{id}/ai-providers/{provider_id}/test — send a trivial
/// prompt to confirm the provider is configured correctly, and mark it
/// verified on success.
pub async fn test(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    if !ai_translation_enabled(&state) {
        return (StatusCode::FORBIDDEN, AI_TRANSLATION_DISABLED_MSG).into_response();
    }
    let site = match crate::models::site::get_by_id(&state.db, id).await {
        Ok(s) => s,
        Err(_) => return Redirect::to("/admin/sites").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let row = match ai_provider::get_by_id(&state.db, provider_id).await {
        Ok(Some(row)) if row.site_id == id => row,
        _ => return flash_redirect(id, "Provider not found.").into_response(),
    };
    let Some(config) = ai_provider::decrypt_config(&state.config.secret_key, &row) else {
        return flash_redirect(id, "Failed to decrypt provider config.").into_response();
    };
    if config.requires_global_admin() && !admin.caps.is_global_admin {
        return (StatusCode::FORBIDDEN, LOCAL_MODEL_FORBIDDEN_MSG).into_response();
    }

    match crate::translate::test_provider(id, &config).await {
        Ok(()) => {
            if let Err(e) = ai_provider::mark_verified(&state.db, provider_id).await {
                tracing::error!(
                    "failed to mark AI provider {} verified: {:?}",
                    provider_id,
                    e
                );
            }
            Redirect::to(&format!(
                "/admin/sites/{}/settings?flash=Provider verified.&tab=ai-translation",
                id
            ))
            .into_response()
        }
        Err(e) => {
            tracing::error!("test call failed for AI provider {}: {:?}", provider_id, e);
            flash_redirect(id, &format!("Test failed: {e}")).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_can_change_model_without_reentering_anthropic_key() {
        let existing = AiProviderConfig::Anthropic {
            api_key: "saved-secret".to_string(),
            model_name: "claude-sonnet-old".to_string(),
        };
        let form = AiProviderForm {
            provider_type: "anthropic".to_string(),
            anthropic_model_choice: "claude-haiku-new".to_string(),
            ..Default::default()
        };
        let config = config_from_form(&form, Some(&existing), true).unwrap();
        match config {
            AiProviderConfig::Anthropic {
                api_key,
                model_name,
            } => {
                assert_eq!(api_key, "saved-secret");
                assert_eq!(model_name, "claude-haiku-new");
            }
            _ => panic!("expected Anthropic config"),
        }
    }

    #[test]
    fn blank_edit_fields_retain_openai_compatible_config() {
        let existing = AiProviderConfig::OpenaiCompatible {
            base_url: "https://example.test/v1".to_string(),
            api_key: "saved-secret".to_string(),
            model_name: "saved-model".to_string(),
        };
        let form = AiProviderForm {
            provider_type: "openai_compatible".to_string(),
            ..Default::default()
        };
        let config = config_from_form(&form, Some(&existing), true).unwrap();
        match config {
            AiProviderConfig::OpenaiCompatible {
                base_url,
                api_key,
                model_name,
            } => {
                assert_eq!(base_url, "https://example.test/v1");
                assert_eq!(api_key, "saved-secret");
                assert_eq!(model_name, "saved-model");
            }
            _ => panic!("expected OpenAI-compatible config"),
        }
    }

    #[test]
    fn custom_model_takes_precedence_over_dropdown() {
        let form = AiProviderForm {
            provider_type: "anthropic".to_string(),
            anthropic_api_key: "new-secret".to_string(),
            anthropic_model_choice: "listed-model".to_string(),
            anthropic_model_name: "custom-model".to_string(),
            ..Default::default()
        };
        let config = config_from_form(&form, None, true).unwrap();
        match config {
            AiProviderConfig::Anthropic { model_name, .. } => {
                assert_eq!(model_name, "custom-model")
            }
            _ => panic!("expected Anthropic config"),
        }
    }

    #[test]
    fn discovery_does_not_require_a_model_selection() {
        let form = AiProviderForm {
            provider_type: "anthropic".to_string(),
            anthropic_api_key: "new-secret".to_string(),
            ..Default::default()
        };
        assert!(config_from_form(&form, None, false).is_ok());
        assert!(config_from_form(&form, None, true).is_err());
    }
}
