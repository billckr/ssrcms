//! Outbound transactional email, via any of several third-party providers.
//!
//! A site can configure any number of named provider accounts (Mailgun,
//! generic SMTP, SendGrid, Postmark) on its Settings → Email Settings tab
//! (see `models::email_provider`). Each form independently picks which one
//! (if any) to send its notify/confirm emails through — there's no single
//! "default" provider per site, since different forms may want different
//! accounts. A form with none selected falls back to the install-wide
//! Mailgun account in `AppConfig`, same as before this module supported
//! more than one provider.

use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::email_provider::{self, ProviderConfig};
use crate::models::mail_log::{self, RecordSend};

pub struct EmailMessage<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    /// The form (if any) this send was triggered by — recorded on the
    /// mail_log row so a form's analytics page can show just its own sends.
    pub form_id: Option<Uuid>,
    /// Which configured `email_providers` row to send through. `None` falls
    /// back to the install-wide Mailgun account.
    pub provider_id: Option<Uuid>,
}

/// site_settings keys for the install-wide Mailgun fallback account. Kept
/// for back-compat with `AppConfig`'s own `mailgun_*` fields — per-site
/// overrides now live in the `email_providers` table instead.
pub const SETTING_DOMAIN: &str = "mailgun_domain";
pub const SETTING_API_KEY_ENCRYPTED: &str = "mailgun_api_key_encrypted";

/// Send a transactional email on behalf of `site_id`, using the provider
/// `msg.provider_id` points at, or the install-wide Mailgun account if
/// unset. A no-op (logs a warning, returns `Ok`) when neither is
/// configured — sending mail is opt-in, not a hard requirement.
pub async fn send_for_site(state: &AppState, site_id: Uuid, msg: EmailMessage<'_>) -> anyhow::Result<()> {
    let Some(config) = resolve_provider(state, site_id, msg.provider_id).await else {
        tracing::warn!("no email provider configured for site {} — skipping email to {}", site_id, msg.to);
        return Ok(());
    };

    let sent = send_via(&config, msg.to, msg.subject, msg.text).await;

    match sent {
        Ok(message_id) => {
            tracing::info!(
                "email accepted for {} (site {}, message id: {})",
                msg.to, site_id, message_id.as_deref().unwrap_or("unknown"),
            );
            record_attempt(state, site_id, &msg, true, message_id.as_deref(), None).await;
            Ok(())
        }
        Err(e) => {
            let error = e.to_string();
            record_attempt(state, site_id, &msg, false, None, Some(&error)).await;
            Err(e)
        }
    }
}

/// Send a one-off test email through `config` — used by the "Test" button
/// on a saved-but-unverified provider. Doesn't touch `mail_log`; success
/// here is what flips a provider's `verified` flag.
pub async fn send_test_email(config: &ProviderConfig, to: &str) -> anyhow::Result<()> {
    send_via(
        config,
        to,
        "Test email from Synaptic Signals",
        "This is a test email confirming your email provider is configured correctly.",
    ).await?;
    Ok(())
}

/// Dispatches to the right provider's API/transport. Returns the
/// provider's own message id, when it gives one, for the mail_log entry.
async fn send_via(config: &ProviderConfig, to: &str, subject: &str, text: &str) -> anyhow::Result<Option<String>> {
    match config {
        ProviderConfig::Mailgun { domain, api_key } => send_via_mailgun(domain, api_key, to, subject, text).await,
        ProviderConfig::Smtp { host, port, username, password, tls_mode } =>
            send_via_smtp(host, *port, username, password, tls_mode, to, subject, text).await,
        ProviderConfig::SendGrid { api_key, from_email } => send_via_sendgrid(api_key, from_email, to, subject, text).await,
        ProviderConfig::Postmark { server_token, message_stream, from_email } =>
            send_via_postmark(server_token, message_stream, from_email, to, subject, text).await,
    }
}

async fn send_via_mailgun(domain: &str, api_key: &str, to: &str, subject: &str, text: &str) -> anyhow::Result<Option<String>> {
    // Always send From the configured domain, not some other address —
    // Mailgun sandbox domains in particular reject a From address outside
    // the sending domain.
    let from = format!("noreply@{domain}");
    let url = format!("https://api.mailgun.net/v3/{domain}/messages");
    let form = reqwest::multipart::Form::new()
        .text("from", from)
        .text("to", to.to_string())
        .text("subject", subject.to_string())
        .text("text", text.to_string());

    let resp = reqwest::Client::new()
        .post(&url)
        .basic_auth("api", Some(api_key))
        .multipart(form)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("mailgun send failed ({status}): {body}");
    }

    // Mailgun's success body is `{"id": "<message-id>", "message": "Queued. Thank you."}`.
    let body = resp.text().await.unwrap_or_default();
    let message_id = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_string));
    Ok(message_id)
}

