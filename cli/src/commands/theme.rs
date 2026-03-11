use clap::Subcommand;
use std::path::Path;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum ThemeAction {
    /// List installed themes (reads theme.toml manifests from ./themes/)
    List {
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Activate a theme for a site
    Activate {
        /// Name of the theme to activate (must match the name field in theme.toml)
        name: String,
        /// Site hostname or UUID to target (omit for single-site / global fallback)
        #[arg(long)]
        site: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
        /// Path to the server PID file (used to signal a live reload without restart)
        #[arg(long, default_value = "synaptic.pid")]
        pid_file: String,
    },
    /// Reload the active theme's templates from disk without restarting (sends SIGUSR1)
    Reload {
        /// Path to the server PID file
        #[arg(long, default_value = "synaptic.pid")]
        pid_file: String,
    },
}

pub async fn run(action: ThemeAction) -> anyhow::Result<()> {
    match action {
        ThemeAction::List { database_url } => list(database_url).await,
        ThemeAction::Activate { name, site, database_url, pid_file } => {
            activate(name, site, database_url, pid_file).await
        }
        ThemeAction::Reload { pid_file } => {
            signal_reload(&pid_file, "current");
            Ok(())
        }
    }
}

/// Collect all theme directories under themes/global/ and themes/sites/*/.
/// Returns (path, source_label) pairs. Falls back to flat themes/ for pre-multisite layouts.
fn collect_theme_dirs() -> Vec<(std::path::PathBuf, String)> {
    let themes_root = Path::new("themes");
    let mut dirs: Vec<(std::path::PathBuf, String)> = Vec::new();

    let global_dir = themes_root.join("global");
    if global_dir.is_dir() {
        dirs.push((global_dir, "global".into()));
    }

    let sites_dir = themes_root.join("sites");
    if sites_dir.is_dir() {
        for entry in std::fs::read_dir(&sites_dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                let label = format!("site:{}", p.file_name().unwrap_or_default().to_string_lossy());
                dirs.push((p, label));
            }
        }
    }

    // Fallback: flat themes/ (pre-multisite installs).
    if dirs.is_empty() && themes_root.is_dir() {
        dirs.push((themes_root.to_path_buf(), "global".into()));
    }

    dirs
}

/// Scan theme directories and return a list of (name, version, api_version, description, source).
fn scan_themes() -> anyhow::Result<Vec<(String, String, String, String, String)>> {
    let mut themes = Vec::new();
    for (dir, source) in collect_theme_dirs() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if dir_name.starts_with('.') { continue; }
            let toml_path = path.join("theme.toml");
            if !toml_path.exists() { continue; }
            let content = std::fs::read_to_string(&toml_path)
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {e}", toml_path.display()))?;
            let table: toml::Value = content.parse()
                .map_err(|e| anyhow::anyhow!("Invalid TOML in {}: {e}", toml_path.display()))?;
            let theme = table.get("theme").unwrap_or(&table);
            let name        = theme.get("name").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
            let version     = theme.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let api_version = theme.get("api_version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let description = theme.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            themes.push((name, version, api_version, description, source.clone()));
        }
    }
    themes.sort_by(|a, b| a.4.cmp(&b.4).then(a.0.cmp(&b.0)));
    Ok(themes)
}

async fn list(database_url: Option<String>) -> anyhow::Result<()> {
    let themes = scan_themes()?;

    if themes.is_empty() {
        println!("(no themes found)");
        return Ok(());
    }

    // Load per-site active themes from DB if available.
    // Map: site_id (or None for global) -> active_theme value.
    let active_map = load_active_themes(database_url).await;

    // Also load site hostnames so we can show a friendly label.
    // Map: site_id -> hostname.
    let site_names = load_site_names(&active_map).await;

    println!("\n{:<22} {:<10} {:<6} {:<10} {}", "Name", "Version", "API", "Source", "Description");
    println!("{}", "-".repeat(76));

    for (name, version, api, desc, source) in &themes {
        // Collect every site where this theme is active.
        let active_for: Vec<String> = active_map.iter().filter_map(|(site_id, t)| {
            if t == name {
                let label = match site_id {
                    Some(id) => site_names.get(id).cloned().unwrap_or_else(|| id.to_string()),
                    None => "global".to_string(),
                };
                Some(label)
            } else {
                None
            }
        }).collect();

        let marker = if active_for.is_empty() { "  " } else { " *" };
        let active_str = if active_for.is_empty() {
            String::new()
        } else {
            format!("  [active: {}]", active_for.join(", "))
        };
        println!("{}{:<20} {:<10} {:<6} {:<10} {}{}", marker, name, version, api, source, desc, active_str);
    }

    println!();
    Ok(())
}

