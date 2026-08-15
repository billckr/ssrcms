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
