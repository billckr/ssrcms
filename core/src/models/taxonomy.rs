use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaxonomyType {
    Category,
    Tag,
}

impl TaxonomyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaxonomyType::Category => "category",
            TaxonomyType::Tag => "tag",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Taxonomy {
    pub id: Uuid,
    pub site_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub taxonomy: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

/// Template context view of a taxonomy term, including URL and post count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermContext {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub taxonomy: String,
    pub url: String,
    pub post_count: i64,
}

impl TermContext {
    pub fn from_taxonomy(tax: &Taxonomy, base_url: &str, post_count: i64) -> Self {
        let archive_segment = match tax.taxonomy.as_str() {
            "category" => "category",
            "tag" => "tag",
            other => other,
        };
        TermContext {
            id: tax.id.to_string(),
            name: tax.name.clone(),
            slug: tax.slug.clone(),
            taxonomy: tax.taxonomy.clone(),
            url: format!("{}/{}/{}", base_url, archive_segment, tax.slug),
            post_count,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTaxonomy {
    pub site_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub taxonomy: TaxonomyType,
    pub description: Option<String>,
}

pub async fn create(pool: &PgPool, data: &CreateTaxonomy) -> Result<Taxonomy> {
    let tax = sqlx::query_as::<_, Taxonomy>(
        r#"
        INSERT INTO taxonomies (site_id, name, slug, taxonomy, description)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(data.site_id)
    .bind(&data.name)
    .bind(&data.slug)
    .bind(data.taxonomy.as_str())
    .bind(data.description.as_deref().unwrap_or(""))
    .fetch_one(pool)
    .await?;
    Ok(tax)
}

#[allow(dead_code)]
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Taxonomy> {
    sqlx::query_as::<_, Taxonomy>("SELECT * FROM taxonomies WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("taxonomy {id}")))
}

pub async fn get_by_slug(
    pool: &PgPool,
    site_id: Option<Uuid>,
    slug: &str,
    taxonomy: TaxonomyType,
) -> Result<Taxonomy> {
    sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = $1 AND taxonomy = $2 \
         AND ($3::uuid IS NULL OR site_id = $3)",
    )
    .bind(slug)
    .bind(taxonomy.as_str())
    .bind(site_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("{} '{slug}'", taxonomy.as_str())))
}

pub async fn list(pool: &PgPool, site_id: Option<Uuid>, taxonomy: TaxonomyType) -> Result<Vec<Taxonomy>> {
    sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE taxonomy = $1 AND ($2::uuid IS NULL OR site_id = $2) ORDER BY name",
    )
    .bind(taxonomy.as_str())
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Fetch taxonomies for a given post.
pub async fn for_post(pool: &PgPool, post_id: Uuid) -> Result<Vec<Taxonomy>> {
    sqlx::query_as::<_, Taxonomy>(
        r#"
        SELECT t.*
        FROM taxonomies t
        JOIN post_taxonomies pt ON pt.taxonomy_id = t.id
        WHERE pt.post_id = $1
        ORDER BY t.taxonomy, t.name
        "#,
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Assign a taxonomy term to a post (idempotent).
pub async fn attach_to_post(pool: &PgPool, post_id: Uuid, taxonomy_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO post_taxonomies (post_id, taxonomy_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(post_id)
    .bind(taxonomy_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a taxonomy term from a post.
pub async fn detach_from_post(pool: &PgPool, post_id: Uuid, taxonomy_id: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM post_taxonomies WHERE post_id = $1 AND taxonomy_id = $2",
    )
    .bind(post_id)
    .bind(taxonomy_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Count published posts for a given taxonomy term.
pub async fn post_count(pool: &PgPool, taxonomy_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post_taxonomies pt
        JOIN posts p ON p.id = pt.post_id
        WHERE pt.taxonomy_id = $1
          AND p.status = 'published'
        "#,
    )
    .bind(taxonomy_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM taxonomies WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
