use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use synaptic_core::app_state::{AppState, SiteSettings};
use synaptic_core::config::AppConfig;
use synaptic_core::db;
use synaptic_core::plugins::manifest::{PluginManifest, RouteRegistration};
use synaptic_core::plugins::HookRegistry;
use synaptic_core::router;
use synaptic_core::search;
use synaptic_core::templates::TemplateEngine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Config ────────────────────────────────────────────────────────────────
    let cfg = AppConfig::load().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    // ── Logging ───────────────────────────────────────────────────────────────
    let filter = EnvFilter::new(&cfg.log_level);
    let registry = tracing_subscriber::registry().with(filter);
    match cfg.log_format.as_str() {
        "json" => registry.with(tracing_subscriber::fmt::layer().json()).init(),
        _ => registry.with(tracing_subscriber::fmt::layer()).init(),
    }

    info!("Synaptic Signals CMS starting...");

    // ── Uploads directory ─────────────────────────────────────────────────────
    std::fs::create_dir_all(&cfg.uploads_dir)?;

    // ── Database ──────────────────────────────────────────────────────────────
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;
    info!("database connected and migrations applied");

    // ── Session store ─────────────────────────────────────────────────────────
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_sqlx_store::PostgresStore;

    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store);
    info!("session store ready");

    // ── Site settings (from DB) ───────────────────────────────────────────────
    let settings = SiteSettings::load(&pool).await?;
    info!("site: {} — {}", settings.site_name, settings.base_url);

    // ── Plugin & hook registry ────────────────────────────────────────────────
    let hook_registry = Arc::new(HookRegistry::new());

    // ── Template engine ───────────────────────────────────────────────────────
    let engine = TemplateEngine::new(
        &cfg.themes_dir,
        &settings.active_theme,
        &settings.base_url,
        hook_registry.clone(),
        pool.clone(),
    )?;

    // ── Plugin loader ──────────────────────────────────────────────────────────
    let (plugin_routes, loaded_plugins) =
        load_plugins_into_engine(&cfg.plugins_dir, &hook_registry, &engine);

    info!(
        "plugins loaded — {} plugin(s), {} route(s) registered",
        loaded_plugins.len(),
        plugin_routes.len()
    );

    // ── Search index ──────────────────────────────────────────────────────────
    let search_index = search::SearchIndex::open_or_create(
        std::path::Path::new(&cfg.search_index_path),
    )?;
    let search_index = Arc::new(search_index);

    // Rebuild index in the background on startup (non-blocking).
    {
        let idx = (*search_index).clone();
        let db = pool.clone();
        tokio::spawn(async move {
            search::indexer::rebuild_index(idx, db).await;
        });
    }
    info!("search index ready at '{}'", cfg.search_index_path);

    // ── Application state ─────────────────────────────────────────────────────
    let active_theme = Arc::new(std::sync::RwLock::new(settings.active_theme.clone()));
    let state = AppState {
        db: pool.clone(),
        templates: engine,
        settings: Arc::new(settings),
        config: Arc::new(cfg.clone()),
        plugin_routes: Arc::new(plugin_routes),
        search_index,
        loaded_plugins: Arc::new(loaded_plugins),
        active_theme,
    };

    // ── Router ────────────────────────────────────────────────────────────────
    let app = router::build(state.clone(), &cfg.uploads_dir, session_layer);

    // ── PID file ──────────────────────────────────────────────────────────────
    let pid = std::process::id();
    if let Err(e) = std::fs::write(&cfg.pid_file, pid.to_string()) {
        tracing::warn!("could not write PID file '{}': {}", cfg.pid_file, e);
    } else {
        info!("PID {} written to '{}'", pid, cfg.pid_file);
    }

    // ── SIGUSR1 handler — live theme reload ───────────────────────────────────
    {
        use tokio::signal::unix::{signal, SignalKind};

        let templates   = state.templates.clone();
        let active_theme = state.active_theme.clone();
        let db          = pool.clone();

        tokio::spawn(async move {
            let mut stream = match signal(SignalKind::user_defined1()) {
                Ok(s)  => s,
                Err(e) => { tracing::error!("failed to register SIGUSR1 handler: {}", e); return; }
            };
            loop {
                stream.recv().await;
                tracing::info!("SIGUSR1 received — reloading active theme");

                let theme_name: String = sqlx::query_scalar(
                    "SELECT value FROM site_settings WHERE key = 'active_theme'"
                )
                .fetch_optional(&db)
                .await
                .unwrap_or(None)
                .unwrap_or_else(|| "default".to_string());

                match templates.switch_theme(&theme_name) {
                    Ok(_) => {
                        *active_theme.write().unwrap() = theme_name.clone();
                        tracing::info!("theme '{}' reloaded via SIGUSR1", theme_name);
                    }
                    Err(e) => tracing::error!("SIGUSR1 theme reload failed: {}", e),
                }
            }
        });
    }

    // ── Server ────────────────────────────────────────────────────────────────
    let addr = cfg.bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("listening on http://{}", addr);

    axum::serve(listener, app).await?;

    // ── Cleanup ───────────────────────────────────────────────────────────────
    let _ = std::fs::remove_file(&cfg.pid_file);
    Ok(())
}

/// Scan the plugins directory, load manifests, register hooks into the registry,
/// add templates into the engine, and return the collected plugin route table
/// and the list of successfully loaded manifests.
fn load_plugins_into_engine(
    plugins_dir: &str,
    hook_registry: &Arc<HookRegistry>,
    engine: &TemplateEngine,
) -> (HashMap<String, RouteRegistration>, Vec<synaptic_core::plugins::manifest::PluginManifest>) {
    use synaptic_core::plugins::hook_registry::HookHandler;
    use std::path::Path;

    let mut plugin_routes: HashMap<String, RouteRegistration> = HashMap::new();
    let mut loaded_manifests = Vec::new();
    let dir = Path::new(plugins_dir);

    if !dir.exists() {
        return (plugin_routes, loaded_manifests);
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("could not read plugins directory: {}", e);
            return (plugin_routes, loaded_manifests);
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

        // Add all HTML and XML templates from the plugin directory.
        let glob_pattern = format!("{}/**/*.{{html,xml}}", plugin_dir.display());
        let paths = match glob::glob(&glob_pattern) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("glob error for plugin {:?}: {}", plugin_dir, e);
                continue;
            }
        };

        for path in paths.flatten() {
            let rel = match path.strip_prefix(&plugin_dir) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let template_name = rel.to_string_lossy().replace('\\', "/");
            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("could not read template {:?}: {}", path, e);
                    continue;
                }
            };
            if let Err(e) = engine.add_raw_template(&template_name, &source) {
                tracing::warn!("could not register template '{}': {}", template_name, e);
            }
        }

        // Register hooks into the shared registry.
        for (hook_name, template_path) in &manifest.hooks {
            hook_registry.register(
                hook_name,
                HookHandler {
                    plugin_name: manifest.plugin.name.clone(),
                    template_path: template_path.clone(),
                },
            );
        }

        // Collect plugin-registered routes.
        for (path, registration) in manifest.routes.clone() {
            plugin_routes.insert(path, registration);
        }

        info!(
            "loaded plugin '{}' v{}",
            manifest.plugin.name, manifest.plugin.version
        );

        loaded_manifests.push(manifest);
    }

    (plugin_routes, loaded_manifests)
}
