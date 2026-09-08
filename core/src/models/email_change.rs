//! Verified self-service email-change tokens for the /account email flow.
//!
//! Mirrors `password_reset.rs`'s posture: the raw token is only ever held in
//! memory and in the emailed link — the DB stores just its SHA-256 hash, so a
//! DB read alone can't be replayed to change an account's email.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;
use crate::models::user::User;

const TOKEN_TTL_MINUTES: i64 = 60;

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{:x}", digest)
}

/// A still-valid (unexpired, unused) pending change, for rendering a confirm
/// page or a "pending change" banner without consuming the token.
pub struct PendingChange {
    pub user_id: Uuid,
    pub new_email: String,
    pub expires_at: DateTime<Utc>,
}

/// Result of successfully consuming a token and committing the email change.
pub struct AppliedEmailChange {
    pub user: User,
    pub old_email: String,
}

/// Request a new email change for `user_id`, deleting any existing unused
/// request for them first (only one pending change at a time — requesting a
/// new one supersedes the last). Returns the raw token, for embedding in the
/// emailed link — never persisted as-is. `new_email` must already be
/// normalized by the caller.
pub async fn create(pool: &PgPool, user_id: Uuid, new_email: &str) -> Result<String> {
    // Two concatenated UUIDv4s: 64 hex chars, backed by the OS CSPRNG — same
    // approach as password_reset::create.
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::minutes(TOKEN_TTL_MINUTES);

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM email_changes WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO email_changes (user_id, new_email, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(new_email)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(token)
}

/// Look up a still-valid (unexpired, unused) token's pending change without
/// consuming it — used to render the "confirm this change" page. Deliberately
/// non-consuming: corporate mail scanners (e.g. Outlook Safe Links) auto-GET
/// links in inbound mail, which would silently burn a consume-on-GET token
/// before the real user ever opens it.
pub async fn find_valid_by_token(pool: &PgPool, token: &str) -> Option<PendingChange> {
    let token_hash = hash_token(token);
    sqlx::query_as::<_, (Uuid, String, DateTime<Utc>)>(
        "SELECT user_id, new_email, expires_at FROM email_changes
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|(user_id, new_email, expires_at)| PendingChange {
        user_id,
        new_email,
        expires_at,
    })
}

/// Look up the most recent still-valid pending change for a user, for a
/// profile-page "a change to X is pending" banner.
pub async fn find_pending_for_user(pool: &PgPool, user_id: Uuid) -> Option<PendingChange> {
    sqlx::query_as::<_, (Uuid, String, DateTime<Utc>)>(
        "SELECT user_id, new_email, expires_at FROM email_changes
         WHERE user_id = $1 AND used_at IS NULL AND expires_at > NOW()
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|(user_id, new_email, expires_at)| PendingChange {
        user_id,
        new_email,
        expires_at,
    })
}

/// Atomically consume a valid token and commit its candidate email to the
/// user row. `Ok(None)` means the token was invalid, expired, or already
/// used. A unique-violation on `users_email_lower_unique` (another account
/// claimed that address between request and confirm) propagates as `Err` —
/// that index is the real collision guard, not a pre-check in this function.
pub async fn consume_and_apply(pool: &PgPool, token: &str) -> Result<Option<AppliedEmailChange>> {
    let token_hash = hash_token(token);
    let mut tx = pool.begin().await?;

    let row = sqlx::query_as::<_, (Uuid, String)>(
        "UPDATE email_changes SET used_at = NOW()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()
         RETURNING user_id, new_email",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((user_id, new_email)) = row else {
        tx.rollback().await?;
        return Ok(None);
    };

    let old_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

    let new_email = crate::models::user::normalize_email(&new_email);
    let user = sqlx::query_as::<_, User>(
        "UPDATE users SET email = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(&new_email)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some(AppliedEmailChange { user, old_email }))
}

/// Deletes any pending email-change requests for a user — part of GDPR
/// erasure. Required even though `email_changes.user_id` is
/// `ON DELETE CASCADE`: `user::erase_personal_data` anonymizes the row in
/// place rather than deleting it, so that cascade never fires here.
pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM email_changes WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
