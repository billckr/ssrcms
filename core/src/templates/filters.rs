//! Core Tera filters registered by the template engine.
//! All filters documented in docs/plugin-api-v1.md §5.

use std::collections::HashMap;
use tera::{Result, Value};

/// Strip all HTML tags from `html`, returning plain text.
///
/// Uses ammonia with an empty allowed-tag set so that all tags are removed
/// and only text content is returned. This is distinct from `ammonia::clean_text()`
/// which HTML-entity-encodes the markup rather than stripping it.
fn strip_tags(html: &str) -> String {
    ammonia::Builder::new()
        .tags(Default::default())
        .clean(html)
        .to_string()
}

/// `{{ value | date_format(format="%B %-d, %Y") }}`
/// Formats an ISO 8601 datetime string using a strftime format string.
pub fn date_format(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let dt_str = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("date_format requires a string value"))?;

    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("%B %-d, %Y");

    let dt = chrono::DateTime::parse_from_rfc3339(dt_str)
        .map_err(|e| tera::Error::msg(format!("date_format: invalid datetime '{dt_str}': {e}")))?;

    Ok(Value::String(dt.format(format).to_string()))
}

/// `{{ value | excerpt(words=55) }}`
/// Strips HTML and truncates to N words, appending "..." if truncated.
pub fn excerpt(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let html = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("excerpt requires a string value"))?;

    let words_n = args
        .get("words")
        .and_then(|v| v.as_u64())
        .unwrap_or(55) as usize;

    let text = strip_tags(html);
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.len() <= words_n {
        Ok(Value::String(words.join(" ")))
    } else {
        Ok(Value::String(format!("{} ...", words[..words_n].join(" "))))
    }
}

/// `{{ value | strip_html }}`
/// Remove all HTML tags, returning plain text.
pub fn strip_html(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let html = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("strip_html requires a string value"))?;
    Ok(Value::String(strip_tags(html)))
}

/// `{{ value | reading_time(wpm=200) }}`
/// Estimate reading time in minutes (minimum 1).
pub fn reading_time(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("reading_time requires a string value"))?;

    let wpm = args
        .get("wpm")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;

    let plain = strip_tags(text);
    let word_count = plain.split_whitespace().count();
    let minutes = ((word_count as f64 / wpm as f64).ceil() as u64).max(1);

    Ok(Value::Number(minutes.into()))
}

/// `{{ value | slugify }}`
/// Convert a string to a URL-safe slug.
pub fn slugify(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("slugify requires a string value"))?;
    Ok(Value::String(slug::slugify(s)))
}

/// `{{ value | truncate_words(count=20) }}`
/// Truncate to N words (no ellipsis).
pub fn truncate_words(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("truncate_words requires a string value"))?;

    let count = args
        .get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;

    let words: Vec<&str> = s.split_whitespace().take(count).collect();
    Ok(Value::String(words.join(" ")))
}

