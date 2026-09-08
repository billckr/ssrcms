//! Shared application bootstrap — builds a fully-wired `AppState` (plus the
//! two session layers `router::build` needs) from an `AppConfig`.
//!
//! Extracted from `main.rs` so integration tests can construct the exact
//! same `AppState`/`Router` a real running instance would, instead of
//! duplicating (and inevitably drifting from) that setup in test code.
//! `main.rs` still owns everything process-specific that doesn't belong in
//! a reusable library function: logging init, the PID file, the SIGUSR1
//! live-theme-reload handler, background scheduler tasks, and the actual
//! `axum::serve` call.

use std::collections::HashMap;
use std::sync::Arc;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use once_cell::sync::Lazy;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;
use tracing::info;

use crate::app_state::{AppSettings, AppState, SiteCache, SiteSettings};
use crate::config::AppConfig;
use crate::db;
use crate::plugins::loader::LoadedPlugin;
use crate::plugins::manifest::RouteRegistration;
use crate::plugins::HookRegistry;
use crate::templates::TemplateEngine;

/// The `metrics` crate's global recorder can only be installed once per
/// process. A real run only ever calls `bootstrap::build` once, but an
/// integration test binary runs many `#[tokio::test]`s in the same
/// process, each calling this — so the handle is built exactly once and
/// reused, rather than `main.rs`'s original `.expect(...)` (which would
/// panic on the second test).
static METRICS_HANDLE: Lazy<PrometheusHandle> = Lazy::new(|| {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder")
});

pub struct Bootstrapped {
    pub state: AppState,
    pub admin_session_layer: SessionManagerLayer<PostgresStore>,
    pub account_session_layer: SessionManagerLayer<PostgresStore>,
    /// The view-tracking channel's receiver — only `main.rs`'s
    /// `scheduler::spawn_view_flush` consumes this. Handed back rather than
    /// dropped so callers that do want the flush task running (a real
    /// server) can spawn it; test callers that don't care can just drop it.
    pub view_rx: tokio::sync::mpsc::UnboundedReceiver<(uuid::Uuid, String, chrono::NaiveDate)>,
}

