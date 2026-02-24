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
    /// Immutable creator of this site. NULL = installed by CLI / super_admin.
    /// Use site_users WHERE role='admin' AND site_id=X to find the current admin.
    pub owner_user_id: Option<Uuid>,
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

/// Create a site owned by a site_admin, seed default site_settings, and register
/// the owner as admin in site_users — all in a single transaction.
/// Returns the new Site on success.
pub async fn create_with_defaults(
    pool: &PgPool,
    hostname: &str,
    owner_user_id: Uuid,
) -> Result<Site> {
    let mut tx = pool.begin().await?;

    let site = sqlx::query_as::<_, Site>(
        r#"
        INSERT INTO sites (hostname, owner_user_id)
        VALUES ($1, $2)
        RETURNING *
        "#,
    )
    .bind(hostname)
    .bind(owner_user_id)
    .fetch_one(&mut *tx)
    .await?;

    // Seed default site_settings rows.
    let defaults: &[(&str, &str)] = &[
        ("site_name",        hostname),
        ("site_description", ""),
        ("site_url",         &format!("http://{hostname}")),
        ("site_language",    "en-US"),
        ("active_theme",     "default"),
        ("posts_per_page",   "10"),
        ("date_format",      "%B %-d, %Y"),
    ];
    for (key, value) in defaults {
        sqlx::query(
            "INSERT INTO site_settings (site_id, key, value) VALUES ($1, $2, $3)",
        )
        .bind(site.id)
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }

    // Register the owner as admin on their new site.
    sqlx::query(
        r#"
        INSERT INTO site_users (site_id, user_id, role, invited_by)
        VALUES ($1, $2, 'admin', NULL)
        ON CONFLICT (site_id, user_id) DO UPDATE SET role = 'admin'
        "#,
    )
    .bind(site.id)
    .bind(owner_user_id)
    .execute(&mut *tx)
    .await?;

    // Set owner's default_site_id if they don't have one yet.
    sqlx::query(
        "UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2 AND default_site_id IS NULL",
    )
    .bind(site.id)
    .bind(owner_user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(site)
}

/// List all sites where owner_user_id matches — these are sites the given
/// user created themselves (as opposed to being assigned to by a super_admin).
pub async fn list_by_owner(pool: &PgPool, owner_user_id: Uuid) -> Result<Vec<Site>> {
    sqlx::query_as::<_, Site>(
        "SELECT * FROM sites WHERE owner_user_id = $1 ORDER BY created_at ASC",
    )
    .bind(owner_user_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
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
