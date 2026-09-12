//! Single-use TOTP recovery codes, hashed the same way as
//! `password_reset.rs`'s `token_hash` (SHA-256, never stored raw). Ten are
//! issued at a time, replaced wholesale on regeneration.

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

const CODE_COUNT: usize = 10;
/// Excludes visually ambiguous characters (0/O, 1/I/L) so a code read off a
/// screen or printed page is never mistyped.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const GROUP_LEN: usize = 5;

/// Strip formatting a user might paste (dashes, spaces, mixed case) so
/// input matches regardless of how the code is presented back to them.
fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase()
}

fn hash(normalized: &str) -> String {
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn format_code(raw: &str) -> String {
    raw.chars()
        .collect::<Vec<_>>()
        .chunks(GROUP_LEN)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Generate a fresh batch of human-readable, CSPRNG-backed codes (e.g.
/// `XXXXX-XXXXX`), not yet persisted.
pub fn generate_batch() -> Vec<String> {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    (0..CODE_COUNT)
        .map(|_| {
            let raw: String = (0..GROUP_LEN * 2)
                .map(|_| {
                    let idx = (rng.next_u32() as usize) % ALPHABET.len();
                    ALPHABET[idx] as char
                })
                .collect();
            format_code(&raw)
        })
        .collect()
}

/// Replace every recovery code for `user_id` with `codes` (raw, unhashed —
/// hashed here before storage). Used at initial enrollment and whenever the
/// set is regenerated.
pub async fn replace_all(pool: &PgPool, user_id: Uuid, codes: &[String]) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM mfa_recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for code in codes {
        let code_hash = hash(&normalize(code));
        sqlx::query("INSERT INTO mfa_recovery_codes (user_id, code_hash) VALUES ($1, $2)")
            .bind(user_id)
            .bind(&code_hash)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Consume a still-unused recovery code, returning whether it matched.
/// Single-use: matching marks it used so it can't be replayed.
pub async fn consume(pool: &PgPool, user_id: Uuid, code: &str) -> Result<bool> {
    let code_hash = hash(&normalize(code));
    let result = sqlx::query(
        "UPDATE mfa_recovery_codes SET used_at = NOW()
         WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL",
    )
    .bind(user_id)
    .bind(&code_hash)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Unused recovery codes remaining, for the profile page ("N of 10
/// remaining").
pub async fn count_remaining(pool: &PgPool, user_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mfa_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Removes every recovery code for a user — MFA disable, admin-initiated
/// recovery, and GDPR erasure all call this.
pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM mfa_recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_batch_returns_ten_unique_codes() {
        let codes = generate_batch();
        assert_eq!(codes.len(), CODE_COUNT);
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), CODE_COUNT);
    }

    #[test]
    fn generated_codes_exclude_ambiguous_characters() {
        let codes = generate_batch();
        for code in &codes {
            for c in code.chars() {
                if c == '-' {
                    continue;
                }
                assert!(
                    !matches!(c, '0' | 'O' | '1' | 'I' | 'L'),
                    "code {code} contains an ambiguous character"
                );
            }
        }
    }

    #[test]
    fn generated_codes_are_grouped_with_a_dash() {
        let codes = generate_batch();
        for code in &codes {
            assert_eq!(code.len(), GROUP_LEN * 2 + 1);
            assert_eq!(code.chars().nth(GROUP_LEN), Some('-'));
        }
    }

    #[test]
    fn normalization_ignores_case_whitespace_and_dashes() {
        let a = normalize("abcde-fghij");
        let b = normalize("ABCDE-FGHIJ");
        let c = normalize(" ABCDEFGHIJ ");
        let d = normalize("ABCD EFGHIJ");
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(c, d);
    }

    #[test]
    fn hash_is_stable_for_equivalent_input() {
        assert_eq!(
            hash(&normalize("abcde-fghij")),
            hash(&normalize("ABCDE FGHIJ"))
        );
    }
}
