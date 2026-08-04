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
