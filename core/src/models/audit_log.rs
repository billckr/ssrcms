//! Persistent, queryable record of account/site lifecycle events — who did
//! what, to what, and when. Distinct from the tracing log file (rotates,
//! not queryable from inside the app) — see mail_log for the same rationale
//! applied to outbound email.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_email: String,
    pub actor_role: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub target_label: String,
    pub site_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

pub struct NewAuditLog<'a> {
    /// None for events with no attributable account (e.g. a failed login
    /// attempt against an email that doesn't match any user) — actor_email
    /// still captures what was typed. Never use a placeholder UUID here:
    /// the column has a FK to users(id), so an invented id would either
    /// violate the constraint or silently point at an unrelated real user.
    pub actor_user_id: Option<Uuid>,
    pub actor_email: &'a str,
    pub actor_role: &'a str,
    /// Dot-namespaced, e.g. "site.created", "user.deleted", "site_user.added".
    pub action: &'a str,
    /// e.g. "site", "user".
    pub target_type: &'a str,
    pub target_id: Option<Uuid>,
    /// Human-readable snapshot of the target (hostname, username, email) —
    /// kept even if the target row itself is later deleted.
    pub target_label: &'a str,
    /// Which site this pertains to, for scoping a site admin's view. None
    /// for actions with no single owning site (e.g. creating an unassigned
    /// user).
    pub site_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
}

/// Records one audit event. Logging must never block the action it's
/// recording — callers should `tracing::warn!` on `Err` rather than
/// propagate it.
pub async fn record(pool: &PgPool, input: NewAuditLog<'_>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO audit_log
           (actor_user_id, actor_email, actor_role, action, target_type, target_id, target_label, site_id, details)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(input.actor_user_id)
    .bind(input.actor_email)
    .bind(input.actor_role)
    .bind(input.action)
    .bind(input.target_type)
    .bind(input.target_id)
    .bind(input.target_label)
    .bind(input.site_id)
    .bind(input.details)
    .execute(pool)
    .await?;
    Ok(())
}

/// Most recent entries across every site — global admin, unfiltered view.
pub async fn list_all(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<AuditLogEntry>> {
    let rows = sqlx::query_as::<_, AuditLogEntry>(
        "SELECT * FROM audit_log ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Entries scoped to a set of sites — a site admin's own site(s), or the
/// global admin's "filter to one site" view (pass a single-element slice).
pub async fn list_for_sites(pool: &PgPool, site_ids: &[Uuid], limit: i64, offset: i64) -> Result<Vec<AuditLogEntry>> {
    let rows = sqlx::query_as::<_, AuditLogEntry>(
        "SELECT * FROM audit_log WHERE site_id = ANY($1) ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(site_ids)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_all(pool: &PgPool) -> Result<i64> {
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log").fetch_one(pool).await?;
    Ok(c)
}

pub async fn count_for_sites(pool: &PgPool, site_ids: &[Uuid]) -> Result<i64> {
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE site_id = ANY($1)")
        .bind(site_ids)
        .fetch_one(pool)
        .await?;
    Ok(c)
}
