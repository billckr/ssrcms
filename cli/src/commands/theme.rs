use clap::Subcommand;
use std::path::Path;

#[derive(Subcommand)]
pub enum ThemeAction {
    /// List installed themes (reads theme.toml manifests from ./themes/)
    List,
    /// Activate a theme by name (updates site_settings in the database and signals a live reload)
    Activate {
        /// Name of the theme to activate (must match the name field in theme.toml)
        name: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        database_url: Option<String>,
        /// Path to the server PID file (used to signal a live reload without restart)
        #[arg(long, default_value = "synaptic.pid")]
        pid_file: String,
    },
}

pub async fn run(action: ThemeAction) -> anyhow::Result<()> {
    match action {
        ThemeAction::List => list(),
        ThemeAction::Activate { name, database_url, pid_file } => activate(name, database_url, pid_file).await,
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

fn list() -> anyhow::Result<()> {
    let dirs = collect_theme_dirs();
    if dirs.is_empty() {
        println!("No themes directory found (expected: ./themes/global/)");
        return Ok(());
    }

    let active_hint = std::env::var("ACTIVE_THEME").unwrap_or_default();
    let mut themes: Vec<(String, String, String, String, bool, String)> = Vec::new();

    for (dir, source) in &dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
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
            let active = name == active_hint;
            themes.push((name, version, api_version, description, active, source.clone()));
        }
    }

    themes.sort_by(|a, b| a.5.cmp(&b.5).then(a.0.cmp(&b.0)));

    if themes.is_empty() {
        println!("(no themes found)");
        return Ok(());
    }

    println!("\n{:<22} {:<10} {:<6} {:<10} {}", "Name", "Version", "API", "Source", "Description");
    println!("{}", "-".repeat(76));

    for (name, version, api, desc, active, source) in &themes {
        let marker = if *active { " *" } else { "  " };
        println!("{}{:<20} {:<10} {:<6} {:<10} {}", marker, name, version, api, source, desc);
    }

    println!();
    Ok(())
}

async fn activate(name: String, database_url: Option<String>, pid_file: String) -> anyhow::Result<()> {
    // Search themes/global/ and themes/sites/*/ for a theme whose toml name matches.
    let _theme_path = collect_theme_dirs()
        .into_iter()
        .find_map(|(dir, _)| {
            // Check every subdirectory in this parent for a matching theme name.
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
            "Theme '{}' not found in ./themes/global/ or ./themes/sites/*/. Check the name and try again.", name
        ))?;

    if let Some(url) = database_url {
        // Safety: single-threaded CLI, no other threads reading the environment.
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    let pool = super::connect_db().await?;

    sqlx::query(
        "INSERT INTO site_settings (key, value) VALUES ('active_theme', $1)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"
    )
    .bind(&name)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update active_theme in database: {e}"))?;

    println!("Theme '{}' activated in database.", name);

    // Try to signal the running server to reload without a restart.
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
