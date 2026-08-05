//! Password recovery tokens for the public /recover flow.
//!
//! The raw token is only ever held in memory and in the emailed link — the
//! DB stores just its SHA-256 hash, so a DB read alone can't be replayed to
//! reset an account (same posture as `password_hash`).

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

/// Generate a new reset token for `user_id`, store its hash, and return the
/// raw token (for embedding in the emailed link — never persisted as-is).
pub async fn create(pool: &PgPool, user_id: Uuid) -> Result<String> {
    // Two concatenated UUIDv4s: 64 hex chars, backed by the OS CSPRNG — same
    // approach already used for fallback username generation, so no new
    // random-number-generation dependency is introduced just for this.
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::minutes(TOKEN_TTL_MINUTES);

    sqlx::query(
        "INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(token)
}

/// Look up the user a still-valid (unexpired, unused) token belongs to,
/// without consuming it — used to decide whether to show the "set a new
/// password" form.
pub async fn find_valid_user_id(pool: &PgPool, token: &str) -> Option<Uuid> {
    let token_hash = hash_token(token);
    sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM password_resets
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Mark a token used so it can't be replayed, returning the user it
/// belonged to if it was still valid at the time of the call.
pub async fn consume(pool: &PgPool, token: &str) -> Option<Uuid> {
    let token_hash = hash_token(token);
    sqlx::query_scalar::<_, Uuid>(
        "UPDATE password_resets SET used_at = NOW()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()
         RETURNING user_id",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}
