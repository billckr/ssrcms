//! Admin handlers for a site's configured AI translation providers (AI
//! Translation tab on Site Settings). Mirrors `email_providers.rs` exactly.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
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
    pub openai_compatible_base_url: String,
    #[serde(default)]
    pub openai_compatible_api_key: String,
    #[serde(default)]
    pub openai_compatible_model_name: String,
}

/// Builds the right `AiProviderConfig` variant from whichever fields matter
/// for `form.provider_type`, ignoring the rest (the other provider type's
/// fields are hidden but still submitted, since it's all one `<form>`).
fn config_from_form(form: &AiProviderForm) -> Result<AiProviderConfig, &'static str> {
    match form.provider_type.as_str() {
        "anthropic" => {
            if form.anthropic_api_key.trim().is_empty()
                || form.anthropic_model_name.trim().is_empty()
            {
                return Err("Enter both an API key and a model name.");
            }
            Ok(AiProviderConfig::Anthropic {
                api_key: form.anthropic_api_key.trim().to_string(),
                model_name: form.anthropic_model_name.trim().to_string(),
            })
        }
        "openai_compatible" => {
            if form.openai_compatible_base_url.trim().is_empty()
                || form.openai_compatible_model_name.trim().is_empty()
            {
                return Err("Enter both a base URL and a model name.");
            }
            Ok(AiProviderConfig::OpenaiCompatible {
                base_url: form.openai_compatible_base_url.trim().to_string(),
                api_key: form.openai_compatible_api_key.trim().to_string(),
                model_name: form.openai_compatible_model_name.trim().to_string(),
            })
        }
        _ => Err("Unknown provider type."),
    }
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
    let config = match config_from_form(&form) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };

    if let Err(e) = ai_provider::create(
        &state.db,
        id,
        form.label.trim(),
        &config,
        &state.config.secret_key,
    )
    .await
    {
        tracing::error!("failed to create AI provider for site {}: {:?}", id, e);
        return flash_redirect(id, "Failed to save provider.").into_response();
    }

    Redirect::to(&format!(
        "/admin/sites/{}/settings?flash=Provider added. Test it to verify it works.&tab=ai-translation",
        id
    ))
    .into_response()
}

/// POST /admin/sites/{id}/ai-providers/{provider_id} — update an existing
/// provider's label/credentials. A full overwrite, same shape as create —
/// credentials are never sent back to the browser to prefill, so the edit
/// form re-collects every field. Resets `verified` to false (see
/// `ai_provider::update`), since the new credentials haven't been proven to
/// work yet.
pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<AiProviderForm>,
) -> impl IntoResponse {
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
    let config = match config_from_form(&form) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };

    match ai_provider::update(&state.db, provider_id, id, form.label.trim(), &config, &state.config.secret_key).await {
        Ok(Some(_)) => Redirect::to(&format!("/admin/sites/{}/settings?flash=Provider updated. Test it to re-verify it.&tab=ai-translation", id)).into_response(),
        Ok(None) => flash_redirect(id, "Provider not found.").into_response(),
        Err(e) => {
            tracing::error!("failed to update AI provider {}: {:?}", provider_id, e);
            flash_redirect(id, "Failed to save provider.").into_response()
        }
    }
}

/// POST /admin/sites/{id}/ai-providers/{provider_id}/delete
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
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

    match crate::translate::test_provider(&config).await {
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
