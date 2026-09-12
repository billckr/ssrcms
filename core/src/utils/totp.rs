//! TOTP (RFC 6238) code generation/verification and enrollment URIs, for
//! staff two-factor authentication. Pure local computation — no third-party
//! service. HMAC-SHA1 with 30-second steps and 6 digits, the parameters
//! every real authenticator app (Google Authenticator, Authy, 1Password,
//! Bitwarden) expects; HMAC's security does not depend on SHA-1 collision
//! resistance the way plain hashing does, so this is an interop
//! requirement, not a weaker choice.

use hmac::{Hmac, Mac};
use sha1::Sha1;

const STEP_SECONDS: u64 = 30;
const CODE_DIGITS: u32 = 6;

/// Generate a fresh 160-bit (RFC 4226 recommended) secret, base32-encoded
/// (no padding) — the format `otpauth://` URIs and manual entry both expect.
pub fn generate_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &bytes)
}

/// `otpauth://totp/{issuer}:{label}?secret=...&issuer=...&algorithm=SHA1&digits=6&period=30`,
/// percent-encoded, for QR enrollment.
pub fn provisioning_uri(secret_b32: &str, account_label: &str, issuer: &str) -> String {
    let encoded_label = percent_encode(account_label);
    let encoded_issuer = percent_encode(issuer);
    format!(
        "otpauth://totp/{encoded_issuer}:{encoded_label}?secret={secret_b32}&issuer={encoded_issuer}&algorithm=SHA1&digits=6&period=30"
    )
}

/// Render an `otpauth://` URI as an inline SVG string for direct embedding
/// in the enrollment page — no image codec, no JS library, no external
/// QR-generation service.
pub fn qr_svg(uri: &str) -> Option<String> {
    use qrcode::render::svg;
    use qrcode::QrCode;
    let code = QrCode::new(uri.as_bytes()).ok()?;
    Some(
        code.render()
            .min_dimensions(200, 200)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
    )
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Constant-time byte comparison — avoids a timing side channel when
/// comparing an attacker-supplied code/step against the correct one.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hotp(secret_b32: &str, counter: u64) -> Option<String> {
    let key = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, secret_b32)?;
    let mut mac = Hmac::<Sha1>::new_from_slice(&key).ok()?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();

    // RFC 4226 §5.3 dynamic truncation.
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    let code = binary % 10u32.pow(CODE_DIGITS);
    Some(format!("{:0width$}", code, width = CODE_DIGITS as usize))
}

/// The code for `secret_b32` at `unix_time`, or `None` if the secret isn't
/// valid base32.
pub fn generate_code(secret_b32: &str, unix_time: u64) -> Option<String> {
    hotp(secret_b32, unix_time / STEP_SECONDS)
}

