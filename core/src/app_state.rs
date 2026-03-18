//! Shared application state passed to every Axum handler via State extractor.

use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uuid::Uuid;

use axum_extra::extract::cookie::Key;

use crate::config::AppConfig;
use crate::models::site::Site;
use crate::plugins::loader::LoadedPlugin;
use crate::plugins::manifest::RouteRegistration;
use crate::search::SearchIndex;
use crate::templates::TemplateEngine;

/// Runtime site settings loaded from the site_settings table.
/// Cached on startup; reloaded on config change.
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

impl Default for SiteSettings {
    fn default() -> Self {
        SiteSettings {
            site_name: "Synaptic Signals".to_string(),
            site_description: "Fast by default, secure by design".to_string(),
            base_url: "http://localhost:3000".to_string(),
            language: "en-US".to_string(),
            active_theme: "default".to_string(),
            posts_per_page: 9,
            date_format: "%B %-d, %Y".to_string(),
        }
    }
}

impl SiteSettings {
    /// Load settings for a specific site from the database.
    pub async fn load(pool: &PgPool, site_id: Uuid) -> anyhow::Result<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT key, value FROM site_settings WHERE site_id = $1",
        )
        .bind(site_id)
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

    /// Load settings without a site_id filter — used at startup before sites are configured.
    /// Falls back gracefully to defaults when no rows exist.
    pub async fn load_global(pool: &PgPool) -> anyhow::Result<Self> {
        // After migration 0010, legacy rows may have site_id IS NULL.
        // Before migration 0010, there is no site_id column at all.
        // Either way, fetch all rows and use the first batch found.
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT key, value FROM site_settings WHERE site_id IS NULL",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

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

/// App-wide runtime settings loaded from the app_settings table.
/// Cached in AppState behind an Arc<RwLock<>>; updated without restart when
/// saved via /admin/settings.
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub app_name: String,
    pub timezone: String,
    pub max_upload_mb: i64,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            app_name: "Synaptic".to_string(),
            timezone: "UTC".to_string(),
            max_upload_mb: 25,
        }
    }
}

impl AppSettings {
    pub async fn load(pool: &PgPool) -> anyhow::Result<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT key, value FROM app_settings",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut map: HashMap<String, String> = rows.into_iter().collect();
        Ok(AppSettings {
            app_name: map.remove("app_name").unwrap_or_else(|| "Synaptic".into()),
            timezone: map.remove("timezone").unwrap_or_else(|| "UTC".into()),
            max_upload_mb: map
                .remove("max_upload_mb")
                .and_then(|v| v.parse().ok())
                .unwrap_or(25),
        })
    }
}

