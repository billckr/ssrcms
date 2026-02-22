//! Core Tera filters registered by the template engine.
//! All filters documented in docs/plugin-api-v1.md §5.

use std::collections::HashMap;
use tera::{Result, Value};

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

    let text = ammonia::clean_text(html);
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
    Ok(Value::String(ammonia::clean_text(html)))
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

    let plain = ammonia::clean_text(text);
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
