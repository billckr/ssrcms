//! Poll Designer: single-question vote polls, embedded into post/page
//! content the same way Form Designer forms are. Distinct from
//! `poll_vote` — this module owns the *shape* of a poll (its question and
//! options); `poll_vote` owns the votes visitors cast.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;
use crate::utils::slugify::slugify;

/// How a poll guards against the same visitor voting more than once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteProtection {
    CookieOnly,
    CookieAndIp,
}

impl VoteProtection {
    pub fn as_str(&self) -> &'static str {
        match self {
            VoteProtection::CookieOnly => "cookie_only",
            VoteProtection::CookieAndIp => "cookie_and_ip",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "cookie_only" => Some(VoteProtection::CookieOnly),
            "cookie_and_ip" => Some(VoteProtection::CookieAndIp),
            _ => None,
        }
    }
}

impl Default for VoteProtection {
    fn default() -> Self {
        VoteProtection::CookieAndIp
    }
}

/// One selectable option. `key` is what gets stored on the vote row and
/// used in results; `label` is what's shown to voters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollSettings {
    #[serde(default = "default_success_message")]
    pub success_message: String,
    #[serde(default = "default_button_label")]
    pub button_label: String,
    #[serde(default)]
    pub vote_protection: VoteProtection,
}

fn default_success_message() -> String {
    "Thanks for voting!".to_string()
}
fn default_button_label() -> String {
    "Vote".to_string()
}

