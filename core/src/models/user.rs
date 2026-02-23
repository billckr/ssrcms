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
    #[sqlx(rename = "admin")]
    Admin,
}

#[allow(dead_code)]
impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Subscriber => "subscriber",
            UserRole::Author => "author",
            UserRole::Editor => "editor",
            UserRole::Admin => "admin",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "subscriber" => Some(UserRole::Subscriber),
            "author" => Some(UserRole::Author),
            "editor" => Some(UserRole::Editor),
            "admin" => Some(UserRole::Admin),
            _ => None,
        }
    }

    pub fn can_publish(&self) -> bool {
        matches!(self, UserRole::Author | UserRole::Editor | UserRole::Admin)
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
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
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND is_active = TRUE")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {id}")))
}

pub async fn get_by_username(pool: &PgPool, username: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = $1 AND is_active = TRUE",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user '{username}'")))
}

pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = TRUE",
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

#[allow(dead_code)]
pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE users SET is_active = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
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

/// Reassign all posts from `user_id` to `reassign_to`, then delete the user.
/// Use this instead of `delete()` when content must be preserved — the deleted
/// user's posts transfer to the reassignment target before the row is removed.
pub async fn delete_and_reassign(pool: &PgPool, user_id: Uuid, reassign_to: Uuid) -> Result<()> {
    sqlx::query("UPDATE posts SET author_id = $1 WHERE author_id = $2")
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
        "SELECT * FROM users WHERE is_active = TRUE ORDER BY username",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn count(pool: &PgPool) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE is_active = TRUE",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Returns how many active global-admin accounts exist.
pub async fn count_global_admins(pool: &PgPool) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE",
    )
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
        assert_eq!(UserRole::Admin.as_str(), "admin");
    }

    #[test]
    fn user_role_from_str_valid_values() {
        assert_eq!(UserRole::from_str("subscriber"), Some(UserRole::Subscriber));
        assert_eq!(UserRole::from_str("author"), Some(UserRole::Author));
        assert_eq!(UserRole::from_str("editor"), Some(UserRole::Editor));
        assert_eq!(UserRole::from_str("admin"), Some(UserRole::Admin));
    }

    #[test]
    fn user_role_from_str_invalid_returns_none() {
        assert_eq!(UserRole::from_str("superuser"), None);
        assert_eq!(UserRole::from_str(""), None);
        assert_eq!(UserRole::from_str("root"), None);
    }

    #[test]
    fn user_role_from_str_case_sensitive() {
        assert_eq!(UserRole::from_str("Admin"), None);
        assert_eq!(UserRole::from_str("ADMIN"), None);
        assert_eq!(UserRole::from_str("Author"), None);
    }

    #[test]
    fn can_publish_for_author_editor_admin() {
        assert!(UserRole::Author.can_publish());
        assert!(UserRole::Editor.can_publish());
        assert!(UserRole::Admin.can_publish());
    }

    #[test]
    fn can_publish_false_for_subscriber() {
        assert!(!UserRole::Subscriber.can_publish());
    }

    #[test]
    fn can_manage_users_admin_only() {
        assert!(UserRole::Admin.can_manage_users());
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
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
