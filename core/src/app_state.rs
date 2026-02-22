//! Shared application state passed to every Axum handler via State extractor.

use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;
use crate::plugins::manifest::{PluginManifest, RouteRegistration};
use crate::search::SearchIndex;
use crate::templates::TemplateEngine;

/// Runtime site settings loaded from the site_settings table.
/// Cached on startup; reloaded on config change (Phase 3).
#[derive(Debug, Clone)]
pub struct SiteSettings {
    pub site_name: String,
    pub site_description: String,
    pub base_url: String,
    pub language: String,
    pub active_theme: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

impl SiteSettings {
    pub async fn load(pool: &PgPool) -> anyhow::Result<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM site_settings")
            .fetch_all(pool)
            .await?;

        let mut map: HashMap<String, String> = rows.into_iter().collect();

        Ok(SiteSettings {
            site_name: map.remove("site_name").unwrap_or_else(|| "Synaptic Signals".into()),
            site_description: map
                .remove("site_description")
                .unwrap_or_else(|| "Fast by default, secure by design".into()),
            base_url: map.remove("site_url").unwrap_or_else(|| "http://localhost:3000".into()),
            language: map.remove("site_language").unwrap_or_else(|| "en-US".into()),
            active_theme: map.remove("active_theme").unwrap_or_else(|| "default".into()),
            posts_per_page: map
                .remove("posts_per_page")
                .and_then(|v: String| v.parse().ok())
                .unwrap_or(10),
            date_format: map
                .remove("date_format")
                .unwrap_or_else(|| "%B %-d, %Y".into()),
        })
    }
}

/// Upsert a key-value pair in the site_settings table.
pub async fn set_site_setting(pool: &sqlx::PgPool, key: &str, value: &str) -> crate::errors::Result<()> {
    sqlx::query(
        "INSERT INTO site_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Plugin-registered routes: path → (template_name, content_type).
/// Populated during startup by the plugin loader.
pub type PluginRoutes = Arc<HashMap<String, RouteRegistration>>;

/// Cloneable application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub templates: TemplateEngine,
    pub settings: Arc<SiteSettings>,
    pub config: Arc<AppConfig>,
    /// Routes registered by plugins (e.g. "/sitemap.xml" → seo/sitemap.xml).
    pub plugin_routes: PluginRoutes,
    /// Tantivy full-text search index.
    pub search_index: Arc<SearchIndex>,
    /// Loaded plugin manifests — used by the admin plugins page.
    pub loaded_plugins: Arc<Vec<PluginManifest>>,
    /// Currently active theme name — updated live when admin switches theme.
    /// Shared across all clones so the static file handler always sees the current value.
    pub active_theme: Arc<RwLock<String>>,
}
