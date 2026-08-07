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

/// Number of users holding the 'admin' role on a site — used to warn before
/// removing/demoting the last one, since that leaves the site with no one
/// (other than a super_admin) able to manage it.
pub async fn count_admins(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_users WHERE site_id = $1 AND role = 'admin'",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// If exactly one user holds the 'admin' role on this site, return their id —
/// regardless of whether they are also `sites.owner_user_id`. Used to warn
/// before demoting/removing the last admin, since an additional admin (added
/// via "add as additional Site Admin", or left over after the owner was
/// removed) is not reflected by `sites.owner_user_id` at all.
pub async fn sole_admin(pool: &PgPool, site_id: Uuid) -> Result<Option<Uuid>> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM site_users WHERE site_id = $1 AND role = 'admin'",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(if ids.len() == 1 { Some(ids[0]) } else { None })
}

/// Hostnames of sites where `user_id` is currently the sole 'admin' — used to
/// block deleting/demoting a user when doing so would leave a site with no
/// admin at all (site ownership is required from creation onward, so it must
/// not be possible to remove the last admin either).
pub async fn sole_admin_hostnames(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>> {
    let hostnames: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT s.hostname
        FROM site_users su
        JOIN sites s ON s.id = su.site_id
        WHERE su.user_id = $1
          AND su.role = 'admin'
          AND (SELECT COUNT(*) FROM site_users su2 WHERE su2.site_id = su.site_id AND su2.role = 'admin') = 1
        ORDER BY s.hostname ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(hostnames)
}

/// Batch version of `sole_admin_hostnames` for rendering a user list — one
/// query for every user in `user_ids` instead of one per row, so the Users
/// page can preflight-disable Delete for anyone who's a site's only admin.
pub async fn sole_admin_hostnames_batch(
    pool: &PgPool,
    user_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<String>>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT su.user_id, s.hostname
        FROM site_users su
        JOIN sites s ON s.id = su.site_id
        WHERE su.user_id = ANY($1)
          AND su.role = 'admin'
          AND (SELECT COUNT(*) FROM site_users su2 WHERE su2.site_id = su.site_id AND su2.role = 'admin') = 1
        ORDER BY s.hostname ASC
        "#,
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await?;
    let mut map: std::collections::HashMap<Uuid, Vec<String>> = std::collections::HashMap::new();
    for (uid, hostname) in rows {
        map.entry(uid).or_default().push(hostname);
    }
    Ok(map)
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
    default_site_id: Option<Uuid>,
    site_role: String,
}

/// List all users and their roles for a given site.
/// Excludes soft-deleted users.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<(User, String)>> {
    let rows = sqlx::query_as::<_, UserWithSiteRole>(
        r#"
        SELECT u.id, u.username, u.email, u.display_name, u.password_hash, u.bio,
               u.avatar_media_id, u.role, u.is_active, u.is_protected,
               u.created_at, u.updated_at, u.deleted_at, u.default_site_id,
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
                default_site_id: r.default_site_id,
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

/// List the sites a user should see in their own admin session: the site
/// they're currently logged into (regardless of role there), plus any other
/// sites where they hold the site-owner-level 'admin' role. Editor/author
/// roles on other sites stay confined to logging into that site directly.
pub async fn list_for_user_scoped(
    pool: &PgPool,
    user_id: Uuid,
    current_site_id: Option<Uuid>,
) -> Result<Vec<(Site, String)>> {
    let rows = sqlx::query_as::<_, SiteWithRole>(
        r#"
        SELECT s.id, s.hostname, s.owner_user_id, s.created_at, s.updated_at,
               su.role as site_role
        FROM sites s
        JOIN site_users su ON su.site_id = s.id
        WHERE su.user_id = $1
          AND (su.role = 'admin' OR s.id = $2)
        ORDER BY s.created_at ASC
        "#,
    )
    .bind(user_id)
    .bind(current_site_id)
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

/// List all sites a user has access to, with their role on each site.
/// Used for management views that need the full membership picture (e.g.
/// viewing another user's site-role assignments), not for scoping the
/// current viewer's own admin session — see `list_for_user_scoped` for that.
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