async fn send_via_sendgrid(api_key: &str, from_email: &str, to: &str, subject: &str, text: &str) -> anyhow::Result<Option<String>> {
    let body = serde_json::json!({
        "personalizations": [{ "to": [{ "email": to }] }],
        "from": { "email": from_email },
        "subject": subject,
        "content": [{ "type": "text/plain", "value": text }],
    });

    let resp = reqwest::Client::new()
        .post("https://api.sendgrid.com/v3/mail/send")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("sendgrid send failed ({status}): {body}");
    }

    // SendGrid returns the message id in an `X-Message-Id` header, not a body.
    let message_id = resp.headers()
        .get("x-message-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    Ok(message_id)
}

async fn send_via_postmark(server_token: &str, message_stream: &str, from_email: &str, to: &str, subject: &str, text: &str) -> anyhow::Result<Option<String>> {
    let body = serde_json::json!({
        "From": from_email,
        "To": to,
        "Subject": subject,
        "TextBody": text,
        "MessageStream": message_stream,
    });

    let resp = reqwest::Client::new()
        .post("https://api.postmarkapp.com/email")
        .header("X-Postmark-Server-Token", server_token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("postmark send failed ({status}): {body}");
    }

    let body = resp.text().await.unwrap_or_default();
    let message_id = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("MessageID").and_then(|id| id.as_str()).map(str::to_string));
    Ok(message_id)
}

async fn send_via_smtp(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    tls_mode: &str,
    to: &str,
    subject: &str,
    text: &str,
) -> anyhow::Result<Option<String>> {
    use lettre::message::Message;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

    let email = Message::builder()
        .from(username.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .body(text.to_string())?;

    let creds = Credentials::new(username.to_string(), password.to_string());
    let builder = match tls_mode {
        "implicit" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
        // "starttls" (and any other value) — opportunistic/required STARTTLS,
        // the common case for port 587.
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
    };
    let mailer = builder.port(port).credentials(creds).build();

    mailer.send(email).await?;
    Ok(None)
}

/// Best-effort: a mail_log write failure shouldn't mask the send's own
/// success/failure, so this only ever logs — never returns an error to the
/// caller.
async fn record_attempt(
    state: &AppState,
    site_id: Uuid,
    msg: &EmailMessage<'_>,
    success: bool,
    provider_message_id: Option<&str>,
    error: Option<&str>,
) {
    let result = mail_log::record(&state.db, RecordSend {
        site_id,
        form_id: msg.form_id,
        to_email: msg.to,
        subject: msg.subject,
        success,
        mailgun_message_id: provider_message_id,
        error,
    }).await;
    if let Err(e) = result {
        tracing::error!("failed to write mail_log entry for site {}: {:?}", site_id, e);
    }
}

/// Resolves `provider_id` (a form's chosen `email_providers` row) into a
/// ready-to-send `ProviderConfig`, or falls back to the install-wide
/// Mailgun account in `AppConfig` when unset.
async fn resolve_provider(state: &AppState, site_id: Uuid, provider_id: Option<Uuid>) -> Option<ProviderConfig> {
    if let Some(provider_id) = provider_id {
        return match email_provider::get_by_id(&state.db, provider_id).await {
            Ok(Some(row)) if row.site_id == site_id => {
                match email_provider::decrypt_config(&state.config.secret_key, &row) {
                    Some(config) => {
                        tracing::info!("using provider '{}' ({}) for site {}", row.label, row.provider_type, site_id);
                        Some(config)
                    }
                    None => {
                        tracing::error!("failed to decrypt email_providers config for provider {}", provider_id);
                        None
                    }
                }
            }
            Ok(_) => {
                tracing::warn!("email provider {} not found (or not owned by site {})", provider_id, site_id);
                None
            }
            Err(e) => {
                tracing::error!("failed to load email provider {}: {:?}", provider_id, e);
                None
            }
        };
    }

    let (Some(api_key), Some(domain)) = (&state.config.mailgun_api_key, &state.config.mailgun_domain) else {
        return None;
    };
    tracing::info!("using install-wide mailgun account for site {} (no provider selected)", site_id);
    Some(ProviderConfig::Mailgun { domain: domain.clone(), api_key: api_key.clone() })
}
