//! Calls an admin-configured AI provider to translate a post's title,
//! excerpt, and content into another language. Hand-rolled directly against
//! `reqwest` rather than a third-party AI SDK crate — see
//! `crate::models::ai_provider` for why. Structured directly after
//! `crate::mail`'s per-provider dispatch pattern.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::ai_provider::AiProviderConfig;
use crate::models::embedded_translation::{FormTranslationPayload, PollTranslationPayload};
use crate::models::form_def::FormDef;
use crate::models::poll_def::PollDef;
use crate::models::post::Post;

/// DeepSeek's fixed API base — unlike `OpenaiCompatible`, whose base URL is
/// user-entered, this is a first-class provider with one known endpoint.
const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";

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

// ── Abuse / cost controls ────────────────────────────────────────────────
//
// Unlike ordinary CRUD, every translation call spends real money against a
// paid third-party API, so it needs a ceiling independent of the normal
// admin-auth/CSRF checks already gating these routes — a compromised or
// merely overeager admin account (global or site-scoped) could otherwise
// script repeated calls to run up provider spend. Enforced once per actual
// provider call (inside `translate_post`/`translate_form`/`translate_poll`,
// not once per HTTP request), so translating N embedded forms/polls in one
// admin click correctly consumes N units — matching the real cost impact —
// rather than 1.
//
// Process-local, same tradeoff as `middleware::auth_security`: fine for the
// current single-instance deployment, move to PostgreSQL/Redis before
// running multiple app instances.

/// Hard cap on how much source text (a post's title+excerpt+content, or the
/// equivalent flattened form/poll JSON) is sent to a provider in one call.
/// Comfortably covers any real post, page, form, or poll; guards against a
/// single oversized document driving up per-call token cost.
const MAX_SOURCE_CHARS: usize = 50_000;

/// Per-site ceiling on provider calls per rolling hour — generous enough to
/// bulk-translate an existing site's content into a few locales in one
/// sitting, tight enough to bound a compromised/scripted account's blast
/// radius against the site's own configured (and billed) provider.
const TRANSLATIONS_PER_SITE_PER_HOUR: usize = 50;

struct TranslationBucket {
    calls: Vec<Instant>,
}

static SITE_TRANSLATION_CALLS: Lazy<Mutex<HashMap<Uuid, TranslationBucket>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Returns `false` once `site_id` has made `TRANSLATIONS_PER_SITE_PER_HOUR`
/// calls within the last hour; otherwise records this call and returns `true`.
fn consume_translation_slot(site_id: Uuid) -> bool {
    let now = Instant::now();
    let window = Duration::from_secs(60 * 60);
    let mut buckets = SITE_TRANSLATION_CALLS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if buckets.len() >= 10_000 {
        buckets.retain(|_, bucket| {
            bucket.calls.retain(|at| now.duration_since(*at) < window);
            !bucket.calls.is_empty()
        });
        // Bound memory even during a distributed identifier-flood attack.
        if buckets.len() >= 10_000 && !buckets.contains_key(&site_id) {
            return false;
        }
    }
    let bucket = buckets
        .entry(site_id)
        .or_insert_with(|| TranslationBucket { calls: Vec::new() });
    bucket.calls.retain(|at| now.duration_since(*at) < window);
    if bucket.calls.len() >= TRANSLATIONS_PER_SITE_PER_HOUR {
        return false;
    }
    bucket.calls.push(now);
    true
}

/// Call before sending any prompt to a provider. Checks source length first
/// (a no-op rejection, since it can't be worked around by retrying) so an
/// oversized document doesn't also burn a rate-limit slot.
fn enforce_translation_limits(site_id: Uuid, source_chars: usize) -> anyhow::Result<()> {
    if source_chars > MAX_SOURCE_CHARS {
        anyhow::bail!(
            "content is too long to translate automatically ({source_chars} characters, max {MAX_SOURCE_CHARS})"
        );
    }
    if !consume_translation_slot(site_id) {
        anyhow::bail!(
            "translation rate limit reached for this site ({TRANSLATIONS_PER_SITE_PER_HOUR} per hour) — try again later"
        );
    }
    Ok(())
}

fn http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?)
}

