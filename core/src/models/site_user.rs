//! Per-site user role assignments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;
use crate::models::site::Site;
use crate::models::user::User;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SiteUser {
    pub site_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    /// Who added this user to this site. NULL for legacy / CLI-seeded rows.
    pub invited_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Add (or update role of) a user on a site, recording who did the inviting.
/// Pass `invited_by: None` for CLI-seeded rows or super_admin-initiated entries
/// where attribution is not required.
pub async fn add(
    pool: &PgPool,
    site_id: Uuid,
    user_id: Uuid,
    role: &str,
    invited_by: Option<Uuid>,
) -> Result<SiteUser> {
    let su = sqlx::query_as::<_, SiteUser>(
        r#"
        INSERT INTO site_users (site_id, user_id, role, invited_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (site_id, user_id) DO UPDATE SET role = EXCLUDED.role
        RETURNING *
        "#,
    )
    .bind(site_id)
    .bind(user_id)
    .bind(role)
    .bind(invited_by)
    .fetch_one(pool)
    .await?;
    Ok(su)
}

pub async fn remove(pool: &PgPool, site_id: Uuid, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM site_users WHERE site_id = $1 AND user_id = $2")
        .bind(site_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_role(pool: &PgPool, site_id: Uuid, user_id: Uuid) -> Result<Option<String>> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM site_users WHERE site_id = $1 AND user_id = $2",
    )
    .bind(site_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(role)
}

pub async fn update_role(pool: &PgPool, site_id: Uuid, user_id: Uuid, role: &str) -> Result<()> {
    sqlx::query("UPDATE site_users SET role = $1 WHERE site_id = $2 AND user_id = $3")
        .bind(role)
        .bind(site_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Raw row for list_for_site join query.
#[derive(sqlx::FromRow)]
struct UserWithSiteRole {
    id: Uuid,
    username: String,
    email: String,
    display_name: String,
    password_hash: String,
    bio: String,
    avatar_media_id: Option<Uuid>,
    role: String,
    is_active: bool,
    is_protected: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    site_role: String,
}

/// List all users and their roles for a given site.
/// Excludes soft-deleted users.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<(User, String)>> {
    let rows = sqlx::query_as::<_, UserWithSiteRole>(
        r#"
        SELECT u.id, u.username, u.email, u.display_name, u.password_hash, u.bio,
               u.avatar_media_id, u.role, u.is_active, u.is_protected,
               u.created_at, u.updated_at, u.deleted_at,
               su.role as site_role
        FROM users u
        JOIN site_users su ON su.user_id = u.id
        WHERE su.site_id = $1
          AND u.deleted_at IS NULL
        ORDER BY u.username
        "#,
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let user = User {
                id: r.id,
                username: r.username,
                email: r.email,
                display_name: r.display_name,
                password_hash: r.password_hash,
                bio: r.bio,
                avatar_media_id: r.avatar_media_id,
                role: r.role,
                is_active: r.is_active,
                is_protected: r.is_protected,
                created_at: r.created_at,
                updated_at: r.updated_at,
                deleted_at: r.deleted_at,
            };
            (user, r.site_role)
        })
        .collect())
}

/// Raw row for list_for_user join query.
#[derive(sqlx::FromRow)]
struct SiteWithRole {
    id: Uuid,
    hostname: String,
    owner_user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    site_role: String,
}

/// List all sites a user has access to, with their role on each site.
pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<(Site, String)>> {
    let rows = sqlx::query_as::<_, SiteWithRole>(
        r#"
        SELECT s.id, s.hostname, s.owner_user_id, s.created_at, s.updated_at,
               su.role as site_role
        FROM sites s
        JOIN site_users su ON su.site_id = s.id
        WHERE su.user_id = $1
        ORDER BY s.created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let site = Site {
                id: r.id,
                hostname: r.hostname,
                owner_user_id: r.owner_user_id,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            (site, r.site_role)
        })
        .collect())
}
