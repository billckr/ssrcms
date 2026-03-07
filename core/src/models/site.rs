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
    let mut tx = pool.begin().await?;

    // Identify users who will become orphaned: they only belong to this site
    // and have no other site assignments. Exclude super_admins (they have
    // global access and no site_users rows).
    let orphan_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT su.user_id
           FROM site_users su
           JOIN users u ON u.id = su.user_id
           WHERE su.site_id = $1
             AND u.role != 'super_admin'
             AND u.deleted_at IS NULL
             AND NOT EXISTS (
               SELECT 1 FROM site_users su2
               WHERE su2.user_id = su.user_id
                 AND su2.site_id != $1
             )"#,
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    // Delete the site. ON DELETE CASCADE handles: posts, pages, media,
    // taxonomies, site_settings, site_users, comments, form_submissions,
    // site_plugins for this site.
    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Delete users who only belonged to this site (now fully orphaned).
    // Multi-site users survive — their other site_users rows are untouched.
    if !orphan_ids.is_empty() {
        sqlx::query("DELETE FROM users WHERE id = ANY($1)")
            .bind(&orphan_ids)
            .execute(&mut *tx)
            .await?;
    }

    // Clear default_site_id for any surviving users who still point to the
    // now-deleted site (multi-site users who had it as their home).
    sqlx::query(
        "UPDATE users SET default_site_id = NULL, updated_at = NOW() WHERE default_site_id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Count published posts for a site (used in site listing).
pub async fn post_count(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM posts WHERE site_id = $1 AND post_type = 'post'",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Count pages for a site (used in site listing).
pub async fn page_count(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM posts WHERE site_id = $1 AND post_type = 'page'",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Email of the site owner (any role), if one is assigned.
pub async fn admin_email(pool: &PgPool, site_id: Uuid) -> Result<Option<String>> {
    let email: Option<String> = sqlx::query_scalar(
        r#"SELECT u.email
           FROM sites s
           JOIN users u ON u.id = s.owner_user_id
           WHERE s.id = $1
             AND u.deleted_at IS NULL"#,
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await?;
    Ok(email)
}

/// Count of non-subscriber users assigned to a site (editors, authors, admins).
/// Excludes super_admins — they have global access and are not site members.
pub async fn user_count(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM site_users su
           JOIN users u ON u.id = su.user_id
           WHERE su.site_id = $1
             AND su.role != 'subscriber'
             AND u.role != 'super_admin'
             AND u.deleted_at IS NULL"#,
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Count of subscribers assigned to a site.
pub async fn subscriber_count(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM site_users su
           JOIN users u ON u.id = su.user_id
           WHERE su.site_id = $1
             AND su.role = 'subscriber'
             AND u.deleted_at IS NULL"#,
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}
