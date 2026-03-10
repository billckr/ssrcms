use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MediaFolder {
    pub id:         Uuid,
    pub site_id:    Uuid,
    pub name:       String,
    pub created_at: DateTime<Utc>,
}

pub async fn list(pool: &PgPool, site_id: Uuid) -> Result<Vec<MediaFolder>> {
    let rows = sqlx::query_as::<_, MediaFolder>(
        "SELECT * FROM media_folders WHERE site_id = $1 ORDER BY name ASC"
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create(pool: &PgPool, site_id: Uuid, name: &str) -> Result<MediaFolder> {
    let row = sqlx::query_as::<_, MediaFolder>(
        "INSERT INTO media_folders (site_id, name) VALUES ($1, $2) RETURNING *"
    )
    .bind(site_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM media_folders WHERE id = $1 AND site_id = $2"
    )
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}
