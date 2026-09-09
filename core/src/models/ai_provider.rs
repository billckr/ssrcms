//! A site's configured AI translation provider credentials (Anthropic, or
//! any OpenAI-compatible endpoint — covers OpenAI itself, Ollama, LM Studio,
//! vLLM, or any other self-hosted server speaking that same wire format).
//! Mirrors `email_provider.rs`'s storage shape exactly. See
//! `crate::translate` for the code that actually calls out to these.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

/// Credentials for one AI provider. Serialized to JSON and encrypted at rest
/// (`config_encrypted` column) via `crypto::encrypt`/`decrypt` — the same
/// mechanism `email_provider::ProviderConfig` uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider_type", rename_all = "snake_case")]
pub enum AiProviderConfig {
    Anthropic {
        api_key: String,
        model_name: String,
    },
    OpenaiCompatible {
        base_url: String,
        api_key: String,
        model_name: String,
    },
}

impl AiProviderConfig {
    pub fn provider_type(&self) -> &'static str {
        match self {
            AiProviderConfig::Anthropic { .. } => "anthropic",
            AiProviderConfig::OpenaiCompatible { .. } => "openai_compatible",
        }
    }

    /// Per-field placeholder text for a provider's Edit form — non-secret
    /// fields (model name, base URL) show their real saved value; the API
    /// key shows the same masked form `display_hint` uses. Field names match
    /// the admin form's.
    pub fn field_placeholders(&self) -> Vec<(&'static str, String)> {
        match self {
            AiProviderConfig::Anthropic {
                api_key,
                model_name,
            } => vec![
                ("anthropic_model_name", model_name.clone()),
                ("anthropic_api_key", crate::crypto::mask_secret(api_key)),
            ],
            AiProviderConfig::OpenaiCompatible {
                base_url,
                api_key,
                model_name,
            } => vec![
                ("openai_compatible_base_url", base_url.clone()),
                ("openai_compatible_model_name", model_name.clone()),
                (
                    "openai_compatible_api_key",
                    crate::crypto::mask_secret(api_key),
                ),
            ],
        }
    }

    /// A short, non-sensitive identifying string for the provider list.
    pub fn display_hint(&self) -> String {
        match self {
            AiProviderConfig::Anthropic {
                api_key,
                model_name,
            } => {
                format!("{} · {}", model_name, crate::crypto::mask_secret(api_key))
            }
            AiProviderConfig::OpenaiCompatible {
                base_url,
                model_name,
                ..
            } => {
                format!("{} · {}", model_name, base_url)
            }
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AiProviderRow {
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
pub fn decrypt_config(secret_key: &str, row: &AiProviderRow) -> Option<AiProviderConfig> {
    let json = crate::crypto::decrypt(secret_key, &row.config_encrypted)?;
    serde_json::from_str(&json).ok()
}

pub fn encrypt_config(secret_key: &str, config: &AiProviderConfig) -> String {
    let json = serde_json::to_string(config).unwrap_or_default();
    crate::crypto::encrypt(secret_key, &json)
}

/// Every AI provider configured for a site, most recently created first.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<AiProviderRow>> {
    let rows = sqlx::query_as::<_, AiProviderRow>(
        "SELECT * FROM ai_providers WHERE site_id = $1 ORDER BY created_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Only the providers that have passed a test call — what the post editor's
/// Translate dropdown should offer, since an unverified one is likely
/// misconfigured.
pub async fn list_verified_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<AiProviderRow>> {
    let rows = sqlx::query_as::<_, AiProviderRow>(
        "SELECT * FROM ai_providers WHERE site_id = $1 AND verified = TRUE ORDER BY created_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Whether another provider on this site already uses `label` (case-
/// insensitive). Pass `exclude_id` when checking during an update so the
/// row being edited doesn't collide with itself.
pub async fn label_exists_for_site(
    pool: &PgPool,
    site_id: Uuid,
    label: &str,
    exclude_id: Option<Uuid>,
) -> Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM ai_providers WHERE site_id = $1 AND LOWER(label) = LOWER($2) AND ($3::uuid IS NULL OR id != $3)",
    )
    .bind(site_id)
    .bind(label)
    .bind(exclude_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(c,)| c > 0).unwrap_or(false))
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<AiProviderRow>> {
    let row = sqlx::query_as::<_, AiProviderRow>("SELECT * FROM ai_providers WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create(
    pool: &PgPool,
    site_id: Uuid,
    label: &str,
    config: &AiProviderConfig,
    secret_key: &str,
) -> Result<AiProviderRow> {
    let encrypted = encrypt_config(secret_key, config);
    let row = sqlx::query_as::<_, AiProviderRow>(
        "INSERT INTO ai_providers (site_id, provider_type, label, config_encrypted)
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
/// credential change — a new key/model pair hasn't been proven to work yet.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    site_id: Uuid,
    label: &str,
    config: &AiProviderConfig,
    secret_key: &str,
) -> Result<Option<AiProviderRow>> {
    let encrypted = encrypt_config(secret_key, config);
    let row = sqlx::query_as::<_, AiProviderRow>(
        "UPDATE ai_providers SET label = $1, config_encrypted = $2, verified = FALSE, updated_at = NOW()
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
    sqlx::query("UPDATE ai_providers SET verified = TRUE, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM ai_providers WHERE id = $1 AND site_id = $2")
        .bind(id)
        .bind(site_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anthropic_config() -> AiProviderConfig {
        AiProviderConfig::Anthropic {
            api_key: "sk-ant-api03-abcdefghijklmnopqrstuvwxyz".to_string(),
            model_name: "claude-sonnet-5".to_string(),
        }
    }

    fn openai_compatible_config() -> AiProviderConfig {
        AiProviderConfig::OpenaiCompatible {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: "".to_string(),
            model_name: "llama3.1".to_string(),
        }
    }

    #[test]
    fn provider_type_matches_variant() {
        assert_eq!(anthropic_config().provider_type(), "anthropic");
        assert_eq!(
            openai_compatible_config().provider_type(),
            "openai_compatible"
        );
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = "test-secret-key";
        let config = anthropic_config();
        let encrypted = encrypt_config(key, &config);
        let row = AiProviderRow {
            id: Uuid::new_v4(),
            site_id: Uuid::new_v4(),
            provider_type: config.provider_type().to_string(),
            label: "Test".to_string(),
            config_encrypted: encrypted,
            verified: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let decrypted = decrypt_config(key, &row).expect("should decrypt");
        match decrypted {
            AiProviderConfig::Anthropic {
                api_key,
                model_name,
            } => {
                assert_eq!(api_key, "sk-ant-api03-abcdefghijklmnopqrstuvwxyz");
                assert_eq!(model_name, "claude-sonnet-5");
            }
            _ => panic!("expected Anthropic variant"),
        }
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let config = anthropic_config();
        let encrypted = encrypt_config("right-key", &config);
        let row = AiProviderRow {
            id: Uuid::new_v4(),
            site_id: Uuid::new_v4(),
            provider_type: config.provider_type().to_string(),
            label: "Test".to_string(),
            config_encrypted: encrypted,
            verified: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(decrypt_config("wrong-key", &row).is_none());
    }

    #[test]
    fn field_placeholders_mask_api_key_only() {
        let placeholders = anthropic_config().field_placeholders();
        let api_key_placeholder = placeholders
            .iter()
            .find(|(name, _)| *name == "anthropic_api_key")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_ne!(
            api_key_placeholder,
            "sk-ant-api03-abcdefghijklmnopqrstuvwxyz"
        );

        let model_placeholder = placeholders
            .iter()
            .find(|(name, _)| *name == "anthropic_model_name")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(model_placeholder, "claude-sonnet-5");
    }

    #[test]
    fn display_hint_never_contains_full_api_key() {
        let hint = anthropic_config().display_hint();
        assert!(!hint.contains("sk-ant-api03-abcdefghijklmnopqrstuvwxyz"));
    }
}
