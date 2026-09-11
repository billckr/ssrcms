//! Keeps reusable form/poll placement separate from translated prose.
//!
//! The source post is authoritative for embed identity and order. A
//! translation supplies prose only; at render time its old embed markers
//! are removed and the exact source markers are inserted at the equivalent
//! structural token boundaries. This means an embed-only post edit never
//! needs another model call.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Tag(String),
    Text,
}

#[derive(Debug)]
struct Token {
    kind: TokenKind,
    end: usize,
}

fn embed_regex() -> anyhow::Result<regex_lite::Regex> {
    Ok(regex_lite::Regex::new(
        r#"<ss-form\b[^>]*></ss-form>|<ss-poll\b[^>]*></ss-poll>"#,
    )?)
}

fn tokens(content: &str) -> Vec<Token> {
    let Ok(tag_re) = regex_lite::Regex::new(r#"<[^>]+>"#) else {
        return Vec::new();
    };
    let Ok(name_re) = regex_lite::Regex::new(r#"^<\s*(/?)\s*([A-Za-z0-9-]+)"#) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut cursor = 0;
    for tag in tag_re.find_iter(content) {
        if !content[cursor..tag.start()].trim().is_empty() {
            result.push(Token {
                kind: TokenKind::Text,
                end: tag.start(),
            });
        }
        let signature = name_re
            .captures(tag.as_str())
            .map(|capture| format!("{}{}", &capture[1], capture[2].to_ascii_lowercase()))
            .unwrap_or_else(|| tag.as_str().to_string());
        result.push(Token {
            kind: TokenKind::Tag(signature),
            end: tag.end(),
        });
        cursor = tag.end();
    }
    if !content[cursor..].trim().is_empty() {
        result.push(Token {
            kind: TokenKind::Text,
            end: content.len(),
        });
    }
    result
}

/// Remove only the inert reusable-resource markers. Used to decide whether
/// a post save changed translatable prose or only embed placement.
pub fn without_embeds(content: &str) -> String {
    embed_regex()
        .map(|re| re.replace_all(content, "").to_string())
        .unwrap_or_else(|_| content.to_string())
}

/// Replace all markers in translated content with the exact markers and
/// structural placement from the source.
pub fn reconcile(source: &str, translated: &str) -> anyhow::Result<String> {
    let re = embed_regex()?;
    let mut source_without = String::with_capacity(source.len());
    let mut placements: Vec<(usize, String)> = Vec::new();
    let mut source_cursor = 0;
    let mut token_count = 0;
    for marker in re.find_iter(source) {
        let segment = &source[source_cursor..marker.start()];
        token_count += tokens(segment).len();
        source_without.push_str(segment);
        placements.push((token_count, marker.as_str().to_string()));
        source_cursor = marker.end();
    }
    source_without.push_str(&source[source_cursor..]);

    let translated_without = re.replace_all(translated, "").to_string();
    let source_tokens = tokens(&source_without);
    let translated_tokens = tokens(&translated_without);
    let source_shape: Vec<_> = source_tokens.iter().map(|token| &token.kind).collect();
    let translated_shape: Vec<_> = translated_tokens.iter().map(|token| &token.kind).collect();
    if source_shape != translated_shape {
        anyhow::bail!("source and translated content have different structural shapes");
    }

    let mut at_position: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (slot, marker) in placements {
        let position = if slot == 0 {
            0
        } else {
            translated_tokens
                .get(slot - 1)
                .map(|token| token.end)
                .ok_or_else(|| anyhow::anyhow!("embed placement is outside translated content"))?
        };
        at_position.entry(position).or_default().push(marker);
    }

    let mut result = String::with_capacity(translated_without.len() + source.len() / 8);
    let mut cursor = 0;
    for (position, markers) in at_position {
        result.push_str(&translated_without[cursor..position]);
        for marker in markers {
            result.push_str(&marker);
        }
        cursor = position;
    }
    result.push_str(&translated_without[cursor..]);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_a_new_source_embed_without_touching_translated_prose() {
        let source =
            r#"<p>Please contact us.</p><ss-form data-slug="contact"></ss-form><p>Thanks.</p>"#;
        let translated = r#"<p>Contáctenos.</p><p>Gracias.</p>"#;
        assert_eq!(
            reconcile(source, translated).unwrap(),
            r#"<p>Contáctenos.</p><ss-form data-slug="contact"></ss-form><p>Gracias.</p>"#
        );
    }

    #[test]
    fn source_identity_replaces_old_translated_markers() {
        let source = r#"<p>A</p><ss-poll data-slug="new"></ss-poll>"#;
        let translated = r#"<p>B</p><ss-poll data-slug="old"></ss-poll>"#;
        assert_eq!(
            reconcile(source, translated).unwrap(),
            r#"<p>B</p><ss-poll data-slug="new"></ss-poll>"#
        );
    }

    #[test]
    fn different_document_shapes_are_rejected() {
        assert!(reconcile(
            r#"<p>A</p><ss-form data-slug="x"></ss-form>"#,
            "<div><p>B</p></div>"
        )
        .is_err());
    }

    #[test]
    fn removing_source_embed_removes_it_from_translation() {
        assert_eq!(
            reconcile("<p>A</p>", r#"<p>B</p><ss-form data-slug="x"></ss-form>"#).unwrap(),
            "<p>B</p>"
        );
    }
}