fn build_prompt_with_content(
    post: &Post,
    target_locale_name: &str,
    content: &str,
    embed_instruction: &str,
) -> String {
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
        "Translate the following blog post into {target_locale_name}. {format_instruction}{embed_instruction}\n\n\
        Keep the same tone and register as the original. Do not add, remove, or summarize content — translate it faithfully.\n\n\
        TITLE:\n{title}\n\n{excerpt_line}CONTENT:\n{content}\n\n\
        Respond with ONLY a single JSON object, no markdown code fence, no commentary before or after it, matching exactly this shape:\n\
        {{\"title\": \"...\", \"excerpt\": \"...\" or null, \"content\": \"...\"}}",
        title = post.title,
        content = content,
    )
}

#[cfg(test)]
fn build_prompt(post: &Post, target_locale_name: &str) -> String {
    build_prompt_with_content(post, target_locale_name, &post.content, "")
}

#[derive(Debug)]
struct ProtectedEmbeds {
    content: String,
    markers: Vec<String>,
}

/// Keep reusable-content identity outside the model's control. The provider
/// sees opaque tokens rather than editable `<ss-form>`/`<ss-poll>` tags; the
/// exact source markers are restored only after cardinality/order validation.
fn protect_embeds(content: &str) -> anyhow::Result<ProtectedEmbeds> {
    let re = regex_lite::Regex::new(r#"<ss-form\b[^>]*></ss-form>|<ss-poll\b[^>]*></ss-poll>"#)?;
    let matches: Vec<_> = re.find_iter(content).collect();
    if matches.is_empty() {
        return Ok(ProtectedEmbeds {
            content: content.to_string(),
            markers: Vec::new(),
        });
    }

    let mut protected = String::with_capacity(content.len());
    let mut markers = Vec::with_capacity(matches.len());
    let mut cursor = 0;
    for (index, found) in matches.into_iter().enumerate() {
        protected.push_str(&content[cursor..found.start()]);
        protected.push_str(&format!("[[SYNAPCMS_EMBED_{index}]]"));
        markers.push(found.as_str().to_string());
        cursor = found.end();
    }
    protected.push_str(&content[cursor..]);
    Ok(ProtectedEmbeds {
        content: protected,
        markers,
    })
}

fn restore_embeds(content: &str, markers: &[String]) -> anyhow::Result<String> {
    let mut previous = 0;
    for index in 0..markers.len() {
        let token = format!("[[SYNAPCMS_EMBED_{index}]]");
        if content.matches(&token).count() != 1 {
            anyhow::bail!("translation changed or removed an embedded form/poll marker");
        }
        let position = content.find(&token).unwrap_or_default();
        if index > 0 && position < previous {
            anyhow::bail!("translation reordered embedded form/poll markers");
        }
        previous = position;
    }
    if content.contains("[[SYNAPCMS_EMBED_") && markers.is_empty() {
        anyhow::bail!("translation introduced an unknown embedded-content marker");
    }

    let mut restored = content.to_string();
    for (index, marker) in markers.iter().enumerate() {
        restored = restored.replace(&format!("[[SYNAPCMS_EMBED_{index}]]"), marker);
    }
    if restored.contains("[[SYNAPCMS_EMBED_") {
        anyhow::bail!("translation introduced an unknown embedded-content marker");
    }
    Ok(restored)
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
    let Some(site_id) = post.site_id else {
        anyhow::bail!("post has no site");
    };
    let source_chars = post.title.chars().count()
        + post.excerpt.as_deref().unwrap_or_default().chars().count()
        + post.content.chars().count();
    enforce_translation_limits(site_id, source_chars)?;

    let protected = protect_embeds(&post.content)?;
    let embed_instruction = if protected.markers.is_empty() {
        ""
    } else {
        " Preserve every [[SYNAPCMS_EMBED_N]] token exactly once and in the same order; these tokens represent forms or polls and must not be translated, removed, duplicated, or moved."
    };
    let prompt = build_prompt_with_content(
        post,
        target_locale_name,
        &protected.content,
        embed_instruction,
    );
    let text = send_prompt(config, &prompt, true).await?;
    let mut result = parse_translation_result(&text)?;
    result.content = restore_embeds(&result.content, &protected.markers)?;
    Ok(result)
}

pub async fn translate_form(
    config: &AiProviderConfig,
    form: &FormDef,
    target_locale_name: &str,
) -> anyhow::Result<FormTranslationPayload> {
    let source = serde_json::json!({
        "fields": form.fields.iter().map(|field| serde_json::json!({
            "name": field.name,
            "label": field.label,
            "options": field.options.iter().map(|(value, label)| serde_json::json!({
                "value": value,
                "label": label,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "button_label": form.settings.button_label,
        "success_message": form.settings.success_message,
        "invalid_message": "Please fill in all required fields with valid values and try again.",
        "confirm_subject": form.settings.confirm_subject,
        "confirm_body": form.settings.confirm_body,
    });
    enforce_translation_limits(form.site_id, source.to_string().chars().count())?;
    let prompt = format!(
        "Translate the visitor-facing strings in this form into {target_locale_name}. Keep every field name, option value, and {{{{field_name}}}} template placeholder byte-for-byte unchanged. Return ONLY JSON with this exact shape: {{\"fields\":[{{\"name\":\"...\",\"label\":\"...\",\"options\":[{{\"value\":\"...\",\"label\":\"...\"}}]}}],\"button_label\":\"...\",\"success_message\":\"...\",\"invalid_message\":\"...\",\"confirm_subject\":\"...\",\"confirm_body\":\"...\"}}. SOURCE JSON:\n{source}"
    );
    let text = send_prompt(config, &prompt, true).await?;
    let value: serde_json::Value = serde_json::from_str(strip_code_fence(&text)).map_err(|e| {
        anyhow::anyhow!(
            "could not parse form translation response as JSON: {e}; response_excerpt={}",
            concise_error(&text)
        )
    })?;
    form_payload_from_value(form, value)
}

fn form_payload_from_value(
    form: &FormDef,
    value: serde_json::Value,
) -> anyhow::Result<FormTranslationPayload> {
    #[derive(Deserialize)]
    struct OptionResult {
        value: String,
        label: String,
    }
    #[derive(Deserialize)]
    struct FieldResult {
        name: String,
        label: String,
        options: Vec<OptionResult>,
    }
    #[derive(Deserialize)]
    struct ResultShape {
        fields: Vec<FieldResult>,
        button_label: String,
        success_message: String,
        invalid_message: String,
        confirm_subject: String,
        confirm_body: String,
    }
    let result: ResultShape = serde_json::from_value(value)?;
    if result.fields.len() != form.fields.len() {
        anyhow::bail!("form translation changed the field count");
    }
    if template_placeholders(&result.confirm_subject)
        != template_placeholders(&form.settings.confirm_subject)
        || template_placeholders(&result.confirm_body)
            != template_placeholders(&form.settings.confirm_body)
    {
        anyhow::bail!("form translation changed a confirmation template placeholder");
    }
    let mut field_labels = BTreeMap::new();
    let mut option_labels = BTreeMap::new();
    for (source, translated) in form.fields.iter().zip(result.fields) {
        if translated.name != source.name || translated.options.len() != source.options.len() {
            anyhow::bail!("form translation changed a stable field name or option count");
        }
        field_labels.insert(source.name.clone(), translated.label);
        let mut labels = BTreeMap::new();
        for ((source_value, _), translated_option) in source.options.iter().zip(translated.options)
        {
            if translated_option.value != *source_value {
                anyhow::bail!("form translation changed a stable option value");
            }
            labels.insert(source_value.clone(), translated_option.label);
        }
        if !labels.is_empty() {
            option_labels.insert(source.name.clone(), labels);
        }
    }
    Ok(FormTranslationPayload {
        field_labels,
        option_labels,
        button_label: result.button_label,
        success_message: result.success_message,
        invalid_message: result.invalid_message,
        confirm_subject: result.confirm_subject,
        confirm_body: result.confirm_body,
    })
}

fn template_placeholders(text: &str) -> Vec<&str> {
    let Ok(re) = regex_lite::Regex::new(r"\{\{[A-Za-z0-9_-]+\}\}") else {
        return Vec::new();
    };
    re.find_iter(text).map(|found| found.as_str()).collect()
}

pub async fn translate_poll(
    config: &AiProviderConfig,
    poll: &PollDef,
    target_locale_name: &str,
) -> anyhow::Result<PollTranslationPayload> {
    let source = serde_json::json!({
        "question": poll.question,
        "options": poll.options.iter().map(|option| serde_json::json!({
            "key": option.key,
            "label": option.label,
        })).collect::<Vec<_>>(),
        "button_label": poll.settings.button_label,
        "success_message": poll.settings.success_message,
        "total_votes_label": "{count} total votes",
    });
    enforce_translation_limits(poll.site_id, source.to_string().chars().count())?;
    let prompt = format!(
        "Translate the visitor-facing strings in this poll into {target_locale_name}. Keep every option key byte-for-byte unchanged and preserve the {{count}} placeholder exactly once. Return ONLY JSON with this exact shape: {{\"question\":\"...\",\"options\":[{{\"key\":\"...\",\"label\":\"...\"}}],\"button_label\":\"...\",\"success_message\":\"...\",\"total_votes_label\":\"{{count}} ...\"}}. SOURCE JSON:\n{source}"
    );
    let text = send_prompt(config, &prompt, true).await?;
    let value: serde_json::Value = serde_json::from_str(strip_code_fence(&text)).map_err(|e| {
        anyhow::anyhow!(
            "could not parse poll translation response as JSON: {e}; response_excerpt={}",
            concise_error(&text)
        )
    })?;
    poll_payload_from_value(poll, value)
}

fn poll_payload_from_value(
    poll: &PollDef,
    value: serde_json::Value,
) -> anyhow::Result<PollTranslationPayload> {
    #[derive(Deserialize)]
    struct OptionResult {
        key: String,
        label: String,
    }
    #[derive(Deserialize)]
    struct ResultShape {
        question: String,
        options: Vec<OptionResult>,
        button_label: String,
        success_message: String,
        total_votes_label: String,
    }
    let result: ResultShape = serde_json::from_value(value)?;
    if result.options.len() != poll.options.len()
        || result.total_votes_label.matches("{count}").count() != 1
    {
        anyhow::bail!("poll translation changed its option count or count placeholder");
    }
    let mut option_labels = BTreeMap::new();
    for (source, translated) in poll.options.iter().zip(result.options) {
        if translated.key != source.key {
            anyhow::bail!("poll translation changed a stable option key");
        }
        option_labels.insert(source.key.clone(), translated.label);
    }
    Ok(PollTranslationPayload {
        question: result.question,
        option_labels,
        button_label: result.button_label,
        success_message: result.success_message,
        total_votes_label: result.total_votes_label,
    })
}

/// A small structured-output call confirming that credentials, model, and
/// the response shape used by translation all work. Awaited synchronously
/// from an admin's "Test" click, not spawned. Shares the same per-site
/// rate-limit bucket as actual translations (see "Abuse / cost controls"
/// above) — it's a real, billed call to the same provider, so it needs the
/// same ceiling; skipping the source-length check since there's no source
/// content here, just a fixed trivial prompt.
pub async fn test_provider(site_id: Uuid, config: &AiProviderConfig) -> anyhow::Result<()> {
    if !consume_translation_slot(site_id) {
        anyhow::bail!(
            "translation rate limit reached for this site ({TRANSLATIONS_PER_SITE_PER_HOUR} per hour) — try again later"
        );
    }
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
/// Shares the same per-site rate-limit bucket as `test_provider`/actual
/// translations — see its doc comment.
pub async fn discover_models(
    site_id: Uuid,
    config: &AiProviderConfig,
) -> anyhow::Result<Vec<AvailableModel>> {
    if !consume_translation_slot(site_id) {
        anyhow::bail!(
            "translation rate limit reached for this site ({TRANSLATIONS_PER_SITE_PER_HOUR} per hour) — try again later"
        );
    }
    let mut models = match config {
        AiProviderConfig::Anthropic { api_key, .. } => discover_anthropic_models(api_key).await?,
        AiProviderConfig::Deepseek { api_key, .. } => {
            discover_openai_compatible_models(DEEPSEEK_BASE_URL, api_key).await?
        }
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
    if id.contains("haiku") || has_token("nano") || has_token("mini") || id.contains("deepseek-chat")
    {
        Some("Economy")
    } else if id.contains("sonnet") {
        Some("Balanced")
    } else if id.contains("opus") || has_token("pro") || id.contains("deepseek-reasoner") {
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
        AiProviderConfig::Deepseek {
            api_key,
            model_name,
        } => {
            send_via_openai_compatible(DEEPSEEK_BASE_URL, api_key, model_name, prompt, require_json)
                .await
        }
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

    #[test]
    fn embedded_resources_are_hidden_from_model_and_restored_exactly() {
        let source = r#"<p>Hello</p><ss-form data-slug="contact" data-label="Contact"></ss-form><ss-poll data-slug="choice"></ss-poll>"#;
        let protected = protect_embeds(source).unwrap();
        assert_eq!(protected.markers.len(), 2);
        assert!(!protected.content.contains("contact"));
        assert_eq!(
            restore_embeds(&protected.content, &protected.markers).unwrap(),
            source
        );
    }

    #[test]
    fn embedded_resource_validation_rejects_missing_or_duplicate_tokens() {
        let protected = protect_embeds(
            r#"<ss-form data-slug="contact"></ss-form><ss-poll data-slug="choice"></ss-poll>"#,
        )
        .unwrap();
        assert!(restore_embeds("[[SYNAPCMS_EMBED_0]]", &protected.markers).is_err());
        assert!(restore_embeds(
            "[[SYNAPCMS_EMBED_0]][[SYNAPCMS_EMBED_0]][[SYNAPCMS_EMBED_1]]",
            &protected.markers
        )
        .is_err());
    }

    #[test]
    fn embedded_resource_validation_rejects_reordering() {
        let protected = protect_embeds(
            r#"<ss-form data-slug="contact"></ss-form><ss-poll data-slug="choice"></ss-poll>"#,
        )
        .unwrap();
        assert!(restore_embeds(
            "[[SYNAPCMS_EMBED_1]][[SYNAPCMS_EMBED_0]]",
            &protected.markers
        )
        .is_err());
    }

    #[test]
    fn form_translation_rejects_changed_submission_identifiers() {
        let form = make_form();
        let value = serde_json::json!({
            "fields": [{"name":"nombre","label":"Nombre","options":[{"value":"daily","label":"Diario"}]}],
            "button_label":"Enviar",
            "success_message":"Gracias",
            "invalid_message":"Revise el formulario",
            "confirm_subject":"Recibido",
            "confirm_body":"Gracias"
        });
        assert!(form_payload_from_value(&form, value).is_err());
    }

    #[test]
    fn form_translation_rejects_changed_email_placeholders() {
        let mut form = make_form();
        form.settings.confirm_body = "Hello {{frequency}}".to_string();
        let value = serde_json::json!({
            "fields": [{"name":"frequency","label":"Frecuencia","options":[{"value":"daily","label":"Diario"}]}],
            "button_label":"Enviar",
            "success_message":"Gracias",
            "invalid_message":"Revise el formulario",
            "confirm_subject":"Recibido",
            "confirm_body":"Hola {{frecuencia}}"
        });
        assert!(form_payload_from_value(&form, value).is_err());
    }

    #[test]
    fn poll_translation_rejects_changed_vote_keys() {
        let poll = make_poll();
        let value = serde_json::json!({
            "question":"¿Con qué frecuencia?",
            "options":[{"key":"diario","label":"Diario"}],
            "button_label":"Votar",
            "success_message":"Gracias",
            "total_votes_label":"{count} votos"
        });
        assert!(poll_payload_from_value(&poll, value).is_err());
    }

    fn make_form() -> FormDef {
        let now = chrono::Utc::now();
        FormDef {
            id: uuid::Uuid::new_v4(),
            site_id: uuid::Uuid::new_v4(),
            name: "Newsletter".to_string(),
            slug: "newsletter".to_string(),
            fields: vec![crate::models::form_def::FormField {
                label: "Frequency".to_string(),
                name: "frequency".to_string(),
                field_type: "select".to_string(),
                required: true,
                options: vec![("daily".to_string(), "Daily".to_string())],
            }],
            settings: crate::models::form_def::FormSettings::default(),
            email_provider_id: None,
            total_submissions: 0,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_poll() -> PollDef {
        let now = chrono::Utc::now();
        PollDef {
            id: uuid::Uuid::new_v4(),
            site_id: uuid::Uuid::new_v4(),
            name: "Frequency".to_string(),
            slug: "frequency".to_string(),
            question: "How often?".to_string(),
            options: vec![crate::models::poll_def::PollOption {
                key: "daily".to_string(),
                label: "Daily".to_string(),
            }],
            settings: crate::models::poll_def::PollSettings::default(),
            total_votes: 0,
            created_at: now,
            updated_at: now,
        }
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

    // ── Abuse / cost control tests ───────────────────────────────────────

    #[test]
    fn translation_rate_limit_allows_up_to_the_cap_then_rejects() {
        let site_id = Uuid::new_v4();
        for _ in 0..TRANSLATIONS_PER_SITE_PER_HOUR {
            assert!(consume_translation_slot(site_id));
        }
        assert!(
            !consume_translation_slot(site_id),
            "call past the per-hour cap should be rejected"
        );
    }

    #[test]
    fn translation_rate_limit_is_scoped_per_site() {
        let site_a = Uuid::new_v4();
        let site_b = Uuid::new_v4();
        for _ in 0..TRANSLATIONS_PER_SITE_PER_HOUR {
            assert!(consume_translation_slot(site_a));
        }
        assert!(
            !consume_translation_slot(site_a),
            "site_a should be exhausted"
        );
        assert!(
            consume_translation_slot(site_b),
            "site_b has its own independent budget"
        );
    }

    #[test]
    fn enforce_translation_limits_rejects_oversized_content() {
        let site_id = Uuid::new_v4();
        let err = enforce_translation_limits(site_id, MAX_SOURCE_CHARS + 1)
            .expect_err("oversized content must be rejected");
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn enforce_translation_limits_accepts_content_at_the_cap() {
        let site_id = Uuid::new_v4();
        assert!(enforce_translation_limits(site_id, MAX_SOURCE_CHARS).is_ok());
    }

    #[test]
    fn enforce_translation_limits_rejecting_for_size_does_not_consume_a_rate_limit_slot() {
        let site_id = Uuid::new_v4();
        for _ in 0..(TRANSLATIONS_PER_SITE_PER_HOUR * 2) {
            assert!(enforce_translation_limits(site_id, MAX_SOURCE_CHARS + 1).is_err());
        }
        // None of the oversized-content rejections above should have burned
        // a rate-limit slot, so the site's full budget is still available.
        for _ in 0..TRANSLATIONS_PER_SITE_PER_HOUR {
            assert!(enforce_translation_limits(site_id, 10).is_ok());
        }
        assert!(enforce_translation_limits(site_id, 10).is_err());
    }

    #[test]
    fn enforce_translation_limits_rejects_once_rate_limit_reached() {
        let site_id = Uuid::new_v4();
        for _ in 0..TRANSLATIONS_PER_SITE_PER_HOUR {
            assert!(enforce_translation_limits(site_id, 10).is_ok());
        }
        let err = enforce_translation_limits(site_id, 10)
            .expect_err("call past the per-hour cap should be rejected");
        assert!(err.to_string().contains("rate limit"));
    }

    #[tokio::test]
    async fn translate_post_rejects_oversized_content_before_any_network_call() {
        // A config with an invalid base URL — if this test reaches the
        // network call at all, it will fail with a connection error rather
        // than the expected "too long" message, catching a regression where
        // the length guard stops running before `send_prompt`.
        let config = crate::models::ai_provider::AiProviderConfig::OpenaiCompatible {
            base_url: "http://127.0.0.1:1".to_string(),
            api_key: "test-key".to_string(),
            model_name: "test-model".to_string(),
        };
        let oversized_content = "x".repeat(MAX_SOURCE_CHARS + 1);
        let post = make_post("html", &oversized_content, None);
        let err = translate_post(&config, &post, "Spanish")
            .await
            .expect_err("oversized post content must be rejected");
        assert!(
            err.to_string().contains("too long"),
            "expected a content-length rejection, got: {err}"
        );
    }
}
