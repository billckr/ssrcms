//! Form Designer: reusable form definitions built in the admin panel and
//! (eventually) embedded into post/page content. Distinct from
//! `form_submission` — this module owns the *shape* of a form (its fields
//! and settings); `form_submission` owns the data visitors send in.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::errors::Result;
use crate::utils::slugify::slugify;

/// One field in a form's definition. `options` is only meaningful for
/// `select`/`radio` and holds (value, label) pairs in declared order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub label: String,
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Vec<(String, String)>,
}

/// Per-form behavior settings — everything that isn't a field. Button color
/// is intentionally not here — the submit button is styled from the active
/// theme's CSS at render time, not chosen per-form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSettings {
    #[serde(default = "default_success_message")]
    pub success_message: String,
    #[serde(default = "default_button_label")]
    pub button_label: String,
    #[serde(default = "default_true")]
    pub include_honeypot: bool,
}

fn default_success_message() -> String { "Thank you for your submission!".to_string() }
fn default_button_label() -> String { "Submit".to_string() }
fn default_true() -> bool { true }

impl Default for FormSettings {
    fn default() -> Self {
        FormSettings {
            success_message: default_success_message(),
            button_label: default_button_label(),
            include_honeypot: true,
        }
    }
}

/// A saved form definition, scoped to a site.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FormDefRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub slug: String,
    pub fields: serde_json::Value,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Parsed view of a [`FormDefRow`] — `fields`/`settings` deserialized out of
