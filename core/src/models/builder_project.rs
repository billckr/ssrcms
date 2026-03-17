use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BuilderProject {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_by_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<BuilderProject>> {
    Ok(sqlx::query_as::<_, BuilderProject>(
        "SELECT * FROM builder_projects WHERE site_id = $1 ORDER BY is_active DESC, updated_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<Option<BuilderProject>> {
    Ok(sqlx::query_as::<_, BuilderProject>(
        "SELECT * FROM builder_projects WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_active(pool: &PgPool, site_id: Uuid) -> Result<Option<BuilderProject>> {
    Ok(sqlx::query_as::<_, BuilderProject>(
        "SELECT * FROM builder_projects WHERE site_id = $1 AND is_active = TRUE LIMIT 1",
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(
    pool: &PgPool,
    site_id: Uuid,
    name: &str,
    description: Option<&str>,
    created_by: Option<Uuid>,
) -> Result<BuilderProject> {
    Ok(sqlx::query_as::<_, BuilderProject>(
        "INSERT INTO builder_projects (site_id, name, description, created_by)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(site_id)
    .bind(name)
    .bind(description.filter(|s| !s.is_empty()))
    .bind(created_by)
    .fetch_one(pool)
    .await?)
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    site_id: Uuid,
    name: &str,
    description: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE builder_projects
         SET name = $1, description = $2, updated_at = NOW()
         WHERE id = $3 AND site_id = $4",
    )
    .bind(name)
    .bind(description.filter(|s| !s.is_empty()))
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn activate(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE builder_projects SET is_active = FALSE, updated_at = NOW()
         WHERE site_id = $1 AND is_active = TRUE",
    )
    .bind(site_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE builder_projects SET is_active = TRUE, updated_at = NOW()
         WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn deactivate(pool: &PgPool, site_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE builder_projects SET is_active = FALSE, updated_at = NOW()
         WHERE site_id = $1",
    )
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM builder_projects WHERE id = $1 AND site_id = $2",
    )
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Page count for a project — used in the list view.
pub async fn page_count(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM page_compositions WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}
