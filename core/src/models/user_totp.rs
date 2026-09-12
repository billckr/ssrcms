//! TOTP two-factor authentication for staff logins (`/admin/login`).
//!
//! One row per user. `enabled_at IS NULL` means enrollment was started
//! (secret generated, QR shown) but never confirmed with a correct code —
//! `start_enrollment` overwrites that row wholesale on a fresh attempt, so
//! an abandoned enrollment never lingers as a stale, never-verified secret.
//!
//! The secret is encrypted at rest with `crate::crypto` (AES-256-GCM keyed
//! off `SECRET_KEY`) — the same pattern already used for site email-provider
//! API keys — because, unlike a password, the raw secret must be
//! recoverable to compute/verify codes.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;
use crate::utils::totp;

/// ±1 step = ±30s clock-drift tolerance either side of "now", the
/// conventional TOTP allowance.
const SKEW_STEPS: i64 = 1;

#[derive(sqlx::FromRow)]
struct TotpRow {
    secret_encrypted: String,
    enabled_at: Option<DateTime<Utc>>,
    last_used_step: Option<i64>,
}

/// Start (or restart) enrollment: generates a fresh secret, encrypts it,
/// and upserts the row with `enabled_at = NULL`. Returns the raw base32
/// secret for the QR/manual-entry display — never persisted raw.
pub async fn start_enrollment(pool: &PgPool, secret_key: &str, user_id: Uuid) -> Result<String> {
    let secret = totp::generate_secret();
    let encrypted = crate::crypto::encrypt(secret_key, &secret);

    sqlx::query(
        "INSERT INTO user_totp (user_id, secret_encrypted, enabled_at, last_used_step, updated_at)
         VALUES ($1, $2, NULL, NULL, NOW())
         ON CONFLICT (user_id) DO UPDATE
         SET secret_encrypted = EXCLUDED.secret_encrypted,
             enabled_at = NULL,
             last_used_step = NULL,
             updated_at = NOW()",
    )
    .bind(user_id)
    .bind(&encrypted)
    .execute(pool)
    .await?;

    Ok(secret)
}

/// Confirm a pending enrollment with the first code from the freshly
/// scanned authenticator app. Returns `Ok(false)` for a wrong/missing code
/// (not an error — a wrong code during setup is an expected user mistake).
pub async fn confirm_enrollment(
    pool: &PgPool,
    secret_key: &str,
    user_id: Uuid,
    code: &str,
    now: DateTime<Utc>,
) -> Result<bool> {
    let Some(row) = sqlx::query_as::<_, TotpRow>(
        "SELECT secret_encrypted, enabled_at, last_used_step FROM user_totp WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };
    if row.enabled_at.is_some() {
        // Already confirmed — nothing pending to confirm against.
        return Ok(false);
    }
    let Some(secret) = crate::crypto::decrypt(secret_key, &row.secret_encrypted) else {
        return Ok(false);
    };
    let unix_time = now.timestamp().max(0) as u64;
    if totp::verify_code(&secret, code, unix_time, SKEW_STEPS).is_none() {
        return Ok(false);
    };

    // Deliberately leave `last_used_step` NULL rather than recording the
    // confirmation code's step: this action never establishes a login
    // session (the browser is already authenticated to reach this page at
    // all), so there's nothing to protect by blocking replay of it — and
    // recording it here would otherwise spuriously reject the very next
    // real login if it lands in the same 30-second window as setup.
    sqlx::query("UPDATE user_totp SET enabled_at = $2, updated_at = NOW() WHERE user_id = $1")
        .bind(user_id)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(true)
}

pub async fn is_enabled(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    let enabled: Option<bool> = sqlx::query_scalar(
        "SELECT enabled_at IS NOT NULL FROM user_totp WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(enabled.unwrap_or(false))
}

/// When MFA was confirmed, for display on the profile page. `None` if
/// never enabled.
pub async fn enabled_at(pool: &PgPool, user_id: Uuid) -> Result<Option<DateTime<Utc>>> {
    let at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT enabled_at FROM user_totp WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    Ok(at)
}

/// Decrypts the secret for a still-unconfirmed enrollment, for re-rendering
/// the QR/manual-entry setup page on reload without regenerating it.
/// `None` if there's no pending enrollment for this user.
pub async fn pending_secret(pool: &PgPool, secret_key: &str, user_id: Uuid) -> Result<Option<String>> {
    let Some(row) = sqlx::query_as::<_, TotpRow>(
        "SELECT secret_encrypted, enabled_at, last_used_step FROM user_totp WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    if row.enabled_at.is_some() {
        return Ok(None);
    }
    Ok(crate::crypto::decrypt(secret_key, &row.secret_encrypted))
}

/// Verify a login-time code against the user's *confirmed* secret. Locks
/// the row (`SELECT ... FOR UPDATE`) for the duration of the check so two
/// concurrent requests replaying the same code can't both pass before
/// either one records `last_used_step`.
pub async fn verify_login_code(
    pool: &PgPool,
    secret_key: &str,
    user_id: Uuid,
    code: &str,
    now: DateTime<Utc>,
) -> Result<bool> {
    let mut tx = pool.begin().await?;

    let Some(row) = sqlx::query_as::<_, TotpRow>(
        "SELECT secret_encrypted, enabled_at, last_used_step FROM user_totp WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        tx.rollback().await?;
        return Ok(false);
    };
    if row.enabled_at.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }
    let Some(secret) = crate::crypto::decrypt(secret_key, &row.secret_encrypted) else {
        tx.rollback().await?;
        return Ok(false);
    };
    let unix_time = now.timestamp().max(0) as u64;
    let Some(step) = totp::verify_code(&secret, code, unix_time, SKEW_STEPS) else {
        tx.rollback().await?;
        return Ok(false);
    };
    if row.last_used_step == Some(step) {
        // Same code already consumed this step — block replay.
        tx.rollback().await?;
        return Ok(false);
    }

    sqlx::query("UPDATE user_totp SET last_used_step = $2, updated_at = NOW() WHERE user_id = $1")
        .bind(user_id)
        .bind(step)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(true)
}

/// Remove MFA entirely for a user — self-service disable, or an
/// admin-initiated recovery for a locked-out account. Caller is
/// responsible for also clearing recovery codes
/// (`mfa_recovery_code::delete_all_for_user`).
pub async fn disable(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM user_totp WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Pure-logic edge cases live in `utils::totp`; everything here needs a
    // live Postgres connection and is covered by `core/tests/routes.rs`
    // integration tests instead (enrollment, login-gate, replay, disable).
}
