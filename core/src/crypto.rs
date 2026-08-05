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
    let plaintext = cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext).ok()?;
    String::from_utf8(plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let encrypted = encrypt("test-secret-key", "key-0123456789abcdef");
        assert_eq!(decrypt("test-secret-key", &encrypted).as_deref(), Some("key-0123456789abcdef"));
    }

    #[test]
    fn wrong_key_fails() {
        let encrypted = encrypt("test-secret-key", "hello");
        assert_eq!(decrypt("different-key", &encrypted), None);
    }
}
