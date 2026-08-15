//! A site's configured third-party email accounts (Mailgun, SMTP, SendGrid,
//! Postmark). Distinct from `mail_log` — this owns provider *credentials*,
//! not send history. See `crate::mail` for the code that actually sends
//! through these.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

/// Credentials for one provider type. Serialized to JSON and encrypted at
/// rest (`config_encrypted` column) via `crypto::encrypt`/`decrypt`, the
/// same mechanism the old single-Mailgun-account field used.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider_type", rename_all = "snake_case")]
pub enum ProviderConfig {
    Mailgun { domain: String, api_key: String },
    Smtp { host: String, port: u16, username: String, password: String, tls_mode: String },
    SendGrid { api_key: String, from_email: String },
    Postmark { server_token: String, message_stream: String, from_email: String },
}

impl ProviderConfig {
    pub fn provider_type(&self) -> &'static str {
        match self {
            ProviderConfig::Mailgun { .. } => "mailgun",
            ProviderConfig::Smtp { .. } => "smtp",
            ProviderConfig::SendGrid { .. } => "sendgrid",
            ProviderConfig::Postmark { .. } => "postmark",
        }
    }

    /// Per-field placeholder text for a provider's Edit form — non-secret
    /// fields (domain, host, username, from address, ...) show their real
    /// saved value so the admin can see what's configured without it being
    /// pre-filled into the input (still a full overwrite on save); secret
    /// fields (API keys, passwords, tokens) show the same masked form
    /// `display_hint` uses. Field names match `ProviderForm`'s.
    pub fn field_placeholders(&self) -> Vec<(&'static str, String)> {
        match self {
            ProviderConfig::Mailgun { domain, api_key } => vec![
                ("mailgun_domain", domain.clone()),
                ("mailgun_api_key", mask_secret(api_key)),
            ],
            ProviderConfig::Smtp { host, port, username, password, .. } => vec![
                ("smtp_host", host.clone()),
                ("smtp_port", port.to_string()),
                ("smtp_username", username.clone()),
                ("smtp_password", mask_secret(password)),
            ],
            ProviderConfig::SendGrid { api_key, from_email } => vec![
                ("sendgrid_from_email", from_email.clone()),
                ("sendgrid_api_key", mask_secret(api_key)),
            ],
            ProviderConfig::Postmark { server_token, message_stream, from_email } => vec![
                ("postmark_from_email", from_email.clone()),
                ("postmark_message_stream", message_stream.clone()),
                ("postmark_server_token", mask_secret(server_token)),
            ],
        }
    }

    /// A short, non-sensitive identifying string for the provider list —
    /// enough for an admin to tell configured providers of the same type
    /// apart (e.g. two Mailgun accounts) without ever showing a full secret.
    pub fn display_hint(&self) -> String {
        match self {
            ProviderConfig::Mailgun { domain, api_key } => {
                format!("{} · {}", domain, mask_secret(api_key))
            }
            ProviderConfig::Smtp { host, port, username, .. } => {
                format!("{}@{}:{}", username, host, port)
            }
            ProviderConfig::SendGrid { api_key, from_email } => {
                format!("{} · {}", from_email, mask_secret(api_key))
            }
            ProviderConfig::Postmark { server_token, from_email, .. } => {
                format!("{} · {}", from_email, mask_secret(server_token))
            }
        }
    }
}

/// Masks a secret for display: dash-delimited keys (Mailgun-style) show
/// only their last two segments, e.g. `966c...280-11c539c0-c7ddc18d` becomes
/// `11c539c0-c7ddc18d` — the same trailing portion Mailgun's own dashboard
/// shows. Anything else shows only its last 8 characters.
fn mask_secret(s: &str) -> String {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() >= 3 {
        format!("{}-{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else if s.chars().count() > 8 {
        let tail: String = s.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
        format!("...{}", tail)
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EmailProviderRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub provider_type: String,
    pub label: String,
    pub config_encrypted: String,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Decrypt and parse a row's stored config. `None` on a bad key or corrupt
/// data (shouldn't happen absent a `SECRET_KEY` rotation, but defensive).
pub fn decrypt_config(secret_key: &str, row: &EmailProviderRow) -> Option<ProviderConfig> {
    let json = crate::crypto::decrypt(secret_key, &row.config_encrypted)?;
    serde_json::from_str(&json).ok()
}

pub fn encrypt_config(secret_key: &str, config: &ProviderConfig) -> String {
    let json = serde_json::to_string(config).unwrap_or_default();
    crate::crypto::encrypt(secret_key, &json)
}

/// Every provider configured for a site, most recently created first.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<EmailProviderRow>> {
    let rows = sqlx::query_as::<_, EmailProviderRow>(
        "SELECT * FROM email_providers WHERE site_id = $1 ORDER BY created_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Only the providers that have passed a test send — what a form's "Send
/// via" dropdown should offer, since an unverified one is likely
/// misconfigured.
pub async fn list_verified_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<EmailProviderRow>> {
    let rows = sqlx::query_as::<_, EmailProviderRow>(
        "SELECT * FROM email_providers WHERE site_id = $1 AND verified = TRUE ORDER BY created_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Whether another provider on this site already uses `label` (case-
/// insensitive). Pass `exclude_id` when checking during an update so the
/// row being edited doesn't collide with itself.
pub async fn label_exists_for_site(pool: &PgPool, site_id: Uuid, label: &str, exclude_id: Option<Uuid>) -> Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM email_providers WHERE site_id = $1 AND LOWER(label) = LOWER($2) AND ($3::uuid IS NULL OR id != $3)",
    )
    .bind(site_id)
    .bind(label)
    .bind(exclude_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(c,)| c > 0).unwrap_or(false))
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<EmailProviderRow>> {
    let row = sqlx::query_as::<_, EmailProviderRow>("SELECT * FROM email_providers WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create(pool: &PgPool, site_id: Uuid, label: &str, config: &ProviderConfig, secret_key: &str) -> Result<EmailProviderRow> {
    let encrypted = encrypt_config(secret_key, config);
    let row = sqlx::query_as::<_, EmailProviderRow>(
        "INSERT INTO email_providers (site_id, provider_type, label, config_encrypted)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(site_id)
    .bind(config.provider_type())
    .bind(label)
    .bind(&encrypted)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Updates label and/or credentials. Resets `verified` to false on a
/// credential change — a new key/host pair hasn't been proven to work yet.
pub async fn update(pool: &PgPool, id: Uuid, site_id: Uuid, label: &str, config: &ProviderConfig, secret_key: &str) -> Result<Option<EmailProviderRow>> {
    let encrypted = encrypt_config(secret_key, config);
    let row = sqlx::query_as::<_, EmailProviderRow>(
        "UPDATE email_providers SET label = $1, config_encrypted = $2, verified = FALSE, updated_at = NOW()
         WHERE id = $3 AND site_id = $4
         RETURNING *",
    )
    .bind(label)
    .bind(&encrypted)
    .bind(id)
    .bind(site_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn mark_verified(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("UPDATE email_providers SET verified = TRUE, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM email_providers WHERE id = $1 AND site_id = $2")
        .bind(id)
        .bind(site_id)
        .execute(pool)
        .await?;
    Ok(())
}
