/// Converts a string to a URL-safe slug.
///
/// Lowercases the input, replaces any non-alphanumeric character with a
/// hyphen, then collapses consecutive hyphens and trims leading/trailing
/// hyphens.
///
/// # Examples
/// ```
/// # use core::utils::slugify::slugify;
/// assert_eq!(slugify("Birthday Party"), "birthday-party");
/// assert_eq!(slugify("Hello, World!"), "hello-world");
/// assert_eq!(slugify("My  Post"), "my-post");
/// ```
///
/// TODO: Unicode normalisation (café → cafe) pre-WASM migration.
pub fn slugify(input: &str) -> String {
    let replaced: String = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    replaced
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Returns `true` if the string is a valid slug.
///
/// A valid slug is non-empty, contains only ASCII alphanumeric characters and
/// hyphens, and does not start or end with a hyphen.
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.contains(char::is_whitespace)
        && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slug() {
        assert_eq!(slugify("Birthday Party"), "birthday-party");
    }

    #[test]
    fn test_special_chars() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
    }

    #[test]
    fn test_double_spaces() {
        assert_eq!(slugify("My  Post"), "my-post");
    }

    #[test]
    fn test_already_slug() {
        assert_eq!(slugify("birthday-party"), "birthday-party");
    }

    #[test]
    fn test_valid_slug() {
        assert!(is_valid_slug("birthday-party"));
        assert!(is_valid_slug("hello-world-123"));
    }

    #[test]
    fn test_invalid_slug_whitespace() {
        assert!(!is_valid_slug("birthday party"));
        assert!(!is_valid_slug("birthday\tparty"));
    }

    #[test]
    fn test_invalid_slug_leading_trailing_hyphen() {
        assert!(!is_valid_slug("-birthday-party"));
        assert!(!is_valid_slug("birthday-party-"));
    }

    #[test]
    fn test_invalid_slug_empty() {
        assert!(!is_valid_slug(""));
    }
}
