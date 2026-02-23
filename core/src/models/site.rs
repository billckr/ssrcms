//! Site model — one row per managed website.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Site {
    pub id: Uuid,
    pub hostname: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create(pool: &PgPool, hostname: &str) -> Result<Site> {
    let site = sqlx::query_as::<_, Site>(
        r#"
        INSERT INTO sites (hostname)
        VALUES ($1)
        RETURNING *
        "#,
    )
    .bind(hostname)
    .fetch_one(pool)
    .await?;
    Ok(site)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Site> {
    sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("site {id}")))
}

pub async fn get_by_hostname(pool: &PgPool, hostname: &str) -> Result<Site> {
    sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE hostname = $1")
        .bind(hostname)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("site '{hostname}'")))
}

pub async fn list(pool: &PgPool) -> Result<Vec<Site>> {
    let sites = sqlx::query_as::<_, Site>("SELECT * FROM sites ORDER BY created_at ASC")
        .fetch_all(pool)
        .await?;
    Ok(sites)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Count published posts for a site (used in site listing).
pub async fn post_count(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM posts WHERE site_id = $1",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}
