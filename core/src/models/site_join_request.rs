//! Verified self-service site-join tokens for the /subscribe flow.
//!
//! When an existing subscriber's email is submitted to /subscribe on a site
//! they don't yet belong to, a hashed, single-use, 60-minute token backs an
//! emailed confirmation link — mirrors `password_reset.rs`/`email_change.rs`.

use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

const TOKEN_TTL_MINUTES: i64 = 60;

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{:x}", digest)
}

/// A still-valid (unexpired, unused) pending join request.
pub struct PendingJoin {
    pub user_id: Uuid,
    pub site_id: Uuid,
}

/// Request a join for `user_id` on `site_id`, deleting any existing unused
/// request for the same pair first (one active request per user+site at a
/// time). Returns the raw token, for embedding in the emailed link — never
/// persisted as-is.
pub async fn create(pool: &PgPool, user_id: Uuid, site_id: Uuid) -> Result<String> {
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::minutes(TOKEN_TTL_MINUTES);

    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM site_join_requests WHERE user_id = $1 AND site_id = $2 AND used_at IS NULL",
    )
    .bind(user_id)
    .bind(site_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO site_join_requests (user_id, site_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(site_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(token)
}

/// Look up a still-valid token's pending join without consuming it — used to
/// render the "confirm this join" page. Deliberately non-consuming: a mail
/// scanner auto-`GET`ing the link would otherwise burn it before the real
/// user opens it (same reasoning as `email_change::find_valid_by_token`).
pub async fn find_valid_by_token(pool: &PgPool, token: &str) -> Option<PendingJoin> {
    let token_hash = hash_token(token);
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT user_id, site_id FROM site_join_requests
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|(user_id, site_id)| PendingJoin { user_id, site_id })
}

/// Mark a token used so it can't be replayed, returning the (user, site) it
/// belonged to if it was still valid. Deliberately not wrapped in a
/// transaction with the follow-up `site_user::add` call — granting
/// membership is idempotent and low-stakes, unlike a password/email change,
/// so a crash between "burn the token" and "grant membership" just means the
/// user requests again, no lockout or security exposure.
pub async fn consume(pool: &PgPool, token: &str) -> Option<(Uuid, Uuid)> {
    let token_hash = hash_token(token);
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "UPDATE site_join_requests SET used_at = NOW()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()
         RETURNING user_id, site_id",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Deletes any pending join requests for a user — part of GDPR erasure.
pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM site_join_requests WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
