pub mod app;
pub mod caddy;
pub mod dev;
pub mod install;
pub mod migrate;
pub mod plugin;
pub mod security;
pub mod site;
pub mod theme;
pub mod user;

use sqlx::postgres::PgPoolOptions;

/// Connect to the database using DATABASE_URL from the environment.
pub async fn connect_db() -> anyhow::Result<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set. Pass it as an env var or create a .env file."))?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("Database connection failed: {e}\nCheck DATABASE_URL is correct and PostgreSQL is running."))?;
    Ok(pool)
}

/// Verify a supplied password against the current super_admin's password
/// hash. Used to gate sensitive CLI operations (creating users, wiping dev
/// data) so having server/DB access alone isn't sufficient to perform them —
/// the operator must also know the super-admin password.
///
/// `supplied` bypasses the interactive prompt (for scripting); pass `None`
/// to prompt interactively. Errors (rather than bailing silently) if no
/// super_admin exists yet — that means `synap install` hasn't been run.
pub async fn verify_super_admin_password(
    pool: &sqlx::PgPool,
    supplied: Option<String>,
) -> anyhow::Result<()> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error looking up super_admin: {e}"))?;

    let hash = match row {
        Some((h,)) => h,
        None => anyhow::bail!(
            "No super_admin found. Run 'synap install' to set up a fresh installation."
        ),
    };

    let password = match supplied {
        Some(p) => p,
        None => dialoguer::Password::new()
            .with_prompt("Super-admin password")
            .interact()
            .map_err(|e| anyhow::anyhow!("Password prompt failed: {e}"))?,
    };

    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };
    let parsed = PasswordHash::new(&hash)
        .map_err(|e| anyhow::anyhow!("Invalid password hash in DB: {e}"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("Incorrect password."))?;

    Ok(())
}
