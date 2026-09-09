//! Denylist of common/weak passwords that are nonetheless long enough to
//! pass `user::validate_password`'s 12-character minimum.
//!
//! This is a small, hand-curated list (not a real breach corpus — see
//! `user::validate_password`'s call site for why) built from well-known
//! "worst passwords" patterns: a weak base word padded with digits to clear
//! a length floor, common keyboard walks, repeated characters, and a few
//! famous passphrases people use ironically/literally. All entries are
//! lowercase and at least 12 characters — anything shorter could never match
//! a password that already passed the length check, so a shorter entry here
//! would just be dead weight.
//!
//! Deliberately an exact-match denylist, not a substring/fuzzy check: a
//! substring match on something like "password" would also reject a
//! legitimate long passphrase that merely happens to contain that word,
//! which is a worse false-positive trade than the coverage it would buy.

use once_cell::sync::Lazy;
use std::collections::HashSet;

static COMMON_PASSWORDS: &[&str] = &[
    // Weak base word + digit padding to clear the 12-character floor.
    "password123456",
    "password1234567",
    "password12345678",
    "letmein123456",
    "letmein1234567",
    "letmeinnow123456",
    "welcome123456",
    "welcome1234567",
    "welcome12345678",
    "iloveyou123456",
    "iloveyou1234567",
    "iloveyou12345678",
    "sunshine123456",
    "sunshine1234567",
    "princess123456",
    "princess1234567",
    "superman123456",
    "superman1234567",
    "batman123456789",
    "football123456",
    "football1234567",
    "baseball123456",
    "baseball1234567",
    "basketball12345",
    "dragon1234567",
    "dragon12345678",
    "monkey1234567",
    "monkey12345678",
    "trustno1123456",
    "trustno1trustno1",
    "whatever123456",
    "whatever1234567",
    "changeme123456",
    "changeme1234567",
    "changeme2024123",
    "temppassword123",
    "temppass123456",
    "defaultpassword",
    "defaultpass123456",
    "testpassword123",
    "testpass123456",
    "adminpassword123",
    "adminpass123456",
    "rootpassword123",
    "rootpassword1234",
    "superadmin123456",
    "mynameisadmin123",
    "newpassword12345",
    "oldpassword12345",
    "mypasswordisbad",
    "thisisapassword",
    "hunter212345678",
    "jennifer12345678",
    "jordan1234567890",
    "michael123456789",
    "jessica123456789",
    "amanda1234567890",
    "ashley1234567890",
    "matthew123456789",
    "brooklyn12345678",
    "liverpool123456",
    "liverpool1234567",
    "arsenal123456789",
    "chelsea123456789",
    "manchester123456",
    "tottenham1234567",
    "starwars12345678",
    "pokemon123456789",
    "minecraft1234567",
    "fortnite12345678",
    "welcome2024123",
    "welcome20241234",
    "hello123456789",
    "hello1234567890",
    // Keyboard walks and adjacent-key patterns long enough to look "random"
    // at a glance but still trivially guessable.
    "qwertyuiop1234",
    "qwertyuiop12345",
    "qwertyqwerty123",
    "qwertyuiopasdf",
    "asdfghjkl123456",
    "asdfghjklzxcvbn",
    "zxcvbnmasdfghjkl",
    "1qaz2wsx3edc456",
    "1qaz2wsx3edc4rfv",
    "1q2w3e4r5t6y",
    "1q2w3e4r5t6y7u",
    "qazwsxedc123456",
    "qazwsxedcrfv123",
    // Sequential / repeated-character padding.
    "abcdefghijklmn",
    "abcdefghijk123",
    "abcdefghijklmnop",
    "123456789012",
    "1234567890123",
    "12345678901234",
    "123456123456",
    "111111111111",
    "1111111111111",
    "000000000000",
    "0000000000000",
    "aaaaaaaaaaaa",
    "aaaaaaaaaaaaaa",
    // Famous / meme passphrases that show up literally in real breach data
    // because people use them as an in-joke, not just as an example.
    "correcthorsebatterystaple",
    "ilovemylife123",
    "letthemeatcake123",
    "thequickbrownfox",
];

static COMMON_PASSWORD_SET: Lazy<HashSet<&'static str>> =
    Lazy::new(|| COMMON_PASSWORDS.iter().copied().collect());

/// Whether `password` (any case, surrounding whitespace ignored) matches a
/// known common/weak password on the denylist above.
pub fn is_common_password(password: &str) -> bool {
    COMMON_PASSWORD_SET.contains(password.trim().to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_meets_the_password_length_floor() {
        for entry in COMMON_PASSWORDS {
            assert!(
                entry.chars().count() >= 12,
                "denylist entry '{entry}' is under the 12-character minimum — \
                 it could never match a password that already passed length validation"
            );
        }
    }

    #[test]
    fn every_entry_is_already_lowercase() {
        for entry in COMMON_PASSWORDS {
            assert_eq!(
                entry.to_lowercase(),
                *entry,
                "denylist entry '{entry}' isn't lowercase — lookups normalize to lowercase, \
                 so a mixed-case entry here can never match"
            );
        }
    }

    #[test]
    fn no_duplicate_entries() {
        let set: HashSet<&str> = COMMON_PASSWORDS.iter().copied().collect();
        assert_eq!(
            set.len(),
            COMMON_PASSWORDS.len(),
            "denylist contains a duplicate entry"
        );
    }

    #[test]
    fn detects_known_common_passwords_case_insensitively() {
        assert!(is_common_password("password123456"));
        assert!(is_common_password("PASSWORD123456"));
        assert!(is_common_password("  correcthorsebatterystaple  "));
    }

    #[test]
    fn does_not_flag_a_reasonable_passphrase() {
        assert!(!is_common_password("Purple-Giraffe-Orbits-Neptune-42"));
    }
}
