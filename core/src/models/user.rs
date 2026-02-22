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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

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

pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE users SET is_active = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
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
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}
