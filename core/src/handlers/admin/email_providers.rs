//! Admin handlers for a site's configured email providers (Email Settings
//! tab on Site Settings). Distinct from `admin::sites` — that owns the site
//! record itself; this owns the `email_providers` rows attached to it.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::email_provider::{self, ProviderConfig};

use super::sites::require_site_manager;

#[derive(Deserialize, Default)]
pub struct ProviderForm {
    pub label: String,
    pub provider_type: String,
    #[serde(default)]
    pub mailgun_domain: String,
    #[serde(default)]
    pub mailgun_api_key: String,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default)]
    pub smtp_port: String,
    #[serde(default)]
    pub smtp_username: String,
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default)]
    pub smtp_tls_mode: String,
    #[serde(default)]
    pub sendgrid_api_key: String,
    #[serde(default)]
    pub sendgrid_from_email: String,
    #[serde(default)]
    pub postmark_server_token: String,
    #[serde(default)]
    pub postmark_message_stream: String,
    #[serde(default)]
    pub postmark_from_email: String,
}

/// Builds the right `ProviderConfig` variant from whichever fields matter
/// for `form.provider_type`, ignoring the rest (the other providers'
/// fields are hidden but still submitted, since it's all one `<form>`).
fn config_from_form(form: &ProviderForm) -> Result<ProviderConfig, &'static str> {
    match form.provider_type.as_str() {
        "mailgun" => {
            if form.mailgun_domain.trim().is_empty() || form.mailgun_api_key.trim().is_empty() {
                return Err("Enter both a domain and a sending key.");
            }
            Ok(ProviderConfig::Mailgun {
                domain: form.mailgun_domain.trim().to_string(),
                api_key: form.mailgun_api_key.trim().to_string(),
            })
        }
        "smtp" => {
            let port: u16 = form.smtp_port.trim().parse().map_err(|_| "Enter a valid port number.")?;
            if form.smtp_host.trim().is_empty() {
                return Err("Enter a host.");
            }
            Ok(ProviderConfig::Smtp {
                host: form.smtp_host.trim().to_string(),
                port,
                username: form.smtp_username.trim().to_string(),
                password: form.smtp_password.trim().to_string(),
                tls_mode: if form.smtp_tls_mode.trim().is_empty() { "starttls".to_string() } else { form.smtp_tls_mode.trim().to_string() },
            })
        }
        "sendgrid" => {
            if form.sendgrid_api_key.trim().is_empty() || form.sendgrid_from_email.trim().is_empty() {
                return Err("Enter both an API key and a from address.");
            }
            Ok(ProviderConfig::SendGrid {
                api_key: form.sendgrid_api_key.trim().to_string(),
                from_email: form.sendgrid_from_email.trim().to_string(),
            })
        }
        "postmark" => {
            if form.postmark_server_token.trim().is_empty() || form.postmark_from_email.trim().is_empty() {
                return Err("Enter both a server token and a from address.");
            }
            Ok(ProviderConfig::Postmark {
                server_token: form.postmark_server_token.trim().to_string(),
                message_stream: if form.postmark_message_stream.trim().is_empty() { "outbound".to_string() } else { form.postmark_message_stream.trim().to_string() },
                from_email: form.postmark_from_email.trim().to_string(),
            })
        }
        _ => Err("Unknown provider type."),
    }
}

fn flash_redirect(site_id: Uuid, msg: &str) -> Redirect {
    let msg = crate::handlers::admin::themes::url_encode_param(msg);
    Redirect::to(&format!("/admin/sites/{}/settings?flash={}&tab=email", site_id, msg))
}

/// POST /admin/sites/{id}/email-providers — add a new provider.
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<ProviderForm>,
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
    match email_provider::label_exists_for_site(&state.db, id, form.label.trim(), None).await {
        Ok(true) => return flash_redirect(id, "A provider with that label already exists.").into_response(),
        Ok(false) => {}
        Err(e) => {
            tracing::error!("failed to check email provider label uniqueness for site {}: {:?}", id, e);
            return flash_redirect(id, "Failed to save provider.").into_response();
        }
    }
    let config = match config_from_form(&form) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };

    if let Err(e) = email_provider::create(&state.db, id, form.label.trim(), &config, &state.config.secret_key).await {
        tracing::error!("failed to create email provider for site {}: {:?}", id, e);
        return flash_redirect(id, "Failed to save provider.").into_response();
    }

    Redirect::to(&format!("/admin/sites/{}/settings?flash=Provider added. Send a test email to verify it.&tab=email", id)).into_response()
}

/// POST /admin/sites/{id}/email-providers/{provider_id} — update an
/// existing provider's label/credentials. A full overwrite, same shape as
/// create — credentials are never sent back to the browser to prefill, so
/// the edit form re-collects every field. Resets `verified` to false (see
/// `email_provider::update`), since the new credentials haven't been proven
/// to work yet.
pub async fn update(
    State(state): State<AppState>,
    admin: AdminUser,
    Path((id, provider_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<ProviderForm>,
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
    match email_provider::label_exists_for_site(&state.db, id, form.label.trim(), Some(provider_id)).await {
        Ok(true) => return flash_redirect(id, "A provider with that label already exists.").into_response(),
        Ok(false) => {}
        Err(e) => {
            tracing::error!("failed to check email provider label uniqueness for site {}: {:?}", id, e);
            return flash_redirect(id, "Failed to save provider.").into_response();
        }
    }
    let config = match config_from_form(&form) {
        Ok(c) => c,
        Err(msg) => return flash_redirect(id, msg).into_response(),
    };

    match email_provider::update(&state.db, provider_id, id, form.label.trim(), &config, &state.config.secret_key).await {
        Ok(Some(_)) => Redirect::to(&format!("/admin/sites/{}/settings?flash=Provider updated. Send a test email to re-verify it.&tab=email", id)).into_response(),
        Ok(None) => flash_redirect(id, "Provider not found.").into_response(),
        Err(e) => {
            tracing::error!("failed to update email provider {}: {:?}", provider_id, e);
            flash_redirect(id, "Failed to save provider.").into_response()
        }
    }
}

/// POST /admin/sites/{id}/email-providers/{provider_id}/delete
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

    if let Err(e) = email_provider::delete(&state.db, provider_id, id).await {
        tracing::error!("failed to delete email provider {}: {:?}", provider_id, e);
    }

    Redirect::to(&format!("/admin/sites/{}/settings?flash=Provider deleted.&tab=email", id)).into_response()
}

/// POST /admin/sites/{id}/email-providers/{provider_id}/test — send a test
/// email to the requesting admin's own address, and mark the provider
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

    let row = match email_provider::get_by_id(&state.db, provider_id).await {
        Ok(Some(row)) if row.site_id == id => row,
        _ => return flash_redirect(id, "Provider not found.").into_response(),
    };
    let Some(config) = email_provider::decrypt_config(&state.config.secret_key, &row) else {
        return flash_redirect(id, "Failed to decrypt provider config.").into_response();
    };

    match crate::mail::send_test_email(&config, &admin.user.email).await {
        Ok(()) => {
            if let Err(e) = email_provider::mark_verified(&state.db, provider_id).await {
                tracing::error!("failed to mark email provider {} verified: {:?}", provider_id, e);
            }
            Redirect::to(&format!("/admin/sites/{}/settings?flash=Test email sent to {} — provider verified.&tab=email", id, admin.user.email)).into_response()
        }
        Err(e) => {
            tracing::error!("test email failed for provider {}: {:?}", provider_id, e);
            flash_redirect(id, &format!("Test email failed: {e}")).into_response()
        }
    }
}
