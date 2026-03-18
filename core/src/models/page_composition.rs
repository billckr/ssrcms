use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PageComposition {
    pub id: Uuid,
    pub site_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub slug: Option<String>,
    pub page_type: String,   // "homepage" | "page"
    pub composition: serde_json::Value,       // live — what visitors see
    pub draft_composition: serde_json::Value, // work in progress — what the editor reads/writes
    pub is_homepage: bool,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_by_project(pool: &PgPool, project_id: Uuid) -> Result<Vec<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE project_id = $1 ORDER BY is_homepage DESC, updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

/// Returns the active homepage for a site via its active project.
pub async fn get_homepage(pool: &PgPool, site_id: Uuid) -> Result<Option<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT pc.* FROM page_compositions pc
         JOIN builder_projects bp ON bp.id = pc.project_id
         WHERE bp.site_id = $1 AND bp.is_active = TRUE AND pc.is_homepage = TRUE
         LIMIT 1",
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

/// Returns a regular builder page matching `slug` for the site's active project.
pub async fn get_by_slug(pool: &PgPool, site_id: Uuid, slug: &str) -> Result<Option<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT pc.* FROM page_compositions pc
         JOIN builder_projects bp ON bp.id = pc.project_id
         WHERE bp.site_id = $1 AND bp.is_active = TRUE
           AND pc.page_type = 'page' AND pc.slug = $2
         LIMIT 1",
    )
    .bind(site_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?)
}

/// Create a new empty page (called from the new-page form before entering the editor).
pub async fn create(
    pool: &PgPool,
    site_id: Uuid,
    project_id: Uuid,
    name: &str,
    page_type: &str,
    slug: Option<&str>,
    is_homepage: bool,
    created_by: Option<Uuid>,
) -> Result<PageComposition> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "INSERT INTO page_compositions
             (site_id, project_id, name, page_type, slug, is_homepage, composition, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, '{}', $7)
         RETURNING *",
    )
    .bind(site_id)
    .bind(project_id)
    .bind(name)
    .bind(page_type)
    .bind(slug)
    .bind(is_homepage)
    .bind(created_by)
    .fetch_one(pool)
    .await?)
}

/// Create a new page pre-populated with an existing draft composition.
pub async fn create_with_draft(
    pool: &PgPool,
    site_id: Uuid,
    project_id: Uuid,
    name: &str,
    page_type: &str,
    slug: Option<&str>,
    is_homepage: bool,
    draft: serde_json::Value,
    created_by: Option<Uuid>,
) -> Result<PageComposition> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "INSERT INTO page_compositions
             (site_id, project_id, name, page_type, slug, is_homepage, composition, draft_composition, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, '{}', $7, $8)
         RETURNING *",
    )
    .bind(site_id)
    .bind(project_id)
    .bind(name)
    .bind(page_type)
    .bind(slug)
    .bind(is_homepage)
    .bind(&draft)
    .bind(created_by)
    .fetch_one(pool)
    .await?)
}

/// Duplicate a page: copies its draft_composition into a new page of type "page".
/// The duplicate is always type "page" — special types (homepage, post_template, etc.)
/// cannot be duplicated directly since only one of each is allowed per project.
pub async fn duplicate(
    pool: &PgPool,
    source_id: Uuid,
    site_id: Uuid,
    project_id: Uuid,
    name_override: Option<&str>,
    created_by: Option<Uuid>,
) -> Result<PageComposition> {
    let source = sqlx::query_as::<_, PageComposition>(
        "SELECT * FROM page_compositions WHERE id = $1 AND site_id = $2",
    )
    .bind(source_id)
    .bind(site_id)
    .fetch_one(pool)
    .await?;

    let new_name = name_override.map(|s| s.to_string()).unwrap_or_else(|| format!("{} (copy)", source.name));
    // Derive slug from the new name if the source had none (homepage, post_template, etc.)
    let new_slug = match source.slug.as_deref() {
        Some(s) => format!("{}-copy", s),
        None => new_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-"),
    };
    let new_slug = Some(new_slug);

    Ok(sqlx::query_as::<_, PageComposition>(
        "INSERT INTO page_compositions
             (site_id, project_id, name, page_type, slug, is_homepage, composition, draft_composition, created_by)
         VALUES ($1, $2, $3, 'page', $4, FALSE, '{}', $5, $6)
         RETURNING *",
    )
    .bind(site_id)
    .bind(project_id)
    .bind(&new_name)
    .bind(new_slug.as_deref())
    .bind(&source.draft_composition)
    .bind(created_by)
    .fetch_one(pool)
    .await?)
}

/// Save to draft only — does not affect what visitors see.
pub async fn save_composition(
    pool: &PgPool,
    id: Uuid,
    site_id: Uuid,
    name: &str,
    draft: serde_json::Value,
) -> Result<PageComposition> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "UPDATE page_compositions
         SET name = $1, draft_composition = $2, updated_at = NOW()
         WHERE id = $3 AND site_id = $4
         RETURNING *",
    )
    .bind(name)
    .bind(&draft)
    .bind(id)
    .bind(site_id)
    .fetch_one(pool)
    .await?)
}

/// Promote draft to live — updates both columns atomically.
pub async fn publish_composition(
    pool: &PgPool,
    id: Uuid,
    site_id: Uuid,
    name: &str,
    data: serde_json::Value,
) -> Result<PageComposition> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "UPDATE page_compositions
         SET name = $1, draft_composition = $2, composition = $2, updated_at = NOW()
         WHERE id = $3 AND site_id = $4
         RETURNING *",
    )
    .bind(name)
    .bind(&data)
    .bind(id)
    .bind(site_id)
    .fetch_one(pool)
    .await?)
}

pub async fn activate_homepage(pool: &PgPool, id: Uuid, project_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE page_compositions SET is_homepage = FALSE, updated_at = NOW()
         WHERE project_id = $1 AND is_homepage = TRUE",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE page_compositions SET is_homepage = TRUE, updated_at = NOW()
         WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn deactivate_homepage(pool: &PgPool, project_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE page_compositions SET is_homepage = FALSE, updated_at = NOW()
         WHERE project_id = $1",
    )
    .bind(project_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Count pages in a project that have been published (composition has at least one block).
pub async fn count_published(pool: &PgPool, project_id: Uuid) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM page_compositions
         WHERE project_id = $1
           AND jsonb_array_length(composition->'content') > 0",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM page_compositions WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns the archive template page for a site's active project, if one exists.
/// Used for both category and tag archive URLs.
pub async fn get_archive_template(pool: &PgPool, site_id: Uuid) -> Result<Option<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT pc.* FROM page_compositions pc
         JOIN builder_projects bp ON bp.id = pc.project_id
         WHERE bp.site_id = $1 AND bp.is_active = TRUE AND pc.page_type = 'archive_template'
         LIMIT 1",
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

/// Returns the post template page for a site's active project, if one exists.
pub async fn get_post_template(pool: &PgPool, site_id: Uuid) -> Result<Option<PageComposition>> {
    Ok(sqlx::query_as::<_, PageComposition>(
        "SELECT pc.* FROM page_compositions pc
         JOIN builder_projects bp ON bp.id = pc.project_id
         WHERE bp.site_id = $1 AND bp.is_active = TRUE AND pc.page_type = 'post_template'
         LIMIT 1",
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}
