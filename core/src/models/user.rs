use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

/// User roles in order of increasing privilege.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    #[sqlx(rename = "subscriber")]
    Subscriber,
    #[sqlx(rename = "author")]
    Author,
    #[sqlx(rename = "editor")]
    Editor,
    /// Owns and administers one or more sites. Accesses admin via site_users role.
    #[sqlx(rename = "site_admin")]
    SiteAdmin,
    /// Agency-level super-admin. Unrestricted access to all sites; bypasses site_users.
    #[sqlx(rename = "super_admin")]
    SuperAdmin,
}

#[allow(dead_code)]
impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Subscriber => "subscriber",
            UserRole::Author => "author",
            UserRole::Editor => "editor",
            UserRole::SiteAdmin => "site_admin",
            UserRole::SuperAdmin => "super_admin",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "subscriber" => Some(UserRole::Subscriber),
            "author" => Some(UserRole::Author),
            "editor" => Some(UserRole::Editor),
            "site_admin" => Some(UserRole::SiteAdmin),
            "super_admin" => Some(UserRole::SuperAdmin),
            _ => None,
        }
    }

    pub fn can_publish(&self) -> bool {
        matches!(self, UserRole::Author | UserRole::Editor | UserRole::SiteAdmin | UserRole::SuperAdmin)
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, UserRole::SuperAdmin)
    }
}

/// Generate an 8-char password satisfying `validate_password`'s rule: 1
/// uppercase, 1 digit, 1 symbol from `!@#$%&`, rest lowercase, shuffled.
/// Used to seed accounts nobody has typed a password for yet (e.g. WP
/// author import) — the caller is responsible for getting it to the user.
pub fn generate_password() -> String {
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::from_entropy();
    let lower = b"abcdefghijklmnopqrstuvwxyz";
    let symbols = b"!@#$%&";
    let mut chars: Vec<char> = Vec::with_capacity(8);
    chars.push((lower[rng.gen_range(0..lower.len())] as char).to_ascii_uppercase());
    chars.push(char::from_digit(rng.gen_range(0..10), 10).unwrap());
    chars.push(symbols[rng.gen_range(0..symbols.len())] as char);
    for _ in 0..5 {
        chars.push(lower[rng.gen_range(0..lower.len())] as char);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}

/// Validate a plaintext password against site-wide requirements.
///
/// Rules: 8–12 characters, at least one uppercase letter, at least one digit,
/// and at least one symbol from the allowed set `!@#$%&`.
pub fn validate_password(password: &str) -> std::result::Result<(), &'static str> {
    let len = password.len();
    if len < 8 {
        return Err("Password must be at least 8 characters");
    }
    if len > 12 {
        return Err("Password must be no more than 12 characters");
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err("Password must contain at least one uppercase letter");
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one number");
    }
    const ALLOWED_SYMBOLS: &[char] = &['!', '@', '#', '$', '%', '&'];
    if !password.chars().any(|c| ALLOWED_SYMBOLS.contains(&c)) {
        return Err("Password must contain at least one symbol: ! @ # $ % &");
    }
    Ok(())
}

/// Validate a username against site-wide requirements.
///
/// Rules: 5–15 characters, lowercase letters/digits/hyphens only, and cannot
/// start or end with a hyphen (the only symbol the character set allows).
pub fn validate_username(username: &str) -> std::result::Result<(), &'static str> {
    let len = username.len();
    if len < 5 {
        return Err("Username must be at least 5 characters");
    }
    if len > 15 {
        return Err("Username must be no more than 15 characters");
    }
    if !username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("Username may only contain lowercase letters, numbers and hyphens");
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err("Username cannot start or end with a symbol");
    }
    Ok(())
}

/// Validate a display name against a sane length ceiling. No character
/// restrictions — legal names can contain spaces, apostrophes, non-ASCII,
/// etc. — only length is bounded, since an unbounded name breaks layout in
/// admin lists/author URLs/email templates (e.g. a 250-char string).
/// Does not check emptiness — callers already handle that with their own
/// "required" message.
pub fn validate_display_name(display_name: &str) -> std::result::Result<(), &'static str> {
    if display_name.chars().count() > 60 {
        return Err("Display name must be no more than 60 characters");
    }
    Ok(())
}

