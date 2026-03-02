//! Per-site plugin activation records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SitePlugin {
    pub site_id: Uuid,
    pub plugin_name: String,
    pub active: bool,
    pub installed_at: DateTime<Utc>,
}

/// Record a plugin as installed for a site (active = false by default).
/// Idempotent: does nothing if already installed.
pub async fn install(pool: &PgPool, site_id: Uuid, plugin_name: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO site_plugins (site_id, plugin_name, active) VALUES ($1, $2, false)
         ON CONFLICT (site_id, plugin_name) DO NOTHING",
    )
    .bind(site_id)
    .bind(plugin_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Set a plugin's active flag to true for a site.
/// The plugin must already be installed (row must exist).
pub async fn activate(pool: &PgPool, site_id: Uuid, plugin_name: &str) -> Result<()> {
    sqlx::query(
        "UPDATE site_plugins SET active = true WHERE site_id = $1 AND plugin_name = $2",
    )
    .bind(site_id)
    .bind(plugin_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Set a plugin's active flag to false for a site.
pub async fn deactivate(pool: &PgPool, site_id: Uuid, plugin_name: &str) -> Result<()> {
    sqlx::query(
        "UPDATE site_plugins SET active = false WHERE site_id = $1 AND plugin_name = $2",
    )
    .bind(site_id)
    .bind(plugin_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a plugin record for a site.
pub async fn delete(pool: &PgPool, site_id: Uuid, plugin_name: &str) -> Result<()> {
    sqlx::query("DELETE FROM site_plugins WHERE site_id = $1 AND plugin_name = $2")
        .bind(site_id)
        .bind(plugin_name)
        .execute(pool)
        .await?;
    Ok(())
}

/// List all installed plugins for a site.
pub async fn list_for_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<SitePlugin>> {
    let rows: Vec<SitePlugin> = sqlx::query_as(
        "SELECT site_id, plugin_name, active, installed_at
         FROM site_plugins
         WHERE site_id = $1
         ORDER BY plugin_name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Check whether a specific plugin is active for a site.
pub async fn is_active(pool: &PgPool, site_id: Uuid, plugin_name: &str) -> Result<bool> {
    let active: Option<bool> = sqlx::query_scalar(
        "SELECT active FROM site_plugins WHERE site_id = $1 AND plugin_name = $2",
    )
    .bind(site_id)
    .bind(plugin_name)
    .fetch_optional(pool)
    .await?;
    Ok(active.unwrap_or(false))
}

/// Return the names of all active plugins for a site.
pub async fn active_plugin_names(pool: &PgPool, site_id: Uuid) -> Result<Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT plugin_name FROM site_plugins WHERE site_id = $1 AND active = true ORDER BY plugin_name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(names)
}
