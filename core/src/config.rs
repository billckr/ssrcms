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

    /// Path to the Tantivy search index directory
    #[serde(default = "default_search_index_path")]
    pub search_index_path: String,

    /// Path to the PID file written on startup (used by synaptic-cli for live reload)
    #[serde(default = "default_pid_file")]
    pub pid_file: String,
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

fn default_log_level() -> String {
    "info".to_string()
}

fn default_search_index_path() -> String {
    "search-index".to_string()
}

fn default_pid_file() -> String {
    "synaptic.pid".to_string()
}

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
            search_index_path: default_search_index_path(),
            pid_file: default_pid_file(),
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
