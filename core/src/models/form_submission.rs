use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::errors::Result;

/// A single form submission stored for a site.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FormSubmission {
    pub id: Uuid,
    pub site_id: Uuid,
    pub form_name: String,
    pub data: serde_json::Value,
    pub ip_address: Option<String>,
    pub read_at: Option<DateTime<Utc>>,
    pub submitted_at: DateTime<Utc>,
}

/// Create a new form submission.
pub struct CreateFormSubmission {
    pub site_id: Uuid,
    pub form_name: String,
    pub data: serde_json::Value,
    pub ip_address: Option<String>,
}

pub async fn create(pool: &PgPool, input: CreateFormSubmission) -> Result<FormSubmission> {
    let row = sqlx::query_as::<_, FormSubmission>(
        "INSERT INTO form_submissions (site_id, form_name, data, ip_address)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(input.site_id)
    .bind(&input.form_name)
    .bind(&input.data)
    .bind(&input.ip_address)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// List distinct form names for a site with submission counts and latest timestamp.
pub async fn list_forms(pool: &PgPool, site_id: Uuid) -> Result<Vec<FormSummary>> {
    let rows = sqlx::query_as::<_, FormSummary>(
        "SELECT form_name,
                COUNT(*) AS submission_count,
                MAX(submitted_at) AS last_submitted_at,
                COUNT(*) FILTER (WHERE read_at IS NULL) AS unread_count
         FROM form_submissions
         WHERE site_id = $1
         GROUP BY form_name
         ORDER BY MAX(submitted_at) DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FormSummary {
    pub form_name: String,
    pub submission_count: i64,
    pub last_submitted_at: DateTime<Utc>,
    pub unread_count: i64,
}

/// List submissions for a specific form, newest first.
pub async fn list_submissions(
    pool: &PgPool,
    site_id: Uuid,
    form_name: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<FormSubmission>> {
    let rows = sqlx::query_as::<_, FormSubmission>(
        "SELECT * FROM form_submissions
         WHERE site_id = $1 AND form_name = $2
         ORDER BY submitted_at DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(site_id)
    .bind(form_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a single submission by ID, enforcing site ownership.
pub async fn delete(pool: &PgPool, site_id: Uuid, id: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM form_submissions WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .fetch_optional(pool)
    .await?;
    Ok(())
}

/// Delete all submissions for a named form on a site.
pub async fn delete_all(pool: &PgPool, site_id: Uuid, form_name: &str) -> Result<()> {
    sqlx::query(
        "DELETE FROM form_submissions WHERE site_id = $1 AND form_name = $2",
    )
    .bind(site_id)
    .bind(form_name)
    .fetch_optional(pool)
    .await?;
    Ok(())
}

/// Count unread submissions for a site across all forms.
pub async fn count_unread(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM form_submissions WHERE site_id = $1 AND read_at IS NULL",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Check whether a form is currently blocked from accepting submissions.
pub async fn is_blocked(pool: &PgPool, site_id: Uuid, form_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM form_blocks WHERE site_id = $1 AND form_name = $2)",
    )
    .bind(site_id)
    .bind(form_name)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

/// Block a form — future submissions will be silently rejected.
pub async fn block(pool: &PgPool, site_id: Uuid, form_name: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO form_blocks (site_id, form_name) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(site_id)
    .bind(form_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Unblock a form — resume accepting submissions.
pub async fn unblock(pool: &PgPool, site_id: Uuid, form_name: &str) -> Result<()> {
    sqlx::query("DELETE FROM form_blocks WHERE site_id = $1 AND form_name = $2")
        .bind(site_id)
        .bind(form_name)
        .execute(pool)
        .await?;
    Ok(())
}

/// Return all blocked form names for a site.
pub async fn blocked_names(pool: &PgPool, site_id: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT form_name FROM form_blocks WHERE site_id = $1")
        .bind(site_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

/// Mark all submissions for a form as read.
pub async fn mark_all_read(pool: &PgPool, site_id: Uuid, form_name: &str) -> Result<()> {
    sqlx::query(
        "UPDATE form_submissions SET read_at = NOW()
         WHERE site_id = $1 AND form_name = $2 AND read_at IS NULL",
    )
    .bind(site_id)
    .bind(form_name)
    .fetch_optional(pool)
    .await?;
    Ok(())
}
