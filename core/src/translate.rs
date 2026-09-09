//! Calls an admin-configured AI provider to translate a post's title,
//! excerpt, and content into another language. Hand-rolled directly against
//! `reqwest` rather than a third-party AI SDK crate — see
//! `crate::models::ai_provider` for why. Structured directly after
//! `crate::mail`'s per-provider dispatch pattern.

use std::time::Duration;

use serde::Deserialize;

use crate::models::ai_provider::AiProviderConfig;
use crate::models::post::Post;

/// The three flat, translatable fields of a plain post/page. Builder/
/// page-composition posts (a JSON block tree, not these three strings) are
/// out of scope — see `documentation/posts.md`'s translation section.
#[derive(Debug, Deserialize)]
pub struct TranslationResult {
    pub title: String,
    pub excerpt: Option<String>,
    pub content: String,
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);

fn http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?)
}

fn build_prompt(post: &Post, target_locale_name: &str) -> String {
    let format_instruction = if post.content_format == "markdown" {
        "The content is Markdown — preserve all Markdown syntax (headings, links, lists, emphasis, code fences, etc.) exactly as-is and translate only the prose text."
    } else {
        "The content is HTML — preserve every HTML tag and attribute byte-for-byte and translate only the human-readable text nodes and any alt/title attribute values."
    };

    let excerpt_line = post
        .excerpt
        .as_deref()
        .map(|e| format!("EXCERPT:\n{e}\n\n"))
        .unwrap_or_default();

    format!(
        "Translate the following blog post into {target_locale_name}. {format_instruction}\n\n\
        Keep the same tone and register as the original. Do not add, remove, or summarize content — translate it faithfully.\n\n\
        TITLE:\n{title}\n\n{excerpt_line}CONTENT:\n{content}\n\n\
        Respond with ONLY a single JSON object, no markdown code fence, no commentary before or after it, matching exactly this shape:\n\
        {{\"title\": \"...\", \"excerpt\": \"...\" or null, \"content\": \"...\"}}",
        title = post.title,
        content = post.content,
    )
}

/// Strip a defensive ```json ... ``` fence if the model added one despite
/// being told not to — some models do this reflexively for JSON output.
fn strip_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s))
        .unwrap_or(trimmed)
        .trim()
}

fn parse_translation_result(text: &str) -> anyhow::Result<TranslationResult> {
    let json = strip_code_fence(text);
    serde_json::from_str(json).map_err(|e| {
        anyhow::anyhow!("could not parse translation response as JSON: {e}\nresponse was: {json}")
    })
}

/// Translate `post`'s title/excerpt/content into `target_locale_name` (an
/// English display name, e.g. "Spanish" — see `utils::locales`).
pub async fn translate_post(
    config: &AiProviderConfig,
    post: &Post,
    target_locale_name: &str,
) -> anyhow::Result<TranslationResult> {
    let prompt = build_prompt(post, target_locale_name);
    let text = send_prompt(config, &prompt, true).await?;
    parse_translation_result(&text)
}

/// A trivial round-trip call to confirm the provider's credentials and
/// endpoint actually work, mirroring `mail::send_test_email`'s role —
/// awaited synchronously from an admin's "Test" click, not spawned.
pub async fn test_provider(config: &AiProviderConfig) -> anyhow::Result<()> {
    let text = send_prompt(config, "Reply with only the word OK.", false).await?;
    if !text.trim().eq_ignore_ascii_case("OK") {
        anyhow::bail!("provider returned an unexpected response instead of OK");
    }
    Ok(())
}

async fn send_prompt(
    config: &AiProviderConfig,
    prompt: &str,
    require_json: bool,
) -> anyhow::Result<String> {
    match config {
        AiProviderConfig::Anthropic {
            api_key,
            model_name,
        } => send_via_anthropic(api_key, model_name, prompt).await,
        AiProviderConfig::OpenaiCompatible {
            base_url,
            api_key,
            model_name,
        } => send_via_openai_compatible(base_url, api_key, model_name, prompt, require_json).await,
    }
}

async fn send_via_anthropic(
    api_key: &str,
    model_name: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let body = serde_json::json!({
        "model": model_name,
        "max_tokens": 8192,
        "messages": [{ "role": "user", "content": prompt }],
    });

    let resp = http_client()?
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic request failed ({status}): {body}");
    }

    let body: serde_json::Value = resp.json().await?;
    body["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find_map(|b| b["text"].as_str()))
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Anthropic response had no text content block"))
}

