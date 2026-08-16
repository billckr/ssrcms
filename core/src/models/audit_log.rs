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

/// Maps an activity-log sort key to its backing column. `sites.hostname`
/// requires the LEFT JOIN in [`list_filtered`]/[`count_filtered`] — every
/// other column is unambiguous on `audit_log` alone. Anything unrecognized
/// (including "") falls back to `created_at`, the natural recency order.
fn sort_column(sort: &str) -> &'static str {
    match sort {
        "who" => "audit_log.actor_email",
        "action" => "audit_log.action",
        "target" => "audit_log.target_label",
        "site" => "sites.hostname",
        _ => "audit_log.created_at",
    }
}

/// Entries for the admin Activity Log, with search/sort/pagination applied
/// in SQL rather than in memory — this table can grow far larger than the
/// sites list, so unlike `admin::pages::sites` this doesn't fetch-then-filter.
/// `site_ids: None` is the global-admin unfiltered view; `Some(&[..])` scopes
/// to one or more sites (a site admin's own sites, or the "filter to one
/// site" dropdown). `search` matches actor email, action, target label, or
/// site hostname (case-insensitive substring); pass `""` to skip filtering.
pub async fn list_filtered(
    pool: &PgPool,
    site_ids: Option<&[Uuid]>,
    search: &str,
    sort: &str,
    dir: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditLogEntry>> {
    let order_col = sort_column(sort);
    // "when" (the default column) reads naturally newest-first; every other
    // column reads naturally ascending (A-Z) unless the caller asks otherwise.
    let order_dir = match dir {
        "asc" => "ASC",
        "desc" => "DESC",
        _ if sort.is_empty() || sort == "when" => "DESC",
        _ => "ASC",
    };
    let sql = format!(
        "SELECT audit_log.* FROM audit_log LEFT JOIN sites ON sites.id = audit_log.site_id \
         WHERE ($1::uuid[] IS NULL OR audit_log.site_id = ANY($1)) \
           AND ($2 = '' OR audit_log.actor_email ILIKE '%' || $2 || '%' \
                OR audit_log.action ILIKE '%' || $2 || '%' \
                OR audit_log.target_label ILIKE '%' || $2 || '%' \
                OR COALESCE(sites.hostname, '') ILIKE '%' || $2 || '%') \
         ORDER BY {order_col} {order_dir} LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query_as::<_, AuditLogEntry>(&sql)
        .bind(site_ids)
        .bind(search)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn count_filtered(pool: &PgPool, site_ids: Option<&[Uuid]>, search: &str) -> Result<i64> {
    let c: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log LEFT JOIN sites ON sites.id = audit_log.site_id \
         WHERE ($1::uuid[] IS NULL OR audit_log.site_id = ANY($1)) \
           AND ($2 = '' OR audit_log.actor_email ILIKE '%' || $2 || '%' \
                OR audit_log.action ILIKE '%' || $2 || '%' \
                OR audit_log.target_label ILIKE '%' || $2 || '%' \
                OR COALESCE(sites.hostname, '') ILIKE '%' || $2 || '%')",
    )
    .bind(site_ids)
    .bind(search)
    .fetch_one(pool)
    .await?;
    Ok(c)
}
