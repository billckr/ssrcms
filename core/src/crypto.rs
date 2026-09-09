//! Symmetric encryption for secrets stored at rest in the DB (e.g. a site's
//! own Mailgun API key, set on the site Settings page). Keyed off
//! `SECRET_KEY` — the value `config.rs` already requires be set in
//! production — so there's no separate secret to provision or rotate.
//!
//! AES-256-GCM with a random nonce per encryption. Output is
//! base64(nonce || ciphertext), one opaque string safe to store in a text
//! column.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};

fn derive_key(secret_key: &str) -> Key<Aes256Gcm> {
    let hash = Sha256::digest(secret_key.as_bytes());
    Key::<Aes256Gcm>::clone_from_slice(&hash)
}

pub fn encrypt(secret_key: &str, plaintext: &str) -> String {
    let cipher = Aes256Gcm::new(&derive_key(secret_key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .expect("AES-GCM encryption of an in-memory buffer cannot fail");
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    STANDARD.encode(combined)
}

/// Returns `None` on any malformed input or if `secret_key` doesn't match
/// the key it was encrypted with (e.g. SECRET_KEY changed since).
pub fn decrypt(secret_key: &str, encoded: &str) -> Option<String> {
    let cipher = Aes256Gcm::new(&derive_key(secret_key));
    let combined = STANDARD.decode(encoded).ok()?;
    if combined.len() < 12 {
        return None;
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .ok()?;
    String::from_utf8(plaintext).ok()
}

/// Masks a secret for display in an admin UI: dash-delimited keys
/// (Mailgun-style) show only their last two segments, e.g.
/// `966c...280-11c539c0-c7ddc18d` becomes `11c539c0-c7ddc18d` — the same
/// trailing portion Mailgun's own dashboard shows. Anything else shows only
/// its last 8 characters. Shared by every provider-config model that stores
/// a secret at rest (email providers, AI providers, ...) so the masking
/// logic exists in exactly one place.
pub fn mask_secret(s: &str) -> String {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() >= 3 {
        format!("{}-{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else if s.chars().count() > 8 {
        let tail: String = s
            .chars()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("...{}", tail)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let encrypted = encrypt("test-secret-key", "key-0123456789abcdef");
        assert_eq!(
            decrypt("test-secret-key", &encrypted).as_deref(),
            Some("key-0123456789abcdef")
        );
    }

    #[test]
    fn wrong_key_fails() {
        let encrypted = encrypt("test-secret-key", "hello");
        assert_eq!(decrypt("different-key", &encrypted), None);
    }

    #[test]
    fn mask_secret_shows_last_two_dash_segments() {
        assert_eq!(
            mask_secret("fixture-middle-visible-tail"),
            "visible-tail"
        );
    }

    #[test]
    fn mask_secret_shows_last_eight_chars_otherwise() {
        assert_eq!(mask_secret("abcdefghijklmnopqrstuvwxyz"), "...stuvwxyz");
    }

    #[test]
    fn mask_secret_leaves_short_strings_unmasked() {
        assert_eq!(mask_secret("short"), "short");
    }
}
