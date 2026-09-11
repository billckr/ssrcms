//! Locale-specific presentation text for reusable Form Designer and Poll
//! Designer definitions. Stable submission/vote identifiers stay on the
//! source definition; only visitor-facing labels and messages live here.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormTranslationPayload {
    /// Field name -> translated label. Field names themselves never change.
    pub field_labels: BTreeMap<String, String>,
    /// Field name -> (stable option value -> translated option label).
    pub option_labels: BTreeMap<String, BTreeMap<String, String>>,
    pub button_label: String,
    pub success_message: String,
    pub invalid_message: String,
    pub confirm_subject: String,
    pub confirm_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollTranslationPayload {
    pub question: String,
    /// Stable option key -> translated option label.
    pub option_labels: BTreeMap<String, String>,
    pub button_label: String,
    pub success_message: String,
    /// Must contain `{count}` exactly once; replaced client-side after tally fetch.
    pub total_votes_label: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FormTranslationRow {
    pub id: Uuid,
    pub form_id: Uuid,
    pub locale: String,
    pub payload: serde_json::Value,
    pub source_updated_at: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FormTranslationRow {
    pub fn parsed(&self) -> Option<FormTranslationPayload> {
        serde_json::from_value(self.payload.clone()).ok()
    }

    pub fn is_stale(&self, source_updated_at: DateTime<Utc>) -> bool {
        self.source_updated_at < source_updated_at
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PollTranslationRow {
    pub id: Uuid,
    pub poll_id: Uuid,
    pub locale: String,
    pub payload: serde_json::Value,
    pub source_updated_at: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PollTranslationRow {
    pub fn parsed(&self) -> Option<PollTranslationPayload> {
        serde_json::from_value(self.payload.clone()).ok()
    }

    pub fn is_stale(&self, source_updated_at: DateTime<Utc>) -> bool {
        self.source_updated_at < source_updated_at
    }
}

pub async fn list_for_form(pool: &PgPool, form_id: Uuid) -> Result<Vec<FormTranslationRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM form_translations WHERE form_id = $1 ORDER BY locale")
            .bind(form_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn list_for_poll(pool: &PgPool, poll_id: Uuid) -> Result<Vec<PollTranslationRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM poll_translations WHERE poll_id = $1 ORDER BY locale")
            .bind(poll_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get_form(
    pool: &PgPool,
    form_id: Uuid,
    locale: &str,
) -> Result<Option<FormTranslationRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM form_translations WHERE form_id = $1 AND locale = $2")
            .bind(form_id)
            .bind(locale)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn get_poll(
    pool: &PgPool,
    poll_id: Uuid,
    locale: &str,
) -> Result<Option<PollTranslationRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM poll_translations WHERE poll_id = $1 AND locale = $2")
            .bind(poll_id)
            .bind(locale)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn upsert_form_on(
    conn: &mut PgConnection,
    form_id: Uuid,
    locale: &str,
    payload: &FormTranslationPayload,
    source_updated_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO form_translations (form_id, locale, payload, source_updated_at, generated_at)
         VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (form_id, locale) DO UPDATE SET
           payload = EXCLUDED.payload,
           source_updated_at = EXCLUDED.source_updated_at,
           generated_at = NOW(),
           updated_at = NOW()",
    )
    .bind(form_id)
    .bind(locale)
    .bind(serde_json::to_value(payload).unwrap_or_default())
    .bind(source_updated_at)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn upsert_poll_on(
    conn: &mut PgConnection,
    poll_id: Uuid,
    locale: &str,
    payload: &PollTranslationPayload,
    source_updated_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO poll_translations (poll_id, locale, payload, source_updated_at, generated_at)
         VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (poll_id, locale) DO UPDATE SET
           payload = EXCLUDED.payload,
           source_updated_at = EXCLUDED.source_updated_at,
           generated_at = NOW(),
           updated_at = NOW()",
    )
    .bind(poll_id)
    .bind(locale)
    .bind(serde_json::to_value(payload).unwrap_or_default())
    .bind(source_updated_at)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn upsert_form(
    pool: &PgPool,
    form_id: Uuid,
    locale: &str,
    payload: &FormTranslationPayload,
    source_updated_at: DateTime<Utc>,
) -> Result<()> {
    let mut conn = pool.acquire().await?;
    upsert_form_on(&mut conn, form_id, locale, payload, source_updated_at).await
}

pub async fn upsert_poll(
    pool: &PgPool,
    poll_id: Uuid,
    locale: &str,
    payload: &PollTranslationPayload,
    source_updated_at: DateTime<Utc>,
) -> Result<()> {
    let mut conn = pool.acquire().await?;
    upsert_poll_on(&mut conn, poll_id, locale, payload, source_updated_at).await
}

pub async fn delete_form(pool: &PgPool, form_id: Uuid, locale: &str) -> Result<()> {
    sqlx::query("DELETE FROM form_translations WHERE form_id = $1 AND locale = $2")
        .bind(form_id)
        .bind(locale)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_poll(pool: &PgPool, poll_id: Uuid, locale: &str) -> Result<()> {
    sqlx::query("DELETE FROM poll_translations WHERE poll_id = $1 AND locale = $2")
        .bind(poll_id)
        .bind(locale)
        .execute(pool)
        .await?;
    Ok(())
}