/// Full user record — never expose password_hash over the API or in templates.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: String,
    /// NEVER include this in template context.
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub bio: String,
    pub avatar_media_id: Option<Uuid>,
    pub role: String,
    pub is_active: bool,
    pub is_protected: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Non-NULL = soft-deleted. User cannot log in; their content is preserved.
    pub deleted_at: Option<DateTime<Utc>>,
    /// The user's preferred/default site. NULL until first site is created.
    pub default_site_id: Option<Uuid>,
}

#[allow(dead_code)]
impl User {
    pub fn role(&self) -> UserRole {
        UserRole::from_str(&self.role).unwrap_or(UserRole::Subscriber)
    }

    /// Returns true if this user's password hash matches the given plaintext password.
    pub fn verify_password(&self, password: &str) -> bool {
        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        let hash = match PasswordHash::new(&self.password_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        Argon2::default().verify_password(password.as_bytes(), &hash).is_ok()
    }
}

/// Subset of User safe for template context — no password hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub role: String,
    pub url: String,
}

impl UserContext {
    pub fn from_user(user: &User, base_url: &str) -> Self {
        UserContext {
            id: user.id.to_string(),
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            bio: user.bio.clone(),
            role: user.role.clone(),
            url: format!("{}/author/{}", base_url, user.username),
        }
    }
}

/// Data required to create a new user.
#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub role: UserRole,
}

/// Data for updating an existing user.
#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    pub username: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub password_hash: Option<String>,
    pub role: Option<UserRole>,
    pub bio: Option<String>,
}

pub async fn create(pool: &PgPool, data: &CreateUser) -> Result<User> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(data.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))?
        .to_string();

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email, display_name, password_hash, role)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(&data.username)
    .bind(&data.email)
    .bind(&data.display_name)
    .bind(&password_hash)
    .bind(data.role.as_str())
    .fetch_one(pool)
    .await?;

    Ok(user)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND is_active = TRUE AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {id}")))
}

/// Like `get_by_id`, but also finds suspended (`is_active = FALSE`) accounts —
/// needed anywhere a suspended user must still be reachable by ID: viewing/
/// editing their admin profile, or the suspend/reactivate guard checks below.
pub async fn get_by_id_include_inactive(pool: &PgPool, id: Uuid) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {id}")))
}

pub async fn get_by_username(pool: &PgPool, username: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = $1 AND is_active = TRUE AND deleted_at IS NULL",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user '{username}'")))
}

/// Like `get_by_username`, but also finds suspended (`is_active = FALSE`) accounts —
/// needed for public author pages, where suspension should not hide already-published content.
pub async fn get_by_username_include_inactive(pool: &PgPool, username: &str) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1 AND deleted_at IS NULL")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user '{username}'")))
}

/// Like `get_by_username_include_inactive`, but scoped to members of one site.
/// Usernames are only unique *within* a site's membership (see
/// `username_available`), so a global-only lookup could resolve to the wrong
/// same-named user on a different site — this is what public author pages
/// (`/author/{{username}}`) must use instead.
pub async fn get_by_username_in_site(pool: &PgPool, site_id: Uuid, username: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT u.* FROM users u
        JOIN site_users su ON su.user_id = u.id
        WHERE su.site_id = $1 AND u.username = $2 AND u.deleted_at IS NULL
        "#,
    )
    .bind(site_id)
    .bind(username)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user '{username}'")))
}

/// Returns true if `username` is free to use by a user who belongs (or is
/// about to belong) to any of `site_ids`. Usernames are no longer globally
/// unique — `users.username` has no DB uniqueness constraint — since two
/// independent site owners' accounts shouldn't have to fight over a shared
/// namespace they don't know exists. Instead, a username only needs to be
/// unique among users who actually share a site: that's what disambiguates
/// public author pages and admin user lists within one site's context.
/// `exclude_user_id` skips the user's own current row (for edits — renaming
/// yourself to your own existing username, or leaving it unchanged, isn't a
/// collision). An empty `site_ids` means the user isn't tied to any site yet,
/// so there's nothing to collide with.
pub async fn username_available(
    pool: &PgPool,
    username: &str,
    site_ids: &[Uuid],
    exclude_user_id: Option<Uuid>,
) -> Result<bool> {
    if site_ids.is_empty() {
        return Ok(true);
    }
    let taken: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM users u
            JOIN site_users su ON su.user_id = u.id
            WHERE u.username = $1
              AND su.site_id = ANY($2)
              AND u.deleted_at IS NULL
              AND ($3::uuid IS NULL OR u.id != $3)
        )
        "#,
    )
    .bind(username)
    .bind(site_ids)
    .bind(exclude_user_id)
    .fetch_one(pool)
    .await?;
    Ok(!taken)
}

pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = TRUE AND deleted_at IS NULL",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user with email '{email}'")))
}

#[allow(dead_code)]
pub async fn update_role(pool: &PgPool, id: Uuid, role: &UserRole) -> Result<()> {
    let affected = sqlx::query(
        "UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(role.as_str())
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!("user {id}")));
    }
    Ok(())
}

/// Suspend a user — blocks login (every login lookup filters `is_active = TRUE`)
/// without touching their content, unlike `soft_delete`.
pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE users SET is_active = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reverse `deactivate` — restores login access.
pub async fn reactivate(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE users SET is_active = TRUE, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Soft-delete a user. Their content (posts, pages, media) is preserved.
/// The user can no longer log in and will not appear in any admin list.
/// Use `delete()` or `delete_and_reassign()` only when hard removal is explicitly required.
pub async fn soft_delete(pool: &PgPool, id: Uuid) -> Result<()> {
    let affected = sqlx::query(
        "UPDATE users SET deleted_at = NOW(), is_active = FALSE, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!("user {id}")));
    }
    Ok(())
}

/// Permanently delete a user and all their posts/pages (cascades post_meta and post_taxonomies).
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM posts WHERE author_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Reassign all posts and media from `user_id` to `reassign_to`, then delete the user.
/// Use this instead of `delete()` when content must be preserved — the deleted
/// user's posts and media transfer to the reassignment target before the row is removed.
pub async fn delete_and_reassign(pool: &PgPool, user_id: Uuid, reassign_to: Uuid) -> Result<()> {
    sqlx::query("UPDATE posts SET author_id = $1 WHERE author_id = $2")
        .bind(reassign_to)
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE media SET uploaded_by = $1 WHERE uploaded_by = $2")
        .bind(reassign_to)
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list(pool: &PgPool) -> Result<Vec<User>> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE is_active = TRUE AND deleted_at IS NULL ORDER BY username",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Like `list`, but includes suspended (`is_active = FALSE`) accounts — for
/// the admin Users page, where a suspended user must still be visible so an
/// admin can reactivate them. `list` (active-only) stays the right choice
/// for contexts like assignable-user dropdowns, where a suspended account
/// shouldn't be selectable.
pub async fn list_all(pool: &PgPool) -> Result<Vec<User>> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE deleted_at IS NULL ORDER BY username",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn count(pool: &PgPool) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE is_active = TRUE AND deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Returns how many active super-admin accounts exist.
pub async fn count_global_admins(pool: &PgPool) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'super_admin' AND is_active = TRUE AND deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Returns the number of users assigned to a specific site via site_users,
/// excluding super_admins and the site admin (role = 'admin' in site_users).
pub async fn count_for_site(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_users su
         JOIN users u ON u.id = su.user_id
         WHERE su.site_id = $1
           AND su.role != 'admin'
           AND u.role != 'super_admin'
           AND u.deleted_at IS NULL",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Count of active "staff" users system-wide (admins, editors, authors),
/// excluding subscribers (they get their own Subscribers card) and the
/// viewing user themselves — a super_admin's own account shouldn't count
/// toward the total they see on their own dashboard.
pub async fn count_staff(pool: &PgPool, exclude_user_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users
         WHERE role != 'subscriber' AND id != $1 AND is_active = TRUE AND deleted_at IS NULL",
    )
    .bind(exclude_user_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Count of active subscriber users system-wide.
pub async fn count_subscribers(pool: &PgPool) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'subscriber' AND is_active = TRUE AND deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Returns the number of staff users (site_users.role != 'subscriber')
/// assigned to a specific site, excluding super_admins (they aren't really
/// "on the team") and the viewing user themselves — a site admin's own
/// account shouldn't count toward the total they see on their own dashboard.
pub async fn count_staff_for_site(pool: &PgPool, site_id: Uuid, exclude_user_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_users su
         JOIN users u ON u.id = su.user_id
         WHERE su.site_id = $1
           AND su.role != 'subscriber'
           AND u.role != 'super_admin'
           AND u.id != $2
           AND u.deleted_at IS NULL",
    )
    .bind(site_id)
    .bind(exclude_user_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Returns the number of subscriber users (site_users.role = 'subscriber')
/// assigned to a specific site. Mirrors the "subscribers" tab on the Users
/// admin page.
pub async fn count_subscribers_for_site(pool: &PgPool, site_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_users su
         JOIN users u ON u.id = su.user_id
         WHERE su.site_id = $1
           AND su.role = 'subscriber'
           AND u.deleted_at IS NULL",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

pub async fn update(pool: &PgPool, id: Uuid, data: &UpdateUser) -> Result<User> {
    let current = get_by_id(pool, id).await?;

    let new_username = data.username.clone().unwrap_or(current.username);
    let new_email = data.email.clone().unwrap_or(current.email);
    let new_display_name = data.display_name.clone().unwrap_or(current.display_name);
    let new_password_hash = data.password_hash.clone().unwrap_or(current.password_hash);
    let new_role = data.role.as_ref().map(|r| r.as_str().to_string()).unwrap_or(current.role);
    let new_bio = data.bio.clone().unwrap_or(current.bio);

    let user = sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET username = $1, email = $2, display_name = $3, password_hash = $4,
            role = $5, bio = $6, updated_at = NOW()
        WHERE id = $7
        RETURNING *
        "#,
    )
    .bind(&new_username)
    .bind(&new_email)
    .bind(&new_display_name)
    .bind(&new_password_hash)
    .bind(&new_role)
    .bind(&new_bio)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Hash a plaintext password using Argon2.
pub fn hash_password(password: &str) -> Result<String> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))
}

/// True when `site_id` is some super_admin's own default/home site — the
/// one they manage System Settings (and the agency-wide logo) from. Used to
/// decide whether the global logo may show on a top-level site's login page:
/// it's the agency's own site, not a white-labeled client's, so no
/// misattribution concern there.
pub async fn is_super_admin_default_site(pool: &PgPool, site_id: Uuid) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'super_admin' AND default_site_id = $1)",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

/// Set (or clear) a user's default site. Pass `None` to clear.
pub async fn set_default_site(pool: &PgPool, user_id: Uuid, site_id: Option<Uuid>) -> Result<()> {
    sqlx::query("UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2")
        .bind(site_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_dashboard_widget_layout(pool: &PgPool, user_id: Uuid) -> Result<Option<serde_json::Value>> {
    let layout: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT dashboard_widget_layout FROM users WHERE id = $1"
    ).bind(user_id).fetch_one(pool).await?;
    Ok(layout)
}

pub async fn set_dashboard_widget_layout(pool: &PgPool, user_id: Uuid, layout: &serde_json::Value) -> Result<()> {
    sqlx::query("UPDATE users SET dashboard_widget_layout = $1, updated_at = NOW() WHERE id = $2")
        .bind(layout)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Verify a plaintext password against a stored Argon2 hash.
#[allow(dead_code)]
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn user_role_as_str_all_variants() {
        assert_eq!(UserRole::Subscriber.as_str(), "subscriber");
        assert_eq!(UserRole::Author.as_str(), "author");
        assert_eq!(UserRole::Editor.as_str(), "editor");
        assert_eq!(UserRole::SiteAdmin.as_str(), "site_admin");
        assert_eq!(UserRole::SuperAdmin.as_str(), "super_admin");
    }

    #[test]
    fn user_role_from_str_valid_values() {
        assert_eq!(UserRole::from_str("subscriber"), Some(UserRole::Subscriber));
        assert_eq!(UserRole::from_str("author"), Some(UserRole::Author));
        assert_eq!(UserRole::from_str("editor"), Some(UserRole::Editor));
        assert_eq!(UserRole::from_str("site_admin"), Some(UserRole::SiteAdmin));
        assert_eq!(UserRole::from_str("super_admin"), Some(UserRole::SuperAdmin));
    }

    #[test]
    fn user_role_from_str_invalid_returns_none() {
        assert_eq!(UserRole::from_str("superuser"), None);
        assert_eq!(UserRole::from_str(""), None);
        assert_eq!(UserRole::from_str("root"), None);
    }

    #[test]
    fn user_role_from_str_admin_no_longer_valid() {
        // "admin" was the old role string; it must no longer parse to a valid variant.
        assert_eq!(UserRole::from_str("admin"), None);
    }

    #[test]
    fn user_role_from_str_case_sensitive() {
        assert_eq!(UserRole::from_str("SuperAdmin"), None);
        assert_eq!(UserRole::from_str("SUPER_ADMIN"), None);
        assert_eq!(UserRole::from_str("Author"), None);
    }

    #[test]
    fn validate_password_accepts_valid() {
        assert!(validate_password("Secure1!").is_ok());
        assert!(validate_password("Hello1!ab").is_ok());
        assert!(validate_password("Abcdef1@").is_ok());
    }

    #[test]
    fn validate_password_rejects_too_short() {
        assert!(validate_password("Ab1!").is_err());
    }

    #[test]
    fn validate_password_rejects_too_long() {
        assert!(validate_password("Abcdefgh1!xxx").is_err()); // 13 chars
    }

    #[test]
    fn validate_password_rejects_no_uppercase() {
        assert!(validate_password("secure1!abc").is_err());
    }

    #[test]
    fn validate_password_rejects_no_digit() {
        assert!(validate_password("SecureAb!").is_err());
    }

    #[test]
    fn validate_password_rejects_no_symbol() {
        assert!(validate_password("Secure1abc").is_err());
    }

    #[test]
    fn validate_password_rejects_disallowed_symbol() {
        // ^ is not in the allowed set !@#$%&
        assert!(validate_password("Secure1^").is_err());
    }

    #[test]
    fn can_publish_for_author_editor_site_admin_super_admin() {
        assert!(UserRole::Author.can_publish());
        assert!(UserRole::Editor.can_publish());
        assert!(UserRole::SiteAdmin.can_publish());
        assert!(UserRole::SuperAdmin.can_publish());
    }

    #[test]
    fn can_publish_false_for_subscriber() {
        assert!(!UserRole::Subscriber.can_publish());
    }

    #[test]
    fn can_manage_users_super_admin_only() {
        assert!(UserRole::SuperAdmin.can_manage_users());
        assert!(!UserRole::Editor.can_manage_users());
        assert!(!UserRole::Author.can_manage_users());
        assert!(!UserRole::Subscriber.can_manage_users());
    }

    #[test]
    fn hash_and_verify_password_round_trip() {
        let hash = hash_password("correct-horse-battery").unwrap();
        assert!(verify_password("correct-horse-battery", &hash));
        assert!(!verify_password("wrong-password", &hash));
    }

    #[test]
    fn hash_password_produces_unique_salts() {
        let hash1 = hash_password("samepassword").unwrap();
        let hash2 = hash_password("samepassword").unwrap();
        assert_ne!(hash1, hash2, "each hash should use a unique salt");
    }

    #[test]
    fn user_context_url_format() {
        let user = User {
            id: Uuid::new_v4(),
            username: "janedoe".to_string(),
            email: "jane@example.com".to_string(),
            display_name: "Jane Doe".to_string(),
            password_hash: "hash".to_string(),
            bio: "".to_string(),
            avatar_media_id: None,
            role: "author".to_string(),
            is_active: true,
            is_protected: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
            default_site_id: None,
        };
        let ctx = UserContext::from_user(&user, "https://example.com");
        assert_eq!(ctx.url, "https://example.com/author/janedoe");
        assert_eq!(ctx.username, "janedoe");
        assert_eq!(ctx.role, "author");
        // password_hash must NOT be present in UserContext
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(!json.contains("password_hash"));
        assert!(!json.contains("hash"));
    }
}
