use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::errors::{AppError, Result};

/// Create and return a PostgreSQL connection pool.
pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
        .map_err(|e| AppError::Config(format!("database connection failed: {e}")))?;

    Ok(pool)
}

/// Run all pending sqlx migrations.
pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("../migrations")
        .run(pool)
        .await
        .map_err(|e: sqlx::migrate::MigrateError| AppError::Config(format!("migration failed: {e}")))?;
    Ok(())
}