/// Load active_theme setting for every site from site_settings.
/// Returns a map of Option<Uuid> (None = global/NULL site_id row) -> theme name.
async fn load_active_themes(database_url: Option<String>) -> std::collections::HashMap<Option<Uuid>, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(url) = database_url {
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = match super::connect_db().await {
        Ok(p) => p,
        Err(_) => return map,
    };
    let rows: Vec<(Option<Uuid>, String)> = sqlx::query_as(
        "SELECT site_id, value FROM site_settings WHERE key = 'active_theme'"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (site_id, value) in rows {
        map.insert(site_id, value);
    }
    map
}

/// Given the active_map keys, fetch hostnames for known site UUIDs.
async fn load_site_names(
    active_map: &std::collections::HashMap<Option<Uuid>, String>,
) -> std::collections::HashMap<Uuid, String> {
    let ids: Vec<Uuid> = active_map.keys().filter_map(|k| *k).collect();
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    // We need a pool — try to connect (DATABASE_URL should already be set by load_active_themes).
    let pool = match super::connect_db().await {
        Ok(p) => p,
        Err(_) => return std::collections::HashMap::new(),
    };
    let mut map = std::collections::HashMap::new();
    for id in ids {
        if let Ok(hostname) = sqlx::query_scalar::<_, String>("SELECT hostname FROM sites WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
        {
            map.insert(id, hostname);
        }
    }
    map
}

async fn activate(
    name: String,
    site: Option<String>,
    database_url: Option<String>,
    pid_file: String,
) -> anyhow::Result<()> {
    // Verify the theme exists on disk.
    let _theme_path = collect_theme_dirs()
        .into_iter()
        .find_map(|(dir, _)| {
            std::fs::read_dir(&dir).ok()?.flatten().find_map(|entry| {
                let path = entry.path();
                if !path.is_dir() { return None; }
                let toml_path = path.join("theme.toml");
                let content = std::fs::read_to_string(&toml_path).ok()?;
                let table: toml::Value = content.parse().ok()?;
                let toml_name = table.get("theme")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if toml_name == name { Some(path) } else { None }
            })
        })
        .ok_or_else(|| anyhow::anyhow!(
            "Theme '{}' not found in ./themes/global/ or ./themes/sites/*/.", name
        ))?;

    if let Some(url) = database_url {
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    let pool = super::connect_db().await?;

    // Resolve --site to a UUID (accepts hostname or raw UUID).
    let site_id: Option<Uuid> = match &site {
        None => None,
        Some(s) => {
            // Try parsing as UUID first, then fall back to hostname lookup.
            if let Ok(id) = s.parse::<Uuid>() {
                Some(id)
            } else {
                let id: Uuid = sqlx::query_scalar("SELECT id FROM sites WHERE hostname = $1")
                    .bind(s)
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("DB error looking up site: {e}"))?
                    .ok_or_else(|| anyhow::anyhow!(
                        "No site found with hostname '{}'. Run 'synap-cli site list' to see available sites.", s
                    ))?;
                Some(id)
            }
        }
    };

    match site_id {
        None => {
            // Global / single-site upsert (site_id IS NULL row).
            sqlx::query(
                "INSERT INTO site_settings (key, value) VALUES ('active_theme', $1)
                 ON CONFLICT (key) WHERE site_id IS NULL DO UPDATE SET value = EXCLUDED.value"
            )
            .bind(&name)
            .execute(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update active_theme: {e}"))?;

            println!("Theme '{}' activated (global).", name);
        }
        Some(id) => {
            // Per-site upsert using the (site_id, key) partial unique index.
            sqlx::query(
                "INSERT INTO site_settings (site_id, key, value) VALUES ($1, 'active_theme', $2)
                 ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL DO UPDATE SET value = EXCLUDED.value"
            )
            .bind(id)
            .bind(&name)
            .execute(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update active_theme for site: {e}"))?;

            let id_str = id.to_string();
            let label = site.as_deref().unwrap_or(&id_str);
            println!("Theme '{}' activated for site '{}'.", name, label);
        }
    }

    signal_reload(&pid_file, &name);
    Ok(())
}

/// Read the PID file and send SIGUSR1 to the server process.
/// Prints a status message either way — never returns an error (signal is best-effort).
fn signal_reload(pid_file: &str, theme_name: &str) {
    let pid_path = std::path::Path::new(pid_file);

    if !pid_path.exists() {
        println!("No PID file found at '{}' — start the server and it will use the new theme.", pid_file);
        return;
    }

    let contents = match std::fs::read_to_string(pid_path) {
        Ok(s) => s,
        Err(e) => {
            println!("Could not read PID file: {}. Restart the server to apply the theme.", e);
            return;
        }
    };

    let pid = match contents.trim().parse::<u32>() {
        Ok(p) => p,
        Err(_) => {
            println!("PID file contains invalid data. Restart the server to apply the theme.");
            return;
        }
    };

    let status = std::process::Command::new("kill")
        .args(["-USR1", &pid.to_string()])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Server (PID {}) signalled — theme '{}' is now live.", pid, theme_name);
        }
        _ => {
            println!("Could not signal server (PID {}). It may not be running — restart to apply the theme.", pid);
        }
    }
}
