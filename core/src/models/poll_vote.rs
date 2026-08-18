//! Votes cast against a `poll_def`. Distinct from `poll_def` — that module
//! owns the poll's shape (question/options); this one owns what visitors
//! actually submitted.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::errors::Result;
use crate::models::poll_def::VoteProtection;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PollVote {
    pub id: Uuid,
    pub poll_id: Uuid,
    pub site_id: Uuid,
    pub option_key: String,
    pub voter_token: String,
    pub ip_address: Option<String>,
    pub voted_at: DateTime<Utc>,
}

/// Whether this browser/IP has already voted on this poll, per the poll's
/// configured protection level. `CookieOnly` only checks the voter token
/// (a missing/absent cookie means "not yet voted" even from a familiar IP);
/// `CookieAndIp` additionally blocks a re-vote from the same IP even if the
/// cookie was cleared.
pub async fn has_voted(
    pool: &PgPool,
    poll_id: Uuid,
    voter_token: &str,
    ip_address: Option<&str>,
    protection: VoteProtection,
) -> bool {
    let by_token: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM poll_votes WHERE poll_id = $1 AND voter_token = $2)",
    )
    .bind(poll_id)
    .bind(voter_token)
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    if by_token {
        return true;
    }
    if protection == VoteProtection::CookieAndIp {
        if let Some(ip) = ip_address.filter(|s| !s.is_empty()) {
            return sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM poll_votes WHERE poll_id = $1 AND ip_address = $2)",
            )
            .bind(poll_id)
            .bind(ip)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
        }
    }
    false
}

pub struct RecordVote<'a> {
    pub poll_id: Uuid,
    pub site_id: Uuid,
    pub option_key: &'a str,
    pub voter_token: &'a str,
    pub ip_address: Option<&'a str>,
}

/// Record a vote and bump the poll's lifetime counter. Relies on the
/// `(poll_id, voter_token)` unique index as the final guard against a
/// double-vote race (two concurrent requests from the same browser before
/// either commits) — the caller should still check `has_voted` first for
/// the normal "redirect to already_voted" UX, but this is what actually
/// makes the dedupe airtight. Returns `true` if a new vote was recorded,
/// `false` if the unique index rejected it as a duplicate.
pub async fn record_vote(pool: &PgPool, input: RecordVote<'_>) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO poll_votes (poll_id, site_id, option_key, voter_token, ip_address)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (poll_id, voter_token) DO NOTHING",
    )
    .bind(input.poll_id)
    .bind(input.site_id)
    .bind(input.option_key)
    .bind(input.voter_token)
    .bind(input.ip_address)
    .execute(pool)
    .await?;

    let inserted = result.rows_affected() > 0;
    if inserted {
        sqlx::query("UPDATE polls SET total_votes = total_votes + 1 WHERE id = $1")
            .bind(input.poll_id)
            .execute(pool)
            .await?;
    }
    Ok(inserted)
}

/// Vote counts per option, in no particular order — the caller (rendering
/// results in the poll's own declared option order) joins this against
/// `PollDef::options`.
pub async fn tally(pool: &PgPool, poll_id: Uuid) -> Result<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT option_key, COUNT(*) FROM poll_votes WHERE poll_id = $1 GROUP BY option_key",
    )
    .bind(poll_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_for_poll(pool: &PgPool, site_id: Uuid, poll_id: Uuid) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM poll_votes WHERE site_id = $1 AND poll_id = $2",
    )
    .bind(site_id)
    .bind(poll_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn list_votes(pool: &PgPool, site_id: Uuid, poll_id: Uuid, limit: i64, offset: i64) -> Result<Vec<PollVote>> {
    let rows = sqlx::query_as::<_, PollVote>(
        "SELECT * FROM poll_votes WHERE site_id = $1 AND poll_id = $2
         ORDER BY voted_at DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(site_id)
    .bind(poll_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every vote's raw row for a poll, across all pages — used for CSV export
/// (same reasoning as `form_submission::list_all_data_for_form`: export
/// should include everything, not just the current page).
pub async fn list_all_votes(pool: &PgPool, site_id: Uuid, poll_id: Uuid) -> Result<Vec<PollVote>> {
    let rows = sqlx::query_as::<_, PollVote>(
        "SELECT * FROM poll_votes WHERE site_id = $1 AND poll_id = $2 ORDER BY voted_at DESC",
    )
    .bind(site_id)
    .bind(poll_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete all votes for a poll and reset its lifetime counter — an explicit
/// "reset results" action, distinct from deleting the poll definition
/// itself.
pub async fn delete_all(pool: &PgPool, site_id: Uuid, poll_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM poll_votes WHERE site_id = $1 AND poll_id = $2")
        .bind(site_id)
        .bind(poll_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE polls SET total_votes = 0 WHERE id = $1")
        .bind(poll_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