/// Validate `code` (must be exactly 6 ASCII digits) against `secret_b32`,
/// trying steps `[-skew_steps, +skew_steps]` around `unix_time` (±1 step =
/// ±30s clock-drift tolerance). Returns the matched step number — for the
/// caller to reject a step already used, blocking replay — or `None`.
pub fn verify_code(
    secret_b32: &str,
    code: &str,
    unix_time: u64,
    skew_steps: i64,
) -> Option<i64> {
    if code.len() != CODE_DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let current_step = (unix_time / STEP_SECONDS) as i64;
    for delta in -skew_steps..=skew_steps {
        let step = current_step + delta;
        if step < 0 {
            continue;
        }
        if let Some(candidate) = hotp(secret_b32, step as u64) {
            if constant_time_eq(candidate.as_bytes(), code.as_bytes()) {
                return Some(step);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B test vectors — the standard's own reference SHA1
    // secret ("12345678901234567890", i.e. ASCII bytes, base32-encoded
    // below) at published timestamps producing published 8-digit codes.
    // Truncated here to this module's 6-digit output (the last 6 of the
    // published 8) since the RFC's own vector table is for 8-digit codes
    // but the truncation algorithm is identical up to digit count — see
    // the dynamic-truncation step in `hotp`.
    fn rfc_secret_b32() -> String {
        base32::encode(
            base32::Alphabet::Rfc4648 { padding: false },
            b"12345678901234567890",
        )
    }

    #[test]
    fn rfc6238_vector_at_time_59() {
        let secret = rfc_secret_b32();
        // RFC 6238 8-digit reference code at T=59 is 94287082.
        assert_eq!(generate_code(&secret, 59).unwrap(), "287082");
    }

    #[test]
    fn rfc6238_vector_at_time_1111111109() {
        let secret = rfc_secret_b32();
        // RFC 6238 8-digit reference code at T=1111111109 is 07081804.
        assert_eq!(generate_code(&secret, 1_111_111_109).unwrap(), "081804");
    }

    #[test]
    fn rfc6238_vector_at_time_1111111111() {
        let secret = rfc_secret_b32();
        // RFC 6238 8-digit reference code at T=1111111111 is 14050471.
        assert_eq!(generate_code(&secret, 1_111_111_111).unwrap(), "050471");
    }

    #[test]
    fn rfc6238_vector_at_time_1234567890() {
        let secret = rfc_secret_b32();
        // RFC 6238 8-digit reference code at T=1234567890 is 89005924.
        assert_eq!(generate_code(&secret, 1_234_567_890).unwrap(), "005924");
    }

    #[test]
    fn code_is_zero_padded_to_six_digits() {
        // T=59 above already produces a code with no leading zero; find one
        // that does by scanning forward from a fixed secret/time — asserts
        // the padding logic itself rather than relying on luck in the RFC
        // vectors above.
        let secret = generate_secret();
        let padded = (0u64..2000)
            .map(|t| generate_code(&secret, t * 30).unwrap())
            .find(|c| c.starts_with('0'));
        let code = padded.expect("expected at least one zero-padded code in 2000 steps");
        assert_eq!(code.len(), 6);
    }

    #[test]
    fn verify_accepts_current_step() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let code = generate_code(&secret, now).unwrap();
        assert_eq!(verify_code(&secret, &code, now, 1), Some((now / 30) as i64));
    }

    #[test]
    fn verify_accepts_previous_step_within_skew() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let prev_step_time = now - STEP_SECONDS;
        let code = generate_code(&secret, prev_step_time).unwrap();
        assert!(verify_code(&secret, &code, now, 1).is_some());
    }

    #[test]
    fn verify_accepts_next_step_within_skew() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let next_step_time = now + STEP_SECONDS;
        let code = generate_code(&secret, next_step_time).unwrap();
        assert!(verify_code(&secret, &code, now, 1).is_some());
    }

    #[test]
    fn verify_rejects_step_outside_skew_window() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let far_time = now + STEP_SECONDS * 2;
        let code = generate_code(&secret, far_time).unwrap();
        assert_eq!(verify_code(&secret, &code, now, 1), None);
    }

    #[test]
    fn verify_rejects_non_digit_input() {
        let secret = generate_secret();
        assert_eq!(verify_code(&secret, "12a456", 1_700_000_000, 1), None);
    }

    #[test]
    fn verify_rejects_wrong_length_input() {
        let secret = generate_secret();
        assert_eq!(verify_code(&secret, "12345", 1_700_000_000, 1), None);
        assert_eq!(verify_code(&secret, "1234567", 1_700_000_000, 1), None);
    }

    #[test]
    fn verify_rejects_empty_and_whitespace_input() {
        let secret = generate_secret();
        assert_eq!(verify_code(&secret, "", 1_700_000_000, 1), None);
        assert_eq!(verify_code(&secret, " 123456", 1_700_000_000, 1), None);
        assert_eq!(verify_code(&secret, "123456 ", 1_700_000_000, 1), None);
    }

    #[test]
    fn verify_rejects_wrong_code() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let correct = generate_code(&secret, now).unwrap();
        let wrong = if correct == "000000" {
            "111111".to_string()
        } else {
            "000000".to_string()
        };
        assert_eq!(verify_code(&secret, &wrong, now, 1), None);
    }

    #[test]
    fn round_trip_generate_then_verify() {
        let secret = generate_secret();
        let now = 1_700_000_000u64;
        let code = generate_code(&secret, now).unwrap();
        assert_eq!(verify_code(&secret, &code, now, 1), Some((now / 30) as i64));
    }

    #[test]
    fn provisioning_uri_percent_encodes_and_includes_params() {
        let uri = provisioning_uri("ABCD1234", "user@example.com", "Synaptic Signals");
        assert!(uri.starts_with("otpauth://totp/Synaptic%20Signals:user%40example.com"));
        assert!(uri.contains("secret=ABCD1234"));
        assert!(uri.contains("issuer=Synaptic%20Signals"));
        assert!(uri.contains("algorithm=SHA1"));
        assert!(uri.contains("digits=6"));
        assert!(uri.contains("period=30"));
    }

    #[test]
    fn constant_time_eq_equal_strings() {
        assert!(constant_time_eq(b"123456", b"123456"));
    }

    #[test]
    fn constant_time_eq_different_length_no_panic() {
        assert!(!constant_time_eq(b"123", b"123456"));
    }

    #[test]
    fn constant_time_eq_single_byte_difference() {
        assert!(!constant_time_eq(b"123456", b"123457"));
    }

    #[test]
    fn qr_svg_produces_svg_markup() {
        let uri = provisioning_uri("ABCD1234", "user@example.com", "Synaptic Signals");
        let svg = qr_svg(&uri).expect("QR generation should succeed for a normal otpauth URI");
        assert!(svg.contains("<svg"));
    }
}