/// their JSONB columns. Kept separate from the row type so a malformed JSONB
/// value (shouldn't happen, but defensive) can't fail the whole query.
#[derive(Debug, Clone, Serialize)]
pub struct FormDef {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub slug: String,
    pub fields: Vec<FormField>,
    pub settings: FormSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<FormDefRow> for FormDef {
    fn from(row: FormDefRow) -> Self {
        FormDef {
            id: row.id,
            site_id: row.site_id,
            name: row.name,
            slug: row.slug,
            fields: serde_json::from_value(row.fields).unwrap_or_default(),
            settings: serde_json::from_value(row.settings).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// List every form defined for a site, most recently updated first.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<FormDef>> {
    let rows = sqlx::query_as::<_, FormDefRow>(
        "SELECT * FROM forms WHERE site_id = $1 ORDER BY updated_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(FormDef::from).collect())
}

pub async fn get_by_id(pool: &PgPool, site_id: Uuid, id: Uuid) -> Result<Option<FormDef>> {
    let row = sqlx::query_as::<_, FormDefRow>(
        "SELECT * FROM forms WHERE site_id = $1 AND id = $2",
    )
    .bind(site_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(FormDef::from))
}

pub async fn get_by_slug(pool: &PgPool, site_id: Uuid, slug: &str) -> Result<Option<FormDef>> {
    let row = sqlx::query_as::<_, FormDefRow>(
        "SELECT * FROM forms WHERE site_id = $1 AND slug = $2",
    )
    .bind(site_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(FormDef::from))
}

/// Generate a unique slug for a site by suffixing `-2`, `-3`, ... on
/// collision — same convention used for post/page slugs.
async fn unique_slug(pool: &PgPool, site_id: Uuid, base: &str, ignore_id: Option<Uuid>) -> Result<String> {
    let base = if base.is_empty() { "form".to_string() } else { base.to_string() };
    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM forms WHERE site_id = $1 AND slug = $2 AND id IS DISTINCT FROM $3)",
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

pub struct CreateFormDef {
    pub site_id: Uuid,
    pub name: String,
    pub fields: Vec<FormField>,
    pub settings: FormSettings,
}

pub async fn create(pool: &PgPool, input: CreateFormDef) -> Result<FormDef> {
    let slug = unique_slug(pool, input.site_id, &slugify(&input.name), None).await?;
    let fields_json = serde_json::to_value(&input.fields).unwrap_or_default();
    let settings_json = serde_json::to_value(&input.settings).unwrap_or_default();
    let row = sqlx::query_as::<_, FormDefRow>(
        "INSERT INTO forms (site_id, name, slug, fields, settings)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(input.site_id)
    .bind(&input.name)
    .bind(&slug)
    .bind(&fields_json)
    .bind(&settings_json)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub struct UpdateFormDef {
    pub name: String,
    pub fields: Vec<FormField>,
    pub settings: FormSettings,
}

/// Update a form's name/fields/settings. The slug is intentionally never
/// changed after creation — it's what post/page embeds and existing
/// `form_submissions.form_name` rows reference, so renaming it here would
/// silently break both.
pub async fn update(pool: &PgPool, site_id: Uuid, id: Uuid, input: UpdateFormDef) -> Result<Option<FormDef>> {
    let fields_json = serde_json::to_value(&input.fields).unwrap_or_default();
    let settings_json = serde_json::to_value(&input.settings).unwrap_or_default();
    let row = sqlx::query_as::<_, FormDefRow>(
        "UPDATE forms SET name = $1, fields = $2, settings = $3, updated_at = NOW()
         WHERE site_id = $4 AND id = $5
         RETURNING *",
    )
    .bind(&input.name)
    .bind(&fields_json)
    .bind(&settings_json)
    .bind(site_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(FormDef::from))
}

pub async fn delete(pool: &PgPool, site_id: Uuid, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM forms WHERE site_id = $1 AND id = $2")
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

/// Render one field's markup. Class names deliberately match the
/// hand-written-form convention already established across themes (see
/// e.g. `themes/global/leisure/templates/contact-page.html` and its
/// "Shared form styles" CSS block: `.form-field`, `.form-required`,
/// `.form-checkbox-label`) rather than inventing a parallel `ss-form-*`
/// scheme — that way a generated form picks up a theme's existing form
/// styling for free, with zero new CSS required on themes that already
/// follow it. `form-field-{type}` is added as a secondary hook for
/// per-type styling (select/radio/toggle) that hand-written forms didn't
/// previously need.
fn render_field_html(f: &FormField, slug: &str) -> String {
    let id = format!("ss-form-{}-{}", html_escape(slug), html_escape(&f.name));
    let required_attr = if f.required { " required" } else { "" };
    let required_mark = if f.required { r#" <span class="form-required" aria-hidden="true">*</span>"# } else { "" };
    let label = html_escape(&f.label);
    let name = html_escape(&f.name);

    match f.field_type.as_str() {
        // Visual-only — no input, no `name`, nothing submitted. `label`
        // doubles as an optional section title (separator) or the callout
        // text itself (note).
        "separator" => {
            let title = if f.label.trim().is_empty() {
                String::new()
            } else {
                format!(r#"<p class="form-separator-title">{label}</p>"#)
            };
            format!(
                r#"<div class="form-field form-field-separator">
  {title}
  <hr class="form-separator">
</div>
"#
            )
        }
        "note" => format!(
            r#"<div class="form-field form-field-note">
  <p class="form-callout">{label}</p>
</div>
"#
        ),
        "checkbox" => format!(
            r#"<div class="form-field form-field-checkbox">
  <label class="form-checkbox-label"><input type="checkbox" id="{id}" name="{name}" value="true"{required_attr}> <span>{label}{required_mark}</span></label>
</div>
"#
        ),
        "toggle" => {
            let off_label = f.options.first().map(|(_, l)| l.as_str()).unwrap_or("Off");
            let on_label = f.options.get(1).map(|(_, l)| l.as_str()).unwrap_or("On");
            let on_value = f.options.get(1).map(|(v, _)| v.as_str()).unwrap_or("true");
            format!(
                r#"<div class="form-field form-field-toggle">
  <label for="{id}">{label}{required_mark}</label>
  <label class="form-toggle-label" for="{id}">
    <span class="form-toggle-off">{off}</span>
    <input type="checkbox" id="{id}" name="{name}" value="{on_value}" class="form-toggle-input"{required_attr}>
    <span class="form-toggle-slider" aria-hidden="true"></span>
    <span class="form-toggle-on">{on}</span>
  </label>
</div>
"#,
                off = html_escape(off_label),
                on = html_escape(on_label),
            )
        }
        "textarea" => format!(
            r#"<div class="form-field form-field-textarea">
  <label for="{id}">{label}{required_mark}</label>
  <textarea id="{id}" name="{name}"{required_attr}></textarea>
</div>
"#
        ),
        "select" => {
            let options: String = f.options.iter().map(|(v, l)| {
                format!(r#"<option value="{}">{}</option>"#, html_escape(v), html_escape(l))
            }).collect();
            format!(
                r#"<div class="form-field form-field-select">
  <label for="{id}">{label}{required_mark}</label>
  <select id="{id}" name="{name}"{required_attr}>{options}</select>
</div>
"#
            )
        }
        "radio" => {
            let options: String = f.options.iter().enumerate().map(|(i, (v, l))| {
                let opt_id = format!("{id}-{i}");
                format!(
                    r#"<label class="form-radio-label" for="{opt_id}"><input type="radio" id="{opt_id}" name="{name}" value="{val}"{required_attr}> {opt_label}</label>"#,
                    opt_id = opt_id, val = html_escape(v), opt_label = html_escape(l),
                )
            }).collect();
            format!(
                r#"<div class="form-field form-field-radio">
  <span class="form-field-legend">{label}{required_mark}</span>
  {options}
</div>
"#
            )
        }
        other => {
            let input_type = match other {
                "email" => "email",
                "number" => "number",
                "phone" => "tel",
                "date" => "date",
                _ => "text",
            };
            format!(
                r#"<div class="form-field form-field-{other}">
  <label for="{id}">{label}{required_mark}</label>
  <input type="{input_type}" id="{id}" name="{name}"{required_attr}>
</div>
"#
            )
        }
    }
}

impl FormDef {
    /// Render the public-facing `<form>` for this definition. Markup and
    /// class names match the hand-written contact/newsletter/subscribe
    /// forms already shipped in themes (`.themed-form`, `.form-field`,
    /// `.honeypot-field`, `.form-success`, and the generic `.btn` button
    /// component) so a theme that already styles those — Leisure does —
    /// renders a fully-styled form with no new CSS. A tiny inline script
    /// swaps in the success message when the page's query string shows
    /// this exact form was just submitted (`?submitted={slug}`, set by
    /// `form::submit`'s redirect), matching what the hand-written pages
    /// do server-side via `{% if request.query.submitted %}` — this just
    /// makes it automatic instead of something every theme has to write.
    pub fn render_html(&self) -> String {
        let slug = html_escape(&self.slug);
        let mut html = format!(
            r#"<form class="themed-form" id="ss-form-{slug}" method="POST" action="/form/{slug}">
"#
        );
        if self.settings.include_honeypot {
            html.push_str(&format!(
                r#"<div class="honeypot-field" aria-hidden="true" tabindex="-1">
  <label for="ss-form-{slug}-hp">Leave this blank</label>
  <input type="text" id="ss-form-{slug}-hp" name="_honeypot" tabindex="-1" autocomplete="off">
</div>
"#
            ));
        }
        for f in &self.fields {
            html.push_str(&render_field_html(f, &self.slug));
        }
        html.push_str(&format!(
            r#"<button type="submit" class="btn">{button_label}</button>
</form>
<div class="form-success" id="ss-form-success-{slug}" role="alert" style="display:none">
  <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
  <span>{success}</span>
</div>
<script>(function(){{var f=document.getElementById('ss-form-{slug}'),s=document.getElementById('ss-form-success-{slug}');if(!f||!s)return;if(new URLSearchParams(location.search).get('submitted')==={slug_js}){{f.style.display='none';s.style.display='';s.scrollIntoView({{behavior:'smooth',block:'start'}});}}}})();</script>
"#,
            button_label = html_escape(&self.settings.button_label),
            success = html_escape(&self.settings.success_message),
            slug_js = serde_json::to_string(&self.slug).unwrap_or_else(|_| "\"\"".to_string()),
        ));
        html
    }
}

/// Expand every `<ss-form data-slug="...">` embed in `content` (dropped in
/// by the post/page editor's "Insert Form" button, see FormEmbedBlot in
/// posts.rs) into the real rendered `<form>` for that saved definition.
/// Cheap no-op (no DB hit) when the content has no embed at all — the
/// overwhelming majority of posts/pages. Embeds referencing a deleted or
/// missing form are silently dropped rather than left as raw markup.
pub async fn expand_embeds(pool: &PgPool, site_id: Uuid, content: &str) -> String {
    if !content.contains("<ss-form") {
        return content.to_string();
    }
    let Ok(tag_re) = regex_lite::Regex::new(r#"<ss-form\b[^>]*data-slug="([^"]*)"[^>]*></ss-form>"#) else {
        return content.to_string();
    };

    let mut slugs: Vec<String> = tag_re.captures_iter(content).map(|c| c[1].to_string()).collect();
    slugs.sort();
    slugs.dedup();

    let mut result = content.to_string();
    for slug in slugs {
        let replacement = match get_by_slug(pool, site_id, &slug).await {
            Ok(Some(form)) => form.render_html(),
            _ => String::new(),
        };
        let escaped_slug = slug.replace('\\', "\\\\").replace('"', "\\\"");
        let Ok(specific_re) = regex_lite::Regex::new(
            &format!(r#"<ss-form\b[^>]*data-slug="{escaped_slug}"[^>]*></ss-form>"#),
        ) else { continue };
        result = specific_re.replace_all(&result, replacement.as_str()).to_string();
    }
    result
}