/// Build a fully-wired `AppState` from `cfg` — same sequence `main.rs` used
/// to run inline: prepares the themes/plugins/uploads/sites directory
/// layout, connects and migrates the database, sets up the session store,
/// loads site settings and resolves the startup theme, builds the template
/// engine and loads plugins, opens the search index, and builds the
/// multi-site cache. Does NOT spawn any background scheduler tasks, write a
/// PID file, or start listening — those stay caller-specific.
pub async fn build(cfg: &AppConfig) -> anyhow::Result<Bootstrapped> {
    // ── Uploads / sites directories ──────────────────────────────────────
    std::fs::create_dir_all(&cfg.uploads_dir)?;
    std::fs::create_dir_all(&cfg.sites_dir)?;

    // ── Theme directory structure ────────────────────────────────────────
    let global_themes_dir = format!("{}/global", cfg.themes_dir);
    if !std::path::Path::new(&global_themes_dir).exists() {
        std::fs::create_dir_all(&global_themes_dir)?;
        if let Ok(entries) = std::fs::read_dir(&cfg.themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if name == "global" || name == "private" {
                    continue;
                }
                let dest = std::path::Path::new(&global_themes_dir).join(&name);
                match std::fs::rename(&path, &dest) {
                    Ok(_) => info!("migrated theme '{}' → themes/global/", name),
                    Err(e) => tracing::warn!("could not migrate theme '{}' to global: {}", name, e),
                }
            }
        }
        info!("theme directory structure initialised — themes/global/ ready");
    }

    // ── Database ──────────────────────────────────────────────────────────
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;
    info!("database connected and migrations applied");

    // ── Session store ─────────────────────────────────────────────────────
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;

    let admin_session_layer = SessionManagerLayer::new(session_store.clone())
        .with_name(crate::middleware::admin_auth::ADMIN_SESSION_COOKIE_NAME)
        .with_secure(!cfg.dev_mode)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::hours(2),
        ))
        .with_always_save(true);
    let account_session_layer = SessionManagerLayer::new(session_store)
        .with_name("session")
        .with_secure(!cfg.dev_mode)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::hours(24),
        ))
        .with_always_save(true);
    info!("session store ready");

    // ── Site settings (from DB) ──────────────────────────────────────────
    let settings = SiteSettings::load_global(&pool).await.unwrap_or_default();
    info!("site: {} — {}", settings.site_name, settings.base_url);

    // ── Determine startup theme ───────────────────────────────────────────
    let startup_sites = crate::models::site::list(&pool).await.unwrap_or_default();
    let startup_theme = if let Some(primary_site) = startup_sites.first() {
        let site_settings = SiteSettings::load(&pool, primary_site.id)
            .await
            .unwrap_or_default();
        info!(
            "startup theme resolved from site '{}' ({}): '{}'",
            primary_site.hostname, primary_site.id, site_settings.active_theme
        );
        site_settings.active_theme
    } else {
        info!(
            "no sites found — using global active_theme: '{}'",
            settings.active_theme
        );
        settings.active_theme.clone()
    };

    // ── Plugin directory structure ────────────────────────────────────────
    let global_plugins_dir = format!("{}/global", cfg.plugins_dir);
    let sites_plugins_dir = format!("{}/sites", cfg.plugins_dir);
    if !std::path::Path::new(&global_plugins_dir).exists() {
        std::fs::create_dir_all(&global_plugins_dir)?;
        if let Ok(entries) = std::fs::read_dir(&cfg.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if name == "global" || name == "sites" {
                    continue;
                }
                let dest = std::path::Path::new(&global_plugins_dir).join(&name);
                match std::fs::rename(&path, &dest) {
                    Ok(_) => info!("migrated plugin '{}' → plugins/global/", name),
                    Err(e) => {
                        tracing::warn!("could not migrate plugin '{}' to global: {}", name, e)
                    }
                }
            }
        }
        info!("plugin directory structure initialised — plugins/global/ ready");
    }
    std::fs::create_dir_all(&sites_plugins_dir)?;

    // ── Plugin & hook registry ────────────────────────────────────────────
    let hook_registry = Arc::new(HookRegistry::new());

    // ── Template engine ───────────────────────────────────────────────────
    let engine = TemplateEngine::new(
        &cfg.themes_dir,
        &cfg.sites_dir,
        &startup_theme,
        &settings.base_url,
        hook_registry.clone(),
        pool.clone(),
    )?;

    // ── Plugin loader ─────────────────────────────────────────────────────
    let (plugin_routes, loaded_plugins) =
        load_plugins_into_engine(&cfg.plugins_dir, &hook_registry, &engine);
    info!(
        "plugins loaded — {} plugin(s), {} route(s) registered",
        loaded_plugins.len(),
        plugin_routes.len()
    );

    // ── Metrics recorder (installed once per process, see METRICS_HANDLE) ──
    let metrics_handle = METRICS_HANDLE.clone();

    // ── Search index ──────────────────────────────────────────────────────
    let search_index =
        crate::search::SearchIndex::open_or_create(std::path::Path::new(&cfg.search_index_path))?;
    let search_index = Arc::new(search_index);
    {
        let idx = (*search_index).clone();
        let db = pool.clone();
        tokio::spawn(async move {
            crate::search::indexer::rebuild_index(idx, db).await;
        });
    }
    info!("search index ready at '{}'", cfg.search_index_path);

    // ── Multi-site cache ──────────────────────────────────────────────────
    let site_cache: SiteCache = {
        use std::sync::RwLock;
        let mut cache = HashMap::new();
        for site in startup_sites {
            let s = SiteSettings::load(&pool, site.id).await.unwrap_or_default();
            info!("loaded site '{}' ({})", site.hostname, site.id);
            cache.insert(site.hostname.clone(), (site, s));
        }
        Arc::new(RwLock::new(cache))
    };

    // ── Hostname symlinks for existing sites' uploads ────────────────────
    {
        let cache = site_cache.read().expect("site_cache poisoned");
        for (hostname, (site, _)) in cache.iter() {
            let tgt = std::path::Path::new(&cfg.uploads_dir).join(site.id.to_string());
            if tgt.is_dir() {
                crate::handlers::uploads::ensure_hostname_symlink(
                    &cfg.uploads_dir,
                    hostname,
                    site.id,
                );
            }
        }
    }

    // ── App-wide settings (from DB) ───────────────────────────────────────
    let app_settings = AppSettings::load(&pool, cfg.max_upload_mb as i64)
        .await
        .unwrap_or_default();
    info!(
        "app: {} | tz: {}",
        app_settings.app_name, app_settings.timezone
    );

    // ── Admin sidebar logo ────────────────────────────────────────────────
    let logo_url = crate::app_state::detect_admin_logo();
    match &logo_url {
        Some(url) => info!("branding: custom admin logo found at '{}'", url),
        None => info!("branding: no custom admin logo found, using app_name text"),
    }

    // ── Application state ─────────────────────────────────────────────────
    let active_theme = Arc::new(std::sync::RwLock::new(startup_theme));
    let cookie_key = axum_extra::extract::cookie::Key::generate();
    let (view_tx, view_rx) =
        tokio::sync::mpsc::unbounded_channel::<(uuid::Uuid, String, chrono::NaiveDate)>();
    let view_buffer: crate::app_state::ViewBuffer = view_tx;
    let state = AppState {
        db: pool.clone(),
        templates: engine,
        settings: Arc::new(settings),
        config: Arc::new(cfg.clone()),
        cookie_key,
        plugin_routes: Arc::new(plugin_routes),
        search_index: search_index.clone(),
        loaded_plugins: Arc::new(loaded_plugins),
        active_theme,
        site_cache,
        metrics_handle,
        metrics_token: cfg.metrics_token.clone(),
        app_settings: Arc::new(std::sync::RwLock::new(app_settings)),
        view_buffer,
        logo_url: Arc::new(std::sync::RwLock::new(logo_url)),
        wp_import_progress: Arc::new(std::sync::RwLock::new(HashMap::new())),
        current_version: crate::version::current_version(),
        latest_release: Arc::new(std::sync::RwLock::new(None)),
    };
    info!("version: running {}", state.current_version);

    Ok(Bootstrapped {
        state,
        admin_session_layer,
        account_session_layer,
        view_rx,
    })
}

