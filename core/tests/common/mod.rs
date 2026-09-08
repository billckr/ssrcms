//! Shared test-support helpers for HTTP-level integration tests
//! (`tests/routes.rs`). Lives under `tests/common/` rather than directly in
//! `tests/` so Cargo doesn't treat it as its own (empty) test binary — see
//! https://doc.rust-lang.org/book/ch11-03-test-organization.html#submodules-in-integration-tests.

use std::path::PathBuf;
use synaptic_core::config::AppConfig;

/// Resolve the workspace root's real `themes/` directory. Integration
/// tests run with the crate's manifest directory (`core/`) as their
/// working directory, not the workspace root, so a bare relative
/// `"themes"` wouldn't resolve — and a fabricated fake theme isn't worth
/// building when the real one is right there and only ever read, never
/// written, by these tests.
fn workspace_themes_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../themes")
        .canonicalize()
        .expect("workspace themes/ directory not found — expected core/../themes to exist")
        .to_string_lossy()
        .to_string()
}

/// Best-effort cleanup of temp dirs left behind by previous test runs
/// (nothing here removes its own dir mid-run — the router/search-index
/// hold live handles into it for the test's lifetime). Only removes dirs
/// older than an hour, so it never races a test that's still running.
fn sweep_stale_tmp_dirs() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("synapcms-test-") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Build a throwaway `AppConfig` for one test run. Everything that gets
/// written to (uploads/sites/plugins/documentation/search-index) points at
/// a fresh temp directory unique to this call, so parallel `#[tokio::test]`
/// runs in the same process never collide with each other.
fn test_config() -> AppConfig {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    sweep_stale_tmp_dirs();
    let tmp = std::env::temp_dir().join(format!("synapcms-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("failed to create test tmp dir");

    AppConfig {
        host: "127.0.0.1".to_string(),
        port: 0, // unused — tests call the router in-process, never bind a socket
        database_url,
        secret_key: "test-secret-key-not-for-production-use-only".to_string(),
        themes_dir: workspace_themes_dir(),
        plugins_dir: tmp.join("plugins").to_string_lossy().to_string(),
        documentation_dir: tmp.join("documentation").to_string_lossy().to_string(),
        uploads_dir: tmp.join("uploads").to_string_lossy().to_string(),
        sites_dir: tmp.join("sites").to_string_lossy().to_string(),
        dev_mode: true,
        log_level: "error".to_string(),
        log_format: "text".to_string(),
        search_index_path: tmp.join("search-index").to_string_lossy().to_string(),
        pid_file: tmp.join("test.pid").to_string_lossy().to_string(),
        caddyfile_path: tmp.join("Caddyfile").to_string_lossy().to_string(),
        metrics_token: None,
        max_upload_mb: 25,
        admin_email: None,
        smtp_host: None,
        smtp_port: 587,
        smtp_username: None,
        smtp_password: None,
        smtp_from_name: None,
        smtp_from_email: None,
        smtp_encryption: "starttls".to_string(),
        mailgun_api_key: None,
        mailgun_domain: None,
        mailgun_base_url: "https://api.mailgun.net/v3".to_string(),
        update_check_enabled: false,
        self_update_enabled: false,
    }
}

/// A `ConnectInfo<SocketAddr>` extension for test requests. A real process
/// only ever gets this via `into_make_service_with_connect_info(...)`
/// (`main.rs`), which is bypassed entirely when driving the router directly
/// through `tower::ServiceExt::oneshot` — several global middleware layers
/// (e.g. `middleware::ip_denylist`) extract `ConnectInfo` unconditionally,
/// so every test request needs this inserted or those layers 500 before
/// reaching the actual handler.
pub fn connect_info() -> axum::extract::ConnectInfo<std::net::SocketAddr> {
    axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}

/// `middleware::site::CurrentSite` resolves every request's site from its
/// `Host` header (defaulting to "localhost" when absent) and returns a hard
/// 404 if no site matches — there's no empty-cache fallback despite that
/// module's own doc comment claiming one (stale; not this crate's problem
/// to fix). A real install always has at least one site via `synap
/// install`; a freshly-migrated test database has none, so every
/// site-dependent route (home, search, ...) would 404 in every test unless
/// one is seeded first. Idempotent — safe to call before every
/// `test_router()`, including from multiple `#[tokio::test]`s sharing one
/// database.
async fn ensure_test_site(database_url: &str) {
    let pool = synaptic_core::db::connect(database_url)
        .await
        .expect("failed to connect for test site setup");
    synaptic_core::db::migrate(&pool)
        .await
        .expect("failed to migrate for test site setup");

    if synaptic_core::models::site::get_by_hostname(&pool, "localhost")
        .await
        .is_ok()
    {
        return;
    }

    // routes.rs's tests run concurrently (no --test-threads=1), so several
    // can reach this point before any of them has inserted — the
    // check-then-create above isn't atomic. Rather than serialize the whole
    // suite over it, treat "someone else's concurrent call just created it"
    // as success: only a real failure (not a duplicate-hostname conflict)
    // should panic.
    match synaptic_core::models::site::create_with_defaults(&pool, "localhost", None, None).await
    {
        Ok(_) => {}
        Err(synaptic_core::errors::AppError::Database(sqlx::Error::Database(db_err)))
            if db_err.is_unique_violation() => {}
        Err(e) => panic!("failed to seed test site: {e}"),
    }
}

/// Build a full `Router` exactly as a live process would — same
/// `bootstrap::build` + `router::build` sequence `main.rs` uses — so route
/// tests exercise real routing, middleware, and session wiring, not a
/// hand-rolled approximation of it.
pub async fn test_router() -> axum::Router {
    let cfg = test_config();
    ensure_test_site(&cfg.database_url).await;
    let bootstrapped = synaptic_core::bootstrap::build(&cfg)
        .await
        .expect("bootstrap failed — is a Postgres instance reachable at $DATABASE_URL?");
    synaptic_core::router::build(
        bootstrapped.state,
        bootstrapped.admin_session_layer,
        bootstrapped.account_session_layer,
    )
}
