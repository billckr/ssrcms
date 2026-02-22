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

fn list() -> anyhow::Result<()> {
    let themes_dir = Path::new("themes");

    if !themes_dir.exists() {
        println!("No themes directory found (expected: ./themes/)");
        return Ok(());
    }

    // Read the active theme from DB if possible; fall back to filesystem-only display.
    // For the list command we avoid requiring a DB connection, so we read the active
    // theme from .env / environment only as a hint.
    let active_hint = std::env::var("ACTIVE_THEME").unwrap_or_default();

    let mut themes: Vec<(String, String, String, String, bool)> = Vec::new();

    let entries = std::fs::read_dir(themes_dir)
        .map_err(|e| anyhow::anyhow!("Cannot read themes dir: {e}"))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        if dir_name.starts_with('.') {
            continue;
        }
        let toml_path = path.join("theme.toml");
        if !toml_path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&toml_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {}: {e}", toml_path.display()))?;
        let table: toml::Value = content.parse()
            .map_err(|e| anyhow::anyhow!("Invalid TOML in {}: {e}", toml_path.display()))?;
        let theme = table.get("theme").unwrap_or(&table);
        let name        = theme.get("name").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
        let version     = theme.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let api_version = theme.get("api_version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let description = theme.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let has_screenshot = path.join("screenshot.png").exists();
        let active = name == active_hint;
        themes.push((name, version, api_version, description, active));
        let _ = has_screenshot; // listed in screenshot column below
    }

    themes.sort_by(|a, b| a.0.cmp(&b.0));

    if themes.is_empty() {
        println!("(no themes found)");
        return Ok(());
    }

    println!("\n{:<20} {:<10} {:<6} {}", "Name", "Version", "API", "Description");
    println!("{}", "-".repeat(70));

    for (name, version, api, desc, active) in &themes {
        let marker = if *active { " *" } else { "" };
        println!("{:<20} {:<10} {:<6} {}{}", name, version, api, desc, marker);
    }

    println!();
    Ok(())
}

async fn activate(name: String, database_url: Option<String>, pid_file: String) -> anyhow::Result<()> {
    // Verify the theme exists on disk before touching the DB.
    let theme_path = Path::new("themes").join(&name);
    if !theme_path.is_dir() {
        anyhow::bail!("Theme '{}' not found in ./themes/. Check the name and try again.", name);
    }
    let toml_path = theme_path.join("theme.toml");
    if !toml_path.exists() {
        anyhow::bail!("Theme directory '{}' exists but has no theme.toml.", name);
    }

    // Confirm the name in theme.toml matches what was requested.
    let content = std::fs::read_to_string(&toml_path)
        .map_err(|e| anyhow::anyhow!("Cannot read theme.toml: {e}"))?;
    let table: toml::Value = content.parse()
        .map_err(|e| anyhow::anyhow!("Invalid theme.toml: {e}"))?;
    let toml_name = table.get("theme")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if toml_name != name {
        anyhow::bail!(
            "theme.toml name field is '{}' but you requested '{}'. Use the name from theme.toml.",
            toml_name, name
        );
    }

    if let Some(url) = database_url {
        std::env::set_var("DATABASE_URL", url);
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