/// Upsert a key-value pair in the app_settings table.
pub async fn set_app_setting(
    pool: &PgPool,
    key: &str,
    value: &str,
) -> crate::errors::Result<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert a key-value pair in the site_settings table for a specific site.
/// Uses the partial unique index site_settings_site_key_idx for conflict resolution.
pub async fn set_site_setting(
    pool: &PgPool,
    site_id: Uuid,
    key: &str,
    value: &str,
) -> crate::errors::Result<()> {
    sqlx::query(
        "INSERT INTO site_settings (site_id, key, value) VALUES ($1, $2, $3)
         ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL
         DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(site_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Hostname-keyed site cache: hostname → (Site, SiteSettings).
pub type SiteCache = Arc<RwLock<HashMap<String, (Site, SiteSettings)>>>;

/// Plugin-registered routes: path → (template_name, content_type).
pub type PluginRoutes = Arc<HashMap<String, RouteRegistration>>;

/// Sender half of the view-tracking channel.
///
/// Each request handler fires a single `.send()` — a non-blocking, lock-free
/// operation — and moves on immediately.  The receiver lives exclusively inside
/// the background flush task (`scheduler::spawn_view_flush`), which drains it
/// every 60 s, deduplicates in a local HashSet, and batch-inserts into
/// `post_views`.
///
/// Why a channel instead of `Arc<Mutex<HashSet>>`:
///   • `std::sync::Mutex::lock()` in an async context blocks the OS thread, not
///     just the async task — under high concurrency this starves Tokio's thread
///     pool and degrades ALL request handling site-wide, not only view counting.
///   • `mpsc::UnboundedSender::send()` uses a lock-free internal queue; thousands
///     of concurrent senders never contend with each other.
///
/// Tuple payload: (post_id, anonymized_ip_hash, viewed_date).
pub type ViewBuffer = mpsc::UnboundedSender<(Uuid, String, chrono::NaiveDate)>;

/// Cloneable application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub templates: TemplateEngine,
    /// Default/fallback site settings (used when site_cache is empty or pre-migration).
    pub settings: Arc<SiteSettings>,
    pub config: Arc<AppConfig>,
    /// HMAC signing key for post-unlock browser-session cookies.
    pub cookie_key: Key,
    /// Routes registered by plugins (e.g. "/sitemap.xml" → seo/sitemap.xml).
    pub plugin_routes: PluginRoutes,
    /// Tantivy full-text search index.
    pub search_index: Arc<SearchIndex>,
    /// Loaded plugins (manifest + source metadata) — used by the admin plugins page.
    pub loaded_plugins: Arc<Vec<LoadedPlugin>>,
    /// Currently active theme name — updated live when admin switches theme.
    pub active_theme: Arc<RwLock<String>>,
    /// Multi-site cache: hostname → (Site, SiteSettings).
    pub site_cache: SiteCache,
    /// Handle for rendering Prometheus metrics text at GET /metrics.
    pub metrics_handle: PrometheusHandle,
    /// Optional bearer token required to access GET /metrics.
    pub metrics_token: Option<String>,
    /// App-wide settings (app_name, timezone, max_upload_mb) — hot-reloadable.
    pub app_settings: Arc<RwLock<AppSettings>>,
    /// Channel sender for view tracking — see `ViewBuffer` type alias for full rationale.
    pub view_buffer: ViewBuffer,
}

impl axum::extract::FromRef<AppState> for axum_extra::extract::cookie::Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

impl AppState {
    /// Resolve a site and its settings by hostname.
    pub fn resolve_site(&self, hostname: &str) -> Option<(Site, SiteSettings)> {
        self.site_cache.read().ok()?.get(hostname).cloned()
    }

    /// Return the active theme for a given site, falling back to the global
    /// active_theme when the site is not found in the cache.
    pub fn active_theme_for_site(&self, site_id: Option<Uuid>) -> String {
        site_id
            .and_then(|id| self.get_site_by_id(id))
            .map(|(_, s)| s.active_theme)
            .unwrap_or_else(|| self.active_theme.read().unwrap().clone())
    }

    /// Return the hostname for a site_id — used to populate the header site indicator.
    pub fn site_hostname(&self, site_id: Option<Uuid>) -> String {
        site_id
            .and_then(|sid| self.get_site_by_id(sid))
            .map(|(s, _)| s.hostname)
            .unwrap_or_default()
    }

    /// Resolve a site by UUID (iterates the cache; used by admin handlers).
    pub fn get_site_by_id(&self, site_id: Uuid) -> Option<(Site, SiteSettings)> {
        self.site_cache
            .read()
            .ok()?
            .values()
            .find(|(s, _)| s.id == site_id)
            .cloned()
    }

    /// Update the active_theme for a specific site in the in-memory cache.
    /// Called after a successful theme switch so the static file handler
    /// immediately serves assets from the new theme without a restart.
    pub fn update_site_theme_in_cache(&self, site_id: Uuid, theme: &str) {
        if let Ok(mut cache) = self.site_cache.write() {
            for val in cache.values_mut() {
                if val.0.id == site_id {
                    val.1.active_theme = theme.to_string();
                    break;
                }
            }
        }
    }

    /// Reload app_settings from the database into the in-memory cache.
    /// Called after saving /admin/settings so changes take effect immediately.
    pub async fn reload_app_settings(&self) -> anyhow::Result<()> {
        let fresh = AppSettings::load(&self.db).await?;
        if let Ok(mut w) = self.app_settings.write() {
            *w = fresh;
        }
        Ok(())
    }

    /// Reload the site cache from the database.
    pub async fn reload_site_cache(&self) -> anyhow::Result<()> {
        let sites = crate::models::site::list(&self.db).await?;
        let mut cache = HashMap::new();
        for site in sites {
            let settings = SiteSettings::load(&self.db, site.id)
                .await
                .unwrap_or_default();
            cache.insert(site.hostname.clone(), (site, settings));
        }
        if let Ok(mut w) = self.site_cache.write() {
            *w = cache;
        }
        Ok(())
    }
}
