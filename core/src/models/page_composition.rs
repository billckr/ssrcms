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
    pub composition: serde_json::Value,
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

pub async fn upsert(
    pool: &PgPool,
    id: Option<Uuid>,
    site_id: Uuid,
    project_id: Option<Uuid>,
    name: &str,
    composition: serde_json::Value,
    created_by: Option<Uuid>,
) -> Result<PageComposition> {
    let row = if let Some(existing_id) = id {
        sqlx::query_as::<_, PageComposition>(
            "UPDATE page_compositions
             SET name = $1, composition = $2, updated_at = NOW()
             WHERE id = $3 AND site_id = $4
             RETURNING *",
        )
        .bind(name)
        .bind(&composition)
        .bind(existing_id)
        .bind(site_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as::<_, PageComposition>(
            "INSERT INTO page_compositions (site_id, project_id, name, composition, created_by)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING *",
        )
        .bind(site_id)
        .bind(project_id)
        .bind(name)
        .bind(&composition)
        .bind(created_by)
        .fetch_one(pool)
        .await?
    };
    Ok(row)
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
