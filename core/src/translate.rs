//! Calls an admin-configured AI provider to translate a post's title,
//! excerpt, and content into another language. Hand-rolled directly against
//! `reqwest` rather than a third-party AI SDK crate — see
//! `crate::models::ai_provider` for why. Structured directly after
//! `crate::mail`'s per-provider dispatch pattern.

use std::time::Duration;

use serde::{Deserialize, Serialize};

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

/// One model returned by a provider's model-discovery endpoint. Providers
/// expose identifiers differently, so the admin UI receives both the stable
/// API ID and the best human-readable name available.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AvailableModel {
    pub id: String,
    pub display_name: String,
    pub cost_tier: Option<&'static str>,
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
        anyhow::anyhow!(
            "could not parse translation response as JSON: {e}; response_excerpt={}",
            concise_error(json)
        )
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

/// A small structured-output call confirming that credentials, model, and
/// the response shape used by translation all work. Awaited synchronously
/// from an admin's "Test" click, not spawned.
pub async fn test_provider(config: &AiProviderConfig) -> anyhow::Result<()> {
    let text = send_prompt(
        config,
        "Reply with ONLY this JSON object, with no markdown fence or commentary: {\"title\":\"Test\",\"excerpt\":null,\"content\":\"Test\"}",
        true,
    )
    .await?;
    parse_translation_result(&text)
        .map_err(|e| anyhow::anyhow!("provider did not return translation-compatible JSON: {e}"))?;
    Ok(())
}

/// Ask the configured service which models are available to its credential.
/// This is deliberately live rather than a hard-coded catalog: access can
/// vary by account and both hosted and local providers change over time.
pub async fn discover_models(config: &AiProviderConfig) -> anyhow::Result<Vec<AvailableModel>> {
    let mut models = match config {
        AiProviderConfig::Anthropic { api_key, .. } => discover_anthropic_models(api_key).await?,
        AiProviderConfig::OpenaiCompatible {
            base_url, api_key, ..
        } => discover_openai_compatible_models(base_url, api_key).await?,
    };
    models.sort_by(|a, b| {
        cost_tier_rank(a.cost_tier)
            .cmp(&cost_tier_rank(b.cost_tier))
            .then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
    });
    models.dedup_by(|a, b| a.id == b.id);
    Ok(models)
}

fn cost_tier_rank(tier: Option<&str>) -> u8 {
    match tier {
        Some("Economy") => 0,
        Some("Balanced") => 1,
        Some("Premium") => 2,
        _ => 3,
    }
}

fn inferred_cost_tier(id: &str) -> Option<&'static str> {
    let id = id.to_ascii_lowercase();
    let has_token = |wanted: &str| {
        id.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|part| part == wanted)
    };
    if id.contains("haiku") || has_token("nano") || has_token("mini") {
        Some("Economy")
    } else if id.contains("sonnet") {
        Some("Balanced")
    } else if id.contains("opus") || has_token("pro") {
        Some("Premium")
    } else {
        None
    }
}

async fn discover_anthropic_models(api_key: &str) -> anyhow::Result<Vec<AvailableModel>> {
    let resp = http_client()?
        .get("https://api.anthropic.com/v1/models?limit=1000")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "Anthropic model discovery failed ({status}): {}",
            concise_error(&body)
        );
    }

    #[derive(Deserialize)]
    struct ModelList {
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        display_name: Option<String>,
    }

    let body: ModelList = resp.json().await?;
    Ok(body
        .data
        .into_iter()
        .map(|model| AvailableModel {
            cost_tier: inferred_cost_tier(&model.id),
            display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
        })
        .collect())
}

async fn discover_openai_compatible_models(
    base_url: &str,
    api_key: &str,
) -> anyhow::Result<Vec<AvailableModel>> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = http_client()?.get(url);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "model discovery failed ({status}): {}",
            concise_error(&body)
        );
    }

    #[derive(Deserialize)]
    struct ModelList {
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        #[serde(default)]
        name: Option<String>,
    }

    let body: ModelList = resp.json().await?;
    Ok(body
        .data
        .into_iter()
        .map(|model| AvailableModel {
            cost_tier: inferred_cost_tier(&model.id),
            display_name: model.name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
        })
        .collect())
}

fn concise_error(body: &str) -> String {
    const MAX_CHARS: usize = 500;
    let cleaned = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= MAX_CHARS {
        cleaned
    } else {
        format!("{}…", cleaned.chars().take(MAX_CHARS).collect::<String>())
    }
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
        anyhow::bail!(
            "Anthropic request failed ({status}): {}",
            concise_error(&body)
        );
    }

    let response_text = resp.text().await?;
    let body: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
        anyhow::anyhow!(
            "Anthropic response was not valid JSON: {e}; response_excerpt={}",
            concise_error(&response_text)
        )
    })?;
    body["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find_map(|b| b["text"].as_str()))
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Anthropic response had no text content block; response_excerpt={}",
                concise_error(&response_text)
            )
        })
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
        anyhow::bail!(
            "OpenAI-compatible request failed ({status}): {}",
            concise_error(&body)
        );
    }

    let response_text = resp.text().await?;
    let body: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
        anyhow::anyhow!(
            "OpenAI-compatible response was not valid JSON: {e}; response_excerpt={}",
            concise_error(&response_text)
        )
    })?;
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "OpenAI-compatible response had no message content; response_excerpt={}",
                concise_error(&response_text)
            )
        })
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
    fn parse_translation_error_bounds_and_flattens_response_excerpt() {
        let response = format!("not json\n{}", "x".repeat(600));
        let error = parse_translation_result(&response).unwrap_err().to_string();
        assert!(!error.contains('\n'));
        assert!(error.contains("response_excerpt=not json "));
        assert!(error.ends_with('…'));
        assert!(error.chars().count() < 650);
    }

    #[test]
    fn cost_tiers_cover_known_families_without_matching_preview() {
        assert_eq!(inferred_cost_tier("claude-haiku-4-5"), Some("Economy"));
        assert_eq!(inferred_cost_tier("gpt-5-mini"), Some("Economy"));
        assert_eq!(inferred_cost_tier("claude-sonnet-5"), Some("Balanced"));
        assert_eq!(inferred_cost_tier("claude-opus-5"), Some("Premium"));
        assert_eq!(inferred_cost_tier("gpt-5-pro"), Some("Premium"));
        assert_eq!(inferred_cost_tier("gpt-4o-preview"), None);
    }

    #[test]
    fn concise_errors_are_single_line_and_bounded() {
        assert_eq!(concise_error("one\n  two\tthree"), "one two three");
        let long = "x".repeat(600);
        assert_eq!(concise_error(&long).chars().count(), 501);
        assert!(concise_error(&long).ends_with('…'));
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