/// Scan the plugins directory, load manifests, register hooks into the registry,
/// add templates into the engine, and return the collected plugin route table
/// and the list of successfully loaded plugins.
///
/// Scans two subdirectories:
/// - `<plugins_dir>/global/`  — agency-managed plugins available to all sites
/// - `<plugins_dir>/sites/<uuid>/` — per-site plugin copies
fn load_plugins_into_engine(
    plugins_dir: &str,
    hook_registry: &Arc<HookRegistry>,
    engine: &TemplateEngine,
) -> (HashMap<String, RouteRegistration>, Vec<LoadedPlugin>) {
    use crate::plugins::hook_registry::HookHandler;
    use crate::plugins::manifest::PluginManifest;
    use std::path::Path;

    let mut plugin_routes: HashMap<String, RouteRegistration> = HashMap::new();
    let mut loaded_plugins: Vec<LoadedPlugin> = Vec::new();
    let mut registered_plugin_names: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    let scan_plugin_dir = |dir: &std::path::PathBuf,
                           source: &str,
                           site_id: Option<uuid::Uuid>,
                           registered: &std::collections::HashSet<String>|
     -> Vec<(String, LoadedPlugin, HashMap<String, RouteRegistration>)> {
        let mut results = Vec::new();
        if !dir.exists() {
            return results;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("could not read plugins dir {:?}: {}", dir, e);
                return results;
            }
        };
        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }
            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }
            let manifest = match PluginManifest::from_file(&manifest_path) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("skipping plugin at {:?}: {}", plugin_dir, e);
                    continue;
                }
            };

            let plugin_name = manifest.plugin.name.clone();
            let is_new_plugin = !registered.contains(&plugin_name);

            if is_new_plugin {
                for ext in &["html", "xml"] {
                    let glob_pattern = format!("{}/**/*.{}", plugin_dir.display(), ext);
                    if let Ok(paths) = glob::glob(&glob_pattern) {
                        for path in paths.flatten() {
                            let rel = match path.strip_prefix(&plugin_dir) {
                                Ok(r) => r,
                                Err(_) => continue,
                            };
                            let template_name = rel.to_string_lossy().replace('\\', "/");
                            let tmpl_source = match std::fs::read_to_string(&path) {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::warn!("could not read template {:?}: {}", path, e);
                                    continue;
                                }
                            };
                            if let Err(e) = engine.add_raw_template(&template_name, &tmpl_source) {
                                tracing::warn!(
                                    "could not register template '{}': {}",
                                    template_name,
                                    e
                                );
                            }
                        }
                    }
                }

                for (hook_name, template_path) in &manifest.hooks {
                    hook_registry.register(
                        hook_name,
                        HookHandler {
                            plugin_name: plugin_name.clone(),
                            template_path: template_path.clone(),
                        },
                    );
                }
            }

            let mut routes = HashMap::new();
            if source == "global" {
                for (path, registration) in manifest.routes.clone() {
                    routes.insert(path, registration);
                }
            }

            info!(
                "loaded plugin '{}' v{} ({})",
                manifest.plugin.name, manifest.plugin.version, source
            );

            let lp = LoadedPlugin {
                manifest,
                directory: plugin_dir,
                source: source.to_string(),
                site_id,
            };
            results.push((plugin_name, lp, routes));
        }
        results
    };

    let global_dir = Path::new(plugins_dir).join("global");
    for (name, lp, routes) in scan_plugin_dir(&global_dir, "global", None, &registered_plugin_names)
    {
        registered_plugin_names.insert(name);
        plugin_routes.extend(routes);
        loaded_plugins.push(lp);
    }

    let sites_dir = Path::new(plugins_dir).join("sites");
    if let Ok(entries) = std::fs::read_dir(&sites_dir) {
        for entry in entries.flatten() {
            let site_dir = entry.path();
            if !site_dir.is_dir() {
                continue;
            }
            let site_id = site_dir
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok());
            for (_name, lp, routes) in
                scan_plugin_dir(&site_dir, "site", site_id, &registered_plugin_names)
            {
                plugin_routes.extend(routes);
                loaded_plugins.push(lp);
            }
        }
    }

    (plugin_routes, loaded_plugins)
}
