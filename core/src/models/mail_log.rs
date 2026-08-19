//! Persistent record of outbound Mailgun send attempts. Distinct from the
//! tracing log file — this survives log rotation and is queryable, meant to
//! back an admin "Email Log" screen and answer "did we even try to send
//! that?" long after the fact.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MailLogEntry {
    pub id: Uuid,
    pub site_id: Uuid,
    pub form_id: Option<Uuid>,
    pub to_email: String,
    pub subject: String,
    pub success: bool,
    pub mailgun_message_id: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct RecordSend<'a> {
    pub site_id: Uuid,
    pub form_id: Option<Uuid>,
    pub to_email: &'a str,
    pub subject: &'a str,
    pub success: bool,
    pub mailgun_message_id: Option<&'a str>,
    pub error: Option<&'a str>,
}

pub async fn record(pool: &PgPool, input: RecordSend<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO mail_log (site_id, form_id, to_email, subject, success, mailgun_message_id, error)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(input.site_id)
    .bind(input.form_id)
    .bind(input.to_email)
    .bind(input.subject)
    .bind(input.success)
    .bind(input.mailgun_message_id)
    .bind(input.error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Most recent sends for a site, newest first.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid, limit: i64) -> Result<Vec<MailLogEntry>> {
    let rows = sqlx::query_as::<_, MailLogEntry>(
        "SELECT * FROM mail_log WHERE site_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(site_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Most recent sends tied to a specific form, newest first.
pub async fn list_for_form(pool: &PgPool, form_id: Uuid, limit: i64) -> Result<Vec<MailLogEntry>> {
    let rows = sqlx::query_as::<_, MailLogEntry>(
        "SELECT * FROM mail_log WHERE form_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(form_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Finds sends to a given email on a site — used by GDPR erasure to
/// surface likely matches for an admin to review before deleting.
pub async fn find_by_email(pool: &PgPool, site_id: Uuid, email: &str) -> Result<Vec<MailLogEntry>> {
    let rows = sqlx::query_as::<_, MailLogEntry>(
        "SELECT * FROM mail_log WHERE site_id = $1 AND to_email ILIKE $2 ORDER BY created_at DESC",
    )
    .bind(site_id)
    .bind(email)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a specific set of mail log entries on a site (e.g. an admin's
/// picks from a GDPR-erasure review list).
pub async fn delete_many(pool: &PgPool, site_id: Uuid, ids: &[Uuid]) -> Result<()> {
    sqlx::query("DELETE FROM mail_log WHERE site_id = $1 AND id = ANY($2)")
        .bind(site_id)
        .bind(ids)
        .execute(pool)
        .await?;
    Ok(())
}

/// Send counts for a form: (total, succeeded, failed).
pub async fn counts_for_form(pool: &PgPool, form_id: Uuid) -> Result<(i64, i64, i64)> {
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE success) FROM mail_log WHERE form_id = $1",
    )
    .bind(form_id)
    .fetch_one(pool)
    .await?;
    Ok((row.0, row.1, row.0 - row.1))
}