impl Default for PollSettings {
    fn default() -> Self {
        PollSettings {
            success_message: default_success_message(),
            button_label: default_button_label(),
            vote_protection: VoteProtection::default(),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PollDefRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub slug: String,
    pub question: String,
    pub options: serde_json::Value,
    pub settings: serde_json::Value,
    pub total_votes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PollDef {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub slug: String,
    pub question: String,
    pub options: Vec<PollOption>,
    pub settings: PollSettings,
    pub total_votes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<PollDefRow> for PollDef {
    fn from(row: PollDefRow) -> Self {
        PollDef {
            id: row.id,
            site_id: row.site_id,
            name: row.name,
            slug: row.slug,
            question: row.question,
            options: serde_json::from_value(row.options).unwrap_or_default(),
            settings: serde_json::from_value(row.settings).unwrap_or_default(),
            total_votes: row.total_votes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<PollDef>> {
    let rows = sqlx::query_as::<_, PollDefRow>(
        "SELECT * FROM polls WHERE site_id = $1 ORDER BY updated_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(PollDef::from).collect())
}

pub async fn get_by_id(pool: &PgPool, site_id: Uuid, id: Uuid) -> Result<Option<PollDef>> {
    let row = sqlx::query_as::<_, PollDefRow>("SELECT * FROM polls WHERE site_id = $1 AND id = $2")
        .bind(site_id)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(PollDef::from))
}

pub async fn get_by_slug(pool: &PgPool, site_id: Uuid, slug: &str) -> Result<Option<PollDef>> {
    let row =
        sqlx::query_as::<_, PollDefRow>("SELECT * FROM polls WHERE site_id = $1 AND slug = $2")
            .bind(site_id)
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(PollDef::from))
}

/// Generate a unique slug for a site by suffixing `-2`, `-3`, ... on
/// collision — same convention as `form_def::unique_slug`.
async fn unique_slug(
    pool: &PgPool,
    site_id: Uuid,
    base: &str,
    ignore_id: Option<Uuid>,
) -> Result<String> {
    let base = if base.is_empty() {
        "poll".to_string()
    } else {
        base.to_string()
    };
    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM polls WHERE site_id = $1 AND slug = $2 AND id IS DISTINCT FROM $3)",
        )
        .bind(site_id)
        .bind(&candidate)
        .bind(ignore_id)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Ok(candidate);
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
}

pub struct CreatePollDef {
    pub site_id: Uuid,
    pub name: String,
    pub question: String,
    pub options: Vec<PollOption>,
    pub settings: PollSettings,
}

pub async fn create(pool: &PgPool, input: CreatePollDef) -> Result<PollDef> {
    let slug = unique_slug(pool, input.site_id, &slugify(&input.name), None).await?;
    let options_json = serde_json::to_value(&input.options).unwrap_or_default();
    let settings_json = serde_json::to_value(&input.settings).unwrap_or_default();
    let row = sqlx::query_as::<_, PollDefRow>(
        "INSERT INTO polls (site_id, name, slug, question, options, settings)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(input.site_id)
    .bind(&input.name)
    .bind(&slug)
    .bind(&input.question)
    .bind(&options_json)
    .bind(&settings_json)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub struct UpdatePollDef {
    pub name: String,
    pub question: String,
    pub options: Vec<PollOption>,
    pub settings: PollSettings,
}

/// Update a poll's name/question/options/settings. The slug never changes
/// after creation — it's what post/page embeds and `poll_votes` (via the
/// public `/poll/{slug}` endpoint) reference. Note: renaming an option's
/// `key` after votes exist under the old key will orphan those votes from
/// the new label in the results view — the editor should warn about this,
/// not silently allow it (handled in the admin UI, not here).
pub async fn update(
    pool: &PgPool,
    site_id: Uuid,
    id: Uuid,
    input: UpdatePollDef,
) -> Result<Option<PollDef>> {
    let options_json = serde_json::to_value(&input.options).unwrap_or_default();
    let settings_json = serde_json::to_value(&input.settings).unwrap_or_default();
    let row = sqlx::query_as::<_, PollDefRow>(
        "UPDATE polls SET name = $1, question = $2, options = $3, settings = $4, updated_at = NOW()
         WHERE site_id = $5 AND id = $6
         RETURNING *",
    )
    .bind(&input.name)
    .bind(&input.question)
    .bind(&options_json)
    .bind(&settings_json)
    .bind(site_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(PollDef::from))
}

pub async fn delete(pool: &PgPool, site_id: Uuid, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM polls WHERE site_id = $1 AND id = $2")
        .bind(site_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl PollDef {
    /// Render the public-facing poll `<form>`. Always renders the blank
    /// voting form — no live results are pre-rendered server-side, keeping
    /// this as stateless as `FormDef::render_html` (no cookie-reading at
    /// render time). The inline script swaps to a fetched results view on
    /// `?voted={slug}` or `?already_voted={slug}`, set by `poll::submit`'s
    /// redirect — same "swap via query string" idiom forms already use.
    pub fn render_html(&self) -> String {
        self.render_html_localized(None, None)
    }

    pub fn render_html_localized(
        &self,
        locale: Option<&str>,
        translation: Option<&crate::models::embedded_translation::PollTranslationPayload>,
    ) -> String {
        let mut poll = self.clone();
        let mut total_votes_label = "{count} total votes".to_string();
        if let Some(translation) = translation {
            poll.question.clone_from(&translation.question);
            poll.settings
                .button_label
                .clone_from(&translation.button_label);
            poll.settings
                .success_message
                .clone_from(&translation.success_message);
            total_votes_label.clone_from(&translation.total_votes_label);
            for option in &mut poll.options {
                if let Some(label) = translation.option_labels.get(&option.key) {
                    option.label.clone_from(label);
                }
            }
        }
        poll.render_html_inner(locale, &total_votes_label)
    }

    fn render_html_inner(&self, locale: Option<&str>, total_votes_label: &str) -> String {
        let slug = html_escape(&self.slug);
        let results_url = match locale {
            Some(locale) => format!("/poll/{}/results?locale={}", self.slug, locale),
            None => format!("/poll/{}/results", self.slug),
        };
        let results_url_js =
            serde_json::to_string(&results_url).unwrap_or_else(|_| "\"\"".to_string());
        let total_votes_label_js = serde_json::to_string(total_votes_label)
            .unwrap_or_else(|_| "\"{count} total votes\"".to_string());
        let options_html: String = self.options.iter().enumerate().map(|(i, o)| {
            let opt_id = format!("ss-poll-{slug}-{i}");
            format!(
                r#"<label class="form-radio-label" for="{opt_id}"><input type="radio" id="{opt_id}" name="option" value="{val}" required> {opt_label}</label>"#,
                opt_id = opt_id, val = html_escape(&o.key), opt_label = html_escape(&o.label),
            )
        }).collect();

        format!(
            r#"<form class="themed-form ss-poll-form" id="ss-poll-{slug}" method="POST" action="/poll/{slug}">
  <div class="form-field form-field-radio">
    <span class="form-field-legend">{question}</span>
    {options_html}
  </div>
  <button type="submit" class="btn">{button_label}</button>
</form>
<div class="ss-poll-results" id="ss-poll-results-{slug}" style="display:none"></div>
<script>(function(){{
  var f=document.getElementById('ss-poll-{slug}'),r=document.getElementById('ss-poll-results-{slug}');
  if(!f||!r)return;
  function escapeHtml(value){{return value.replace(/[&<>"']/g,function(ch){{return {{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}}[ch];}});}}
  var qs=new URLSearchParams(location.search);
  if(qs.get('voted')==={slug_js}||qs.get('already_voted')==={slug_js}){{
    f.style.display='none';
    r.style.display='';
    fetch({results_url_js}).then(function(res){{return res.json();}}).then(function(data){{
      var total=data.total||0;
      var html='';
      (data.options||[]).forEach(function(o){{
        var pct=total>0?Math.round((o.votes/total)*100):0;
        html+='<div class="ss-poll-result-row" style="margin-bottom:8px">'+
          '<div style="display:flex;justify-content:space-between;font-size:13px;margin-bottom:2px"><span>'+escapeHtml(String(o.label||''))+'</span><span>'+pct+'% ('+o.votes+')</span></div>'+
          '<div style="background:var(--tint,#eee);border-radius:4px;height:8px;overflow:hidden"><div style="width:'+pct+'%;height:100%;background:var(--primary,#2563eb)"></div></div>'+
        '</div>';
      }});
      html+='<p style="font-size:12px;color:var(--muted,#64748b);margin-top:6px">'+escapeHtml({total_votes_label_js}.replace('{{count}}',String(total)))+'</p>';
      r.innerHTML=html;
      r.scrollIntoView({{behavior:'smooth',block:'start'}});
    }});
  }}
}})();</script>
"#,
            question = html_escape(&self.question),
            options_html = options_html,
            button_label = html_escape(&self.settings.button_label),
            slug_js = serde_json::to_string(&self.slug).unwrap_or_else(|_| "\"\"".to_string()),
            results_url_js = results_url_js,
            total_votes_label_js = total_votes_label_js,
        )
    }
}

/// Expand every `<ss-poll data-slug="...">` embed in `content` into the real
/// rendered poll form — same regex-replace-by-slug approach as
/// `form_def::expand_embeds`, silently dropping embeds whose poll was
/// deleted.
pub async fn expand_embeds(
    pool: &PgPool,
    site_id: Uuid,
    content: &str,
    locale: Option<&str>,
) -> String {
    let slugs = embedded_slugs(content);
    if slugs.is_empty() {
        return content.to_string();
    }

    let mut result = content.to_string();
    for slug in slugs {
        let replacement = match get_by_slug(pool, site_id, &slug).await {
            Ok(Some(poll)) => {
                let translation = match locale {
                    Some(locale) => {
                        crate::models::embedded_translation::get_poll(pool, poll.id, locale)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|row| row.parsed())
                    }
                    None => None,
                };
                poll.render_html_localized(locale, translation.as_ref())
            }
            _ => String::new(),
        };
        let escaped_slug = slug.replace('\\', "\\\\").replace('"', "\\\"");
        let Ok(specific_re) = regex_lite::Regex::new(&format!(
            r#"<ss-poll\b[^>]*data-slug="{escaped_slug}"[^>]*></ss-poll>"#
        )) else {
            continue;
        };
        result = specific_re
            .replace_all(&result, replacement.as_str())
            .to_string();
    }
    result
}

pub fn embedded_slugs(content: &str) -> Vec<String> {
    if !content.contains("<ss-poll") {
        return Vec::new();
    }
    let Ok(tag_re) =
        regex_lite::Regex::new(r#"<ss-poll\b[^>]*data-slug="([^"]*)"[^>]*></ss-poll>"#)
    else {
        return Vec::new();
    };
    let mut slugs: Vec<String> = tag_re
        .captures_iter(content)
        .map(|c| c[1].to_string())
        .collect();
    slugs.sort();
    slugs.dedup();
    slugs
}