/// `{{ value | absolute_url }}`
/// Prepend `site.url` to a relative path. Requires `site_url` in args (injected by the engine).
pub fn absolute_url(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let path = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("absolute_url requires a string value"))?;

    let base = args
        .get("site_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let path = path.trim_start_matches('/');
    Ok(Value::String(format!("{}/{}", base.trim_end_matches('/'), path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_args() -> HashMap<String, Value> {
        HashMap::new()
    }

    fn args_with(key: &str, val: Value) -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert(key.to_string(), val);
        m
    }

    // --- date_format ---

    #[test]
    fn date_format_default_format() {
        let val = json!("2024-03-15T12:00:00Z");
        let result = date_format(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "March 15, 2024");
    }

    #[test]
    fn date_format_custom_format() {
        let val = json!("2024-03-15T12:00:00Z");
        let args = args_with("format", json!("%Y-%m-%d"));
        let result = date_format(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "2024-03-15");
    }

    #[test]
    fn date_format_invalid_input_returns_error() {
        let val = json!("not-a-date");
        assert!(date_format(&val, &no_args()).is_err());
    }

    // --- excerpt ---

    #[test]
    fn excerpt_truncates_at_55_words_with_ellipsis() {
        let content = "<p>".to_string() + &"word ".repeat(60) + "</p>";
        let val = json!(content);
        let result = excerpt(&val, &no_args()).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.ends_with(" ..."), "expected ' ...' suffix, got: {s:?}");
        let word_count = s.trim_end_matches(" ...").split_whitespace().count();
        assert_eq!(word_count, 55);
    }

    #[test]
    fn excerpt_short_text_returned_unmodified() {
        let val = json!("Hello world");
        let result = excerpt(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "Hello world");
    }

    #[test]
    fn excerpt_custom_words_arg() {
        let val = json!("one two three four five");
        let args = args_with("words", json!(3u64));
        let result = excerpt(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "one two three ...");
    }

    // --- strip_html ---

    #[test]
    fn strip_html_removes_paragraph_tags() {
        let val = json!("<p>Hello</p>");
        let result = strip_html(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "Hello");
    }

    #[test]
    fn strip_html_removes_nested_tags() {
        let val = json!("<div><strong>Bold</strong> and <em>italic</em></div>");
        let result = strip_html(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "Bold and italic");
    }

    #[test]
    fn strip_html_empty_string() {
        let val = json!("");
        let result = strip_html(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "");
    }

    // --- reading_time ---

    #[test]
    fn reading_time_200_words_is_1_min() {
        let val = json!("word ".repeat(200));
        let result = reading_time(&val, &no_args()).unwrap();
        assert_eq!(result.as_u64().unwrap(), 1);
    }

    #[test]
    fn reading_time_100_words_is_1_min_minimum() {
        let val = json!("word ".repeat(100));
        let result = reading_time(&val, &no_args()).unwrap();
        assert_eq!(result.as_u64().unwrap(), 1);
    }

    #[test]
    fn reading_time_400_words_is_2_min() {
        let val = json!("word ".repeat(400));
        let result = reading_time(&val, &no_args()).unwrap();
        assert_eq!(result.as_u64().unwrap(), 2);
    }

    #[test]
    fn reading_time_empty_is_1_min() {
        let val = json!("");
        let result = reading_time(&val, &no_args()).unwrap();
        assert_eq!(result.as_u64().unwrap(), 1);
    }

    // --- slugify ---

    #[test]
    fn slugify_spaces_become_hyphens() {
        let val = json!("Hello World");
        let result = slugify(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "hello-world");
    }

    #[test]
    fn slugify_special_chars_stripped() {
        let val = json!("Hello, World!");
        let result = slugify(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "hello-world");
    }

    #[test]
    fn slugify_already_lowercase_passthrough() {
        let val = json!("my-blog-post");
        let result = slugify(&val, &no_args()).unwrap();
        assert_eq!(result.as_str().unwrap(), "my-blog-post");
    }

    // --- truncate_words ---

    #[test]
    fn truncate_words_truncates_at_n() {
        let val = json!("one two three four five");
        let args = args_with("count", json!(3u64));
        let result = truncate_words(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "one two three");
    }

    #[test]
    fn truncate_words_fewer_than_n_unchanged() {
        let val = json!("one two");
        let args = args_with("count", json!(10u64));
        let result = truncate_words(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "one two");
    }

    #[test]
    fn truncate_words_count_zero_returns_empty() {
        let val = json!("one two three");
        let args = args_with("count", json!(0u64));
        let result = truncate_words(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "");
    }

    // --- absolute_url ---

    #[test]
    fn absolute_url_basic() {
        let val = json!("/blog");
        let args = args_with("site_url", json!("https://example.com"));
        let result = absolute_url(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "https://example.com/blog");
    }

    #[test]
    fn absolute_url_trailing_slash_on_base() {
        let val = json!("/about");
        let args = args_with("site_url", json!("https://example.com/"));
        let result = absolute_url(&val, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "https://example.com/about");
    }
}
