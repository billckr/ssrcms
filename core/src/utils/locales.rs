//! A small, hand-curated list of common locale codes and their English
//! display names, for the AI-translation feature's "which languages does
//! this site offer" checkbox list and the post editor's target-language
//! picker. Not an exhaustive ISO 639/3166 registry — just the languages an
//! admin is realistically likely to want to translate a site into.

static LOCALES: &[(&str, &str)] = &[
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("ja", "Japanese"),
    ("zh-Hans", "Chinese (Simplified)"),
    ("zh-Hant", "Chinese (Traditional)"),
    ("ko", "Korean"),
    ("ar", "Arabic"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("bn", "Bengali"),
    ("ur", "Urdu"),
    ("tr", "Turkish"),
    ("vi", "Vietnamese"),
    ("th", "Thai"),
    ("id", "Indonesian"),
    ("ms", "Malay"),
    ("tl", "Filipino"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("no", "Norwegian"),
    ("fi", "Finnish"),
    ("is", "Icelandic"),
    ("cs", "Czech"),
    ("sk", "Slovak"),
    ("hu", "Hungarian"),
    ("ro", "Romanian"),
    ("bg", "Bulgarian"),
    ("el", "Greek"),
    ("hr", "Croatian"),
    ("sr", "Serbian"),
    ("sl", "Slovenian"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("et", "Estonian"),
    ("sw", "Swahili"),
    ("af", "Afrikaans"),
    ("fa", "Persian"),
    ("ta", "Tamil"),
    ("te", "Telugu"),
    ("mr", "Marathi"),
    ("gu", "Gujarati"),
];

/// Every supported (locale code, English display name) pair.
pub fn all_locales() -> &'static [(&'static str, &'static str)] {
    LOCALES
}

/// The English display name for a locale code, if it's on the list.
pub fn display_name(code: &str) -> Option<&'static str> {
    LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_matches_expected_shape() {
        for (code, _) in LOCALES {
            let valid = code.split('-').enumerate().all(|(i, part)| match i {
                0 => part.len() == 2 && part.chars().all(|c| c.is_ascii_lowercase()),
                1 => {
                    (part.len() == 2 && part.chars().all(|c| c.is_ascii_uppercase()))
                        || (part.len() == 4 && part.chars().next().unwrap().is_ascii_uppercase())
                }
                _ => false,
            });
            assert!(valid, "locale code '{code}' doesn't match expected shape");
        }
    }

    #[test]
    fn no_duplicate_codes() {
        let mut codes: Vec<&str> = LOCALES.iter().map(|(c, _)| *c).collect();
        let original_len = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), original_len, "duplicate locale code found");
    }

    #[test]
    fn no_empty_names() {
        for (code, name) in LOCALES {
            assert!(
                !name.is_empty(),
                "locale '{code}' has an empty display name"
            );
        }
    }

    #[test]
    fn display_name_known_and_unknown_lookups() {
        assert_eq!(display_name("es"), Some("Spanish"));
        assert_eq!(display_name("pt-BR"), Some("Portuguese (Brazil)"));
        assert_eq!(display_name("xx-not-a-real-locale"), None);
    }
}
