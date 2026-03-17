use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::{AppError, Result};

/// A saved visual page composition: layout + per-zone block list.
/// Each composition is tied to a site-owned theme (via `theme_name`).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PageComposition {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub slug: String,
    pub layout: String,
    /// JSON: { "zones": { "<zone>": [ { "block_type": "...", "config": {...} }, ... ] } }
    pub composition: serde_json::Value,
    /// Directory name of the theme this composition belongs to.
    /// Matches a theme folder under `sites/{site_id}/themes/{theme_name}/`.
    pub theme_name: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The top-level structure stored in `composition` JSONB.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompositionJson {
    pub zones: HashMap<String, Vec<BlockEntry>>,
}

/// A single block placement within a zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEntry {
    pub block_type: String,
    pub config: HashMap<String, serde_json::Value>,
}

/// Available layout shells (PoC set).
pub const LAYOUTS: &[(&str, &str)] = &[
    ("single-column", "Single Column"),
    ("left-sidebar", "Left Sidebar"),
    ("right-sidebar", "Right Sidebar"),
];

/// Available block types (PoC set).
pub const BLOCK_TYPES: &[(&str, &str)] = &[
    ("text-block", "Text Block"),
    ("posts-grid", "Posts Grid"),
    ("nav-menu", "Nav Menu"),
    ("contact-form", "Contact Form"),
];

// ── DB helpers ───────────────────────────────────────────────────────────────

pub async fn list(pool: &PgPool, site_id: Uuid) -> Result<Vec<PageComposition>> {
    sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE site_id = $1 ORDER BY updated_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn get(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<PageComposition> {
    sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Composition not found".to_string()),
        other => AppError::from(other),
    })
}

/// Fetch the composition for the given site's currently active theme, if any.
/// Called by the home handler: if the active theme has a composition, render it.
pub async fn get_by_theme(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
) -> Result<Option<PageComposition>> {
    sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE site_id = $1 AND theme_name = $2 LIMIT 1",
    )
    .bind(site_id)
    .bind(theme_name)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create(
    pool: &PgPool,
    site_id: Uuid,
    name: &str,
    slug: &str,
    layout: &str,
    theme_name: &str,
    composition: serde_json::Value,
    created_by: Option<Uuid>,
) -> Result<PageComposition> {
    sqlx::query_as::<_, PageComposition>(
        r#"INSERT INTO page_compositions
               (site_id, name, slug, layout, theme_name, composition, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#,
    )
    .bind(site_id)
    .bind(name)
    .bind(slug)
    .bind(layout)
    .bind(theme_name)
    .bind(composition)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    site_id: Uuid,
    name: &str,
    layout: &str,
    composition: serde_json::Value,
) -> Result<PageComposition> {
    sqlx::query_as::<_, PageComposition>(
        r#"UPDATE page_compositions
           SET name = $3, layout = $4, composition = $5, updated_at = NOW()
           WHERE id = $1 AND site_id = $2
           RETURNING *"#,
    )
    .bind(id)
    .bind(site_id)
    .bind(name)
    .bind(layout)
    .bind(composition)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Composition not found".to_string()),
        other => AppError::from(other),
    })
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM page_compositions WHERE id = $1 AND site_id = $2")
        .bind(id)
        .bind(site_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}
