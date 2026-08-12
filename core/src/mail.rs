//! Outbound transactional email via Mailgun's HTTP API.
//!
//! Deliberately not SMTP: talking to Mailgun over their REST API means this
//! app never touches a raw socket or holds any mail-server responsibility
//! (IP reputation, bounce/complaint handling, spam enforcement) — Mailgun
//! owns all of that.
//!
//! Each site can supply its own Mailgun account (domain + API key, set on
//! that site's Settings page) so client sites aren't forced to share the
//! operator's sending domain, reputation, or bill. A site that hasn't
//! configured its own falls back to the install-wide `mailgun_*` config in
//! `AppConfig`.

use uuid::Uuid;

use crate::app_state::{get_site_setting, AppState};
use crate::config::AppConfig;
use crate::models::mail_log::{self, RecordSend};

pub struct EmailMessage<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    /// The form (if any) this send was triggered by — recorded on the
    /// mail_log row so a form's analytics page can show just its own sends.
    pub form_id: Option<Uuid>,
}

/// site_settings keys for a site's own Mailgun account. The API key is
/// stored encrypted (see `crypto`); the domain isn't a secret.
pub const SETTING_DOMAIN: &str = "mailgun_domain";
pub const SETTING_API_KEY_ENCRYPTED: &str = "mailgun_api_key_encrypted";

struct MailgunCreds {
    api_key: String,
    domain: String,
    base_url: String,
    from: String,
}

/// Send a transactional email on behalf of `site_id`, using that site's own
/// Mailgun account if configured, otherwise the install-wide account. A
/// no-op (logs a warning, returns `Ok`) when neither is configured — sending
/// mail is opt-in, not a hard requirement.
pub async fn send_for_site(state: &AppState, site_id: Uuid, msg: EmailMessage<'_>) -> anyhow::Result<()> {
    let Some(creds) = resolve_creds(state, site_id).await else {
        tracing::warn!("mailgun not configured for site {} — skipping email to {}", site_id, msg.to);
        return Ok(());
    };

    let url = format!("{}/{}/messages", creds.base_url, creds.domain);
    let form = reqwest::multipart::Form::new()
        .text("from", creds.from)
        .text("to", msg.to.to_string())
        .text("subject", msg.subject.to_string())
        .text("text", msg.text.to_string());

    let sent = reqwest::Client::new()
        .post(&url)
        .basic_auth("api", Some(&creds.api_key))
        .multipart(form)
        .send()
        .await;

    let resp = match sent {
        Ok(resp) => resp,
        Err(e) => {
            let error = format!("request to mailgun failed: {e}");
            record_attempt(state, site_id, &msg, false, None, Some(&error)).await;
            return Err(e.into());
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let error = format!("mailgun send failed ({status}): {body}");
        record_attempt(state, site_id, &msg, false, None, Some(&error)).await;
        anyhow::bail!(error);
    }

    // Mailgun's success body is `{"id": "<message-id>", "message": "Queued. Thank you."}`.
    // The id is worth logging: it's the key to look up this exact send in
    // Mailgun's own dashboard/logs if the recipient later says they never got it.
    let body = resp.text().await.unwrap_or_default();
    let message_id = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_string));
    tracing::info!(
        "mailgun accepted email to {} for site {} (message id: {})",
        msg.to,
        site_id,
        message_id.as_deref().unwrap_or("unknown"),
    );
    record_attempt(state, site_id, &msg, true, message_id.as_deref(), None).await;

    Ok(())
}

/// Best-effort: a mail_log write failure shouldn't mask the send's own
/// success/failure, so this only ever logs — never returns an error to the
/// caller.
async fn record_attempt(
    state: &AppState,
    site_id: Uuid,
    msg: &EmailMessage<'_>,
    success: bool,
    mailgun_message_id: Option<&str>,
    error: Option<&str>,
) {
    let result = mail_log::record(&state.db, RecordSend {
        site_id,
        form_id: msg.form_id,
        to_email: msg.to,
        subject: msg.subject,
        success,
        mailgun_message_id,
        error,
    }).await;
    if let Err(e) = result {
        tracing::error!("failed to write mail_log entry for site {}: {:?}", site_id, e);
    }
}

async fn resolve_creds(state: &AppState, site_id: Uuid) -> Option<MailgunCreds> {
    let domain = get_site_setting(&state.db, site_id, SETTING_DOMAIN).await;
    let encrypted_key = get_site_setting(&state.db, site_id, SETTING_API_KEY_ENCRYPTED).await;
    if let (Some(domain), Some(encrypted_key)) = (domain, encrypted_key) {
        match crate::crypto::decrypt(&state.config.secret_key, &encrypted_key) {
            Some(api_key) => {
                // Always send From the site's own domain here, not the
                // operator's global smtp_from_email — this is the client's
                // own Mailgun account, and Mailgun sandbox domains in
                // particular reject a From address outside the sending domain.
                tracing::info!("using site-specific mailgun account for site {} (domain {})", site_id, domain);
                return Some(MailgunCreds {
                    api_key,
                    from: format!("noreply@{domain}"),
                    base_url: state.config.mailgun_base_url.clone(),
                    domain,
                });
            }
            None => tracing::error!("failed to decrypt mailgun_api_key_encrypted for site {}", site_id),
        }
    }

    let (Some(api_key), Some(domain)) = (&state.config.mailgun_api_key, &state.config.mailgun_domain) else {
        return None;
    };
    tracing::info!("using install-wide mailgun account for site {} (no site-specific account set)", site_id);
    Some(MailgunCreds {
        api_key: api_key.clone(),
        domain: domain.clone(),
        base_url: state.config.mailgun_base_url.clone(),
        from: default_from(&state.config, domain),
    })
}

fn default_from(config: &AppConfig, domain: &str) -> String {
    match (&config.smtp_from_name, &config.smtp_from_email) {
        (Some(name), Some(email)) => format!("{name} <{email}>"),
        (None, Some(email)) => email.clone(),
        _ => format!("noreply@{domain}"),
    }
}