async fn send_via_openai_compatible(
    base_url: &str,
    api_key: &str,
    model_name: &str,
    prompt: &str,
    require_json: bool,
) -> anyhow::Result<String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut body = serde_json::json!({
        "model": model_name,
        "messages": [{ "role": "user", "content": prompt }],
    });
    if require_json {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }

    let mut req = http_client()?.post(&url).json(&body);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI-compatible request failed ({status}): {body}");
    }

    let body: serde_json::Value = resp.json().await?;
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("OpenAI-compatible response had no message content"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_code_fence_removes_json_fence() {
        let input = "```json\n{\"title\": \"hi\"}\n```";
        assert_eq!(strip_code_fence(input), "{\"title\": \"hi\"}");
    }

    #[test]
    fn strip_code_fence_removes_bare_fence() {
        let input = "```\n{\"title\": \"hi\"}\n```";
        assert_eq!(strip_code_fence(input), "{\"title\": \"hi\"}");
    }

    #[test]
    fn strip_code_fence_leaves_unfenced_text_alone() {
        let input = "{\"title\": \"hi\"}";
        assert_eq!(strip_code_fence(input), "{\"title\": \"hi\"}");
    }

    #[test]
    fn parse_translation_result_parses_clean_json() {
        let text = r#"{"title": "Hola", "excerpt": "Un resumen", "content": "<p>Contenido</p>"}"#;
        let result = parse_translation_result(text).unwrap();
        assert_eq!(result.title, "Hola");
        assert_eq!(result.excerpt.as_deref(), Some("Un resumen"));
        assert_eq!(result.content, "<p>Contenido</p>");
    }

    #[test]
    fn parse_translation_result_handles_null_excerpt() {
        let text = r#"{"title": "Hola", "excerpt": null, "content": "<p>Contenido</p>"}"#;
        let result = parse_translation_result(text).unwrap();
        assert_eq!(result.excerpt, None);
    }

    #[test]
    fn parse_translation_result_strips_fence_before_parsing() {
        let text = "```json\n{\"title\": \"Hola\", \"excerpt\": null, \"content\": \"hi\"}\n```";
        let result = parse_translation_result(text).unwrap();
        assert_eq!(result.title, "Hola");
    }

    #[test]
    fn parse_translation_result_errors_on_garbage() {
        assert!(parse_translation_result("not json at all").is_err());
    }

    #[test]
    fn build_prompt_preserves_html_instruction_for_html_posts() {
        let post = make_post("html", "<p>Hello</p>", None);
        let prompt = build_prompt(&post, "Spanish");
        assert!(prompt.contains("HTML"));
        assert!(prompt.contains("<p>Hello</p>"));
    }

    #[test]
    fn build_prompt_preserves_markdown_instruction_for_markdown_posts() {
        let post = make_post("markdown", "# Hello", None);
        let prompt = build_prompt(&post, "Spanish");
        assert!(prompt.contains("Markdown"));
    }

    #[test]
    fn build_prompt_omits_excerpt_section_when_none() {
        let post = make_post("html", "<p>Hello</p>", None);
        let prompt = build_prompt(&post, "Spanish");
        assert!(!prompt.contains("EXCERPT:"));
    }

    #[test]
    fn build_prompt_includes_excerpt_when_present() {
        let post = make_post("html", "<p>Hello</p>", Some("A summary"));
        let prompt = build_prompt(&post, "Spanish");
        assert!(prompt.contains("EXCERPT:"));
        assert!(prompt.contains("A summary"));
    }

    fn make_post(content_format: &str, content: &str, excerpt: Option<&str>) -> Post {
        use chrono::Utc;
        use uuid::Uuid;
        Post {
            id: Uuid::new_v4(),
            site_id: Some(Uuid::new_v4()),
            title: "Hello World".to_string(),
            slug: "hello-world".to_string(),
            content: content.to_string(),
            content_format: content_format.to_string(),
            excerpt: excerpt.map(str::to_string),
            status: "published".to_string(),
            post_type: "post".to_string(),
            author_id: Uuid::new_v4(),
            featured_image_id: None,
            published_at: Some(Utc::now()),
            scheduled_at: None,
            submitted_at: None,
            template: None,
            post_password: None,
            comments_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_id: None,
            sources: serde_json::Value::Array(vec![]),
            sources_public: false,
        }
    }
}
