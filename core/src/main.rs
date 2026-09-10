use std::{fs::OpenOptions, path::Path, sync::Mutex};
use tracing::info;

use tracing_subscriber::{
    filter::filter_fn, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

use synaptic_core::config::AppConfig;
use synaptic_core::router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Config ────────────────────────────────────────────────────────────────
    let cfg = AppConfig::load().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    // ── Logging ───────────────────────────────────────────────────────────────
    init_logging(&cfg)?;
    tracing::info!(
        target: "ai_translation",
        event = "audit_log_ready",
        path = %cfg.ai_log_path,
        "AI translation audit log ready"
    );

    if std::env::args().any(|arg| arg == "--emit-ai-log-test-failure") {
        emit_ai_log_test_failure();
        return Ok(());
    }

    info!(ai_log_path = %cfg.ai_log_path, "SynapCMS starting...");

    // ── Bootstrap: config-derived dirs, DB, session store, theme/plugin
    //    loading, search index, AppState — see bootstrap.rs for the full
    //    sequence (shared with the integration test harness). ─────────────
    let synaptic_core::bootstrap::Bootstrapped {
        state,
        admin_session_layer,
        account_session_layer,
        view_rx,
    } = synaptic_core::bootstrap::build(&cfg).await?;

    let pool = state.db.clone();
    let search_index = state.search_index.clone();

    // ── Scheduled post publisher ─────────────────────────────────────────────
    synaptic_core::scheduler::spawn_scheduled_publisher(pool.clone(), search_index.clone());
    info!("scheduler: scheduled post publisher started (60 s interval)");

    // ── View flush task ───────────────────────────────────────────────────────
    synaptic_core::scheduler::spawn_view_flush(pool.clone(), view_rx);
    info!("scheduler: view flush task started (60 s interval)");

    // ── Release check task ───────────────────────────────────────────────────
    if cfg.update_check_enabled {
        synaptic_core::scheduler::spawn_release_check(state.latest_release.clone());
        info!("scheduler: release check task started (6 h interval)");
    } else {
        info!("scheduler: release check task disabled (UPDATE_CHECK_ENABLED=false)");
    }

    // ── Router ────────────────────────────────────────────────────────────────
    let app = router::build(state.clone(), admin_session_layer, account_session_layer);

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

        let templates = state.templates.clone();
        let active_theme = state.active_theme.clone();
        let site_cache = state.site_cache.clone();
        let db = pool.clone();

        tokio::spawn(async move {
            let mut stream = match signal(SignalKind::user_defined1()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("failed to register SIGUSR1 handler: {}", e);
                    return;
                }
            };
            loop {
                stream.recv().await;
                tracing::info!("SIGUSR1 received — reloading active theme");

                // Use the same resolution as startup: prefer the per-site row
                // for the primary site, fall back to the global/NULL row.
                let theme_name: String = sqlx::query_scalar(
                    "SELECT ss.value
                     FROM site_settings ss
                     JOIN sites s ON s.id = ss.site_id
                     WHERE ss.key = 'active_theme'
                     ORDER BY s.created_at
                     LIMIT 1",
                )
                .fetch_optional(&db)
                .await
                .unwrap_or(None)
                .or(sqlx::query_scalar(
                    "SELECT value FROM site_settings
                         WHERE key = 'active_theme' AND site_id IS NULL",
                )
                .fetch_optional(&db)
                .await
                .unwrap_or(None))
                .unwrap_or_else(|| "default".to_string());

                // Also reload all per-site active_theme values from DB into
                // the site cache — this is what per-request rendering reads.
                let site_rows: Vec<(uuid::Uuid, String)> = sqlx::query_as(
                    "SELECT site_id, value FROM site_settings
                     WHERE key = 'active_theme' AND site_id IS NOT NULL",
                )
                .fetch_all(&db)
                .await
                .unwrap_or_default();

                if let Ok(mut cache) = site_cache.write() {
                    for val in cache.values_mut() {
                        if let Some((_, new_theme)) =
                            site_rows.iter().find(|(id, _)| *id == val.0.id)
                        {
                            val.1.active_theme = new_theme.clone();
                        }
                    }
                }

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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    // ── Cleanup ───────────────────────────────────────────────────────────────
    let _ = std::fs::remove_file(&cfg.pid_file);
    Ok(())
}

/// Emit a paired, unmistakably synthetic attempt/failure record so operators
/// can verify ingestion and alerting without changing provider configuration,
/// sending content, spending tokens, or writing to the database.
fn emit_ai_log_test_failure() {
    let attempt_id = uuid::Uuid::new_v4();
    let nil_id = uuid::Uuid::nil();
    tracing::info!(
        target: "ai_translation",
        event = "translation_attempt_started",
        %attempt_id,
        site_id = %nil_id,
        post_id = %nil_id,
        post_type = "diagnostic",
        locale = "test",
        provider_id = %nil_id,
        provider_type = "diagnostic",
        model = "diagnostic-model",
        admin_user_id = %nil_id,
        source_chars = 0,
        synthetic = true,
        "Synthetic AI translation attempt started"
    );
    tracing::error!(
        target: "ai_translation",
        event = "translation_attempt_finished",
        outcome = "failure",
        stage = "provider_or_response",
        %attempt_id,
        site_id = %nil_id,
        post_id = %nil_id,
        locale = "test",
        provider_id = %nil_id,
        provider_type = "diagnostic",
        model = "diagnostic-model",
        duration_ms = 0,
        error = "synthetic diagnostic: provider request failed (429 Too Many Requests): rate_limit_error",
        synthetic = true,
        "Synthetic AI translation attempt failed"
    );
}

/// Keep AI operations in a stable JSON-lines file even when the main log is
/// human-readable. The `ai_translation` target is intentionally excluded from
/// the general layer so content-bearing provider diagnostics are not copied to
/// the broad application log.
fn init_logging(cfg: &AppConfig) -> anyhow::Result<()> {
    let ai_log_path = Path::new(&cfg.ai_log_path);
    if let Some(parent) = ai_log_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let ai_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ai_log_path)?;

    match cfg.log_format.as_str() {
        "json" => {
            let ai_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(Mutex::new(ai_log))
                .with_filter(filter_fn(|metadata| metadata.target() == "ai_translation"));
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_filter(EnvFilter::new(&cfg.log_level))
                        .with_filter(filter_fn(|metadata| metadata.target() != "ai_translation")),
                )
                .with(ai_layer)
                .init()
        }
        _ => {
            let ai_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(Mutex::new(ai_log))
                .with_filter(filter_fn(|metadata| metadata.target() == "ai_translation"));
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_filter(EnvFilter::new(&cfg.log_level))
                        .with_filter(filter_fn(|metadata| metadata.target() != "ai_translation")),
                )
                .with(ai_layer)
                .init()
        }
    }
    Ok(())
}
