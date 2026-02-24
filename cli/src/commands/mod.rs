pub mod dev;
pub mod install;
pub mod migrate;
pub mod plugin;
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
