use serde::Deserialize;

/// Application configuration, loaded from environment variables and/or a config file.
/// Environment variables take precedence over file-based config.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// HTTP bind address
    #[serde(default = "default_host")]
    pub host: String,

    /// HTTP bind port
    #[serde(default = "default_port")]
    pub port: u16,

    /// PostgreSQL connection URL
    pub database_url: String,

    /// Cookie signing secret — must be set in production
    #[serde(default = "default_secret_key")]
    #[allow(dead_code)]
    pub secret_key: String,

    /// Path to the themes directory
    #[serde(default = "default_themes_dir")]
    pub themes_dir: String,

    /// Path to the plugins directory
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: String,

    /// Path to the uploads directory
    #[serde(default = "default_uploads_dir")]
    pub uploads_dir: String,

    /// Enable hot-reload of templates and plugins (dev mode)
    #[serde(default)]
    #[allow(dead_code)]
    pub dev_mode: bool,

    /// Log level filter string (e.g. "info", "debug", "synaptic_core=debug,info")
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log output format: "text" (human-readable, default) or "json" (structured, for log aggregators)
    #[serde(default = "default_log_format")]
    pub log_format: String,

    /// Path to the Tantivy search index directory
    #[serde(default = "default_search_index_path")]
    pub search_index_path: String,

    /// Path to the PID file written on startup (used by synaptic-cli for live reload)
    #[serde(default = "default_pid_file")]
    pub pid_file: String,

    /// Optional bearer token to protect the /metrics endpoint.
    /// If unset, the endpoint is open (restrict access at the network/Caddy level instead).
    pub metrics_token: Option<String>,

    // ── Agency contact ───────────────────────────────────────────────────────
    // Used as the reply-to / notification address for system emails.
    // Set via ADMIN_EMAIL in .env or synaptic.toml.

    /// Administrator contact email (e.g. admin@acme.com)
    pub admin_email: Option<String>,

    // ── Outbound mail (SMTP) ──────────────────────────────────────────────────
    // All mail config lives here, not in the database. Set via .env or synaptic.toml.
    // If smtp_host is not set, outbound mail is disabled and operations that
    // require email (password reset, form notifications) will log a warning.

    /// SMTP server hostname (e.g. smtp.mailgun.org)
    pub smtp_host: Option<String>,

    /// SMTP server port (default: 587 for STARTTLS)
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,

    /// SMTP username / API key
    pub smtp_username: Option<String>,

    /// SMTP password / API secret
    pub smtp_password: Option<String>,

    /// Display name used in the From header (e.g. "Acme Agency")
    pub smtp_from_name: Option<String>,

    /// From email address (e.g. noreply@acme.com)
    pub smtp_from_email: Option<String>,

    /// Encryption mode: "starttls" (default), "tls", or "none"
    #[serde(default = "default_smtp_encryption")]
    pub smtp_encryption: String,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_secret_key() -> String {
    // WARN: Only used for development. Production installs MUST set SECRET_KEY.
    "change-me-in-production-this-is-not-secure".to_string()
}

fn default_themes_dir() -> String {
    "themes".to_string()
}

fn default_plugins_dir() -> String {
    "plugins".to_string()
}

fn default_uploads_dir() -> String {
    "uploads".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_search_index_path() -> String {
    "search-index".to_string()
}

fn default_pid_file() -> String {
    "synaptic.pid".to_string()
}

fn default_smtp_port() -> u16 { 587 }
fn default_smtp_encryption() -> String { "starttls".to_string() }

impl AppConfig {
    /// Load configuration from an optional TOML file and environment variables.
    ///
    /// Layer order (later sources win):
    ///   1. Serde field defaults
    ///   2. `synaptic.toml` in the working directory (or the path in `CONFIG_FILE`)
    ///   3. Environment variables (and `.env` file)
    ///
    /// The TOML file is optional — if it does not exist, only env vars are used.
    /// Set `CONFIG_FILE=/path/to/config.toml` to use a file at a custom path.
    pub fn load() -> anyhow::Result<Self> {
        // Load .env if present (silently ignore if absent)
        let _ = dotenvy::dotenv();

        // Resolve config file path: CONFIG_FILE env var, else default "synaptic.toml"
        let config_file = std::env::var("CONFIG_FILE")
            .unwrap_or_else(|_| "synaptic.toml".to_string());

        let cfg = config::Config::builder()
            // TOML file layer — optional, missing file is not an error
            .add_source(
                config::File::from(std::path::Path::new(&config_file))
                    .required(false),
            )
            // Env var layer — overrides anything in the file
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .ignore_empty(true),
            )
            .build()?;

        let app: AppConfig = cfg.try_deserialize()?;
        Ok(app)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_host() {
        assert_eq!(default_host(), "0.0.0.0");
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 3000u16);
    }

    #[test]
    fn test_default_secret_key_nonempty() {
        assert!(!default_secret_key().is_empty());
    }

    #[test]
    fn test_default_themes_dir() {
        assert_eq!(default_themes_dir(), "themes");
    }

    #[test]
    fn test_default_plugins_dir() {
        assert_eq!(default_plugins_dir(), "plugins");
    }

    #[test]
    fn test_default_uploads_dir() {
        assert_eq!(default_uploads_dir(), "uploads");
    }

    #[test]
    fn test_default_log_level() {
        assert_eq!(default_log_level(), "info");
    }

    #[test]
    fn test_default_log_format() {
        assert_eq!(default_log_format(), "text");
    }

    #[test]
    fn test_default_search_index_path() {
        assert_eq!(default_search_index_path(), "search-index");
    }

    fn make_config(host: &str, port: u16) -> AppConfig {
        AppConfig {
            host: host.to_string(),
            port,
            database_url: "postgres://localhost/test".to_string(),
            secret_key: default_secret_key(),
            themes_dir: default_themes_dir(),
            plugins_dir: default_plugins_dir(),
            uploads_dir: default_uploads_dir(),
            dev_mode: false,
            log_level: default_log_level(),
            log_format: default_log_format(),
            search_index_path: default_search_index_path(),
            pid_file: default_pid_file(),
            metrics_token: None,
            admin_email: None,
            smtp_host: None,
            smtp_port: default_smtp_port(),
            smtp_username: None,
            smtp_password: None,
            smtp_from_name: None,
            smtp_from_email: None,
            smtp_encryption: default_smtp_encryption(),
        }
    }

    #[test]
    fn bind_addr_default_values() {
        let cfg = make_config("0.0.0.0", 3000);
        assert_eq!(cfg.bind_addr(), "0.0.0.0:3000");
    }

    #[test]
    fn bind_addr_custom_host_and_port() {
        let cfg = make_config("127.0.0.1", 8080);
        assert_eq!(cfg.bind_addr(), "127.0.0.1:8080");
    }
}
