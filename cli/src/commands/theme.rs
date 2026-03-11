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
    /// Copy a theme from global to the site folder and activate it
    Activate {
        /// Name of the theme to activate (must match the name field in theme.toml)
        #[arg(value_name = "THEME")]
        name: String,
        /// Site hostname or UUID to target (required for multi-site installs)
        #[arg(long)]
        site: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
        /// Path to the server PID file (used to signal a live reload without restart)
        #[arg(long, default_value = "synaptic.pid")]
        pid_file: String,
    },
    /// Remove a site's local copy of a theme (never touches the global original)
    Remove {
        /// Name of the theme to remove
        #[arg(value_name = "THEME")]
        name: String,
        /// Site hostname or UUID to target
        #[arg(long)]
        site: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
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
        ThemeAction::Remove { name, site, database_url } => {
            remove(name, site, database_url).await
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
/// Returns (dir_name, display_name, version, api_version, description, source).
/// dir_name is the folder name on disk — this is what active_theme in the DB stores.
/// display_name is the human-readable name from theme.toml.
fn scan_themes() -> anyhow::Result<Vec<(String, String, String, String, String, String)>> {
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
            let display_name = theme.get("name").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
            let version      = theme.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let api_version  = theme.get("api_version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let description  = theme.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            themes.push((dir_name, display_name, version, api_version, description, source.clone()));
        }
    }
    themes.sort_by(|a, b| a.5.cmp(&b.5).then(a.0.cmp(&b.0)));
    Ok(themes)
}

async fn list(database_url: Option<String>) -> anyhow::Result<()> {
    let themes = scan_themes()?;

    if themes.is_empty() {
        println!("(no themes found)");
        return Ok(());
    }

    // Load per-site active themes and all site hostnames from DB.
    let active_map = load_active_themes(database_url).await;
    let all_site_names: std::collections::HashMap<Uuid, String> = load_all_site_names().await;

    println!("\n{:<22} {:<10} {:<6} {:<16} {}", "Name", "Version", "API", "Domain", "Description");
    println!("{}", "-".repeat(82));

    for (dir_name, display_name, version, api, desc, source) in &themes {
        // active_theme in the DB stores the directory name, so match on that.
        let active_for: Vec<String> = active_map.iter().filter_map(|(site_id, t)| {
            if t == dir_name {
                let label = match site_id {
                    Some(id) => all_site_names.get(id).cloned().unwrap_or_else(|| id.to_string()),
                    None => "global".to_string(),
                };
                Some(label)
            } else {
                None
            }
        }).collect();

        // Resolve the source label: "global" stays as-is, "site:<uuid>" becomes the hostname.
        let display_source = if source == "global" {
            "global".to_string()
        } else if let Some(uuid_str) = source.strip_prefix("site:") {
            if let Ok(id) = uuid_str.parse::<Uuid>() {
                all_site_names.get(&id).cloned().unwrap_or_else(|| uuid_str.to_string())
            } else {
                source.clone()
            }
        } else {
            source.clone()
        };

        let marker = if active_for.is_empty() { "  " } else { " *" };
        let active_str = if active_for.is_empty() {
            String::new()
        } else {
            format!("  [active: {}]", active_for.join(", "))
        };
        println!("{}{:<20} {:<10} {:<6} {:<16} {}{}", marker, display_name, version, api, display_source, desc, active_str);
    }

    println!();
    Ok(())
}

/// Load active_theme setting for every site from site_settings.
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

/// Fetch hostnames for site UUIDs referenced in the active_map.
/// Fetch all site UUIDs and their hostnames from the DB.
async fn load_all_site_names() -> std::collections::HashMap<Uuid, String> {
    let pool = match super::connect_db().await {
        Ok(p) => p,
        Err(_) => return std::collections::HashMap::new(),
    };
    let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, hostname FROM sites")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    rows.into_iter().collect()
}

/// Resolve a --site value (hostname or UUID string) to a UUID using the DB.
async fn resolve_site_id(pool: &sqlx::PgPool, site: &str) -> anyhow::Result<Uuid> {
    if let Ok(id) = site.parse::<Uuid>() {
        return Ok(id);
    }
    sqlx::query_scalar("SELECT id FROM sites WHERE hostname = $1")
        .bind(site)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow::anyhow!("DB error looking up site: {e}"))?
        .ok_or_else(|| anyhow::anyhow!(
            "No site found with hostname '{}'. Run 'synap-cli site list' to see available sites.", site
        ))
}

/// Recursive directory copy — matches the behaviour of copy_dir_all in the web handler.
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

async fn activate(
    name: String,
    site: Option<String>,
    database_url: Option<String>,
    pid_file: String,
) -> anyhow::Result<()> {
    // Reject obviously unsafe names (matches web handler guard).
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        anyhow::bail!("Invalid theme name.");
    }

    if let Some(url) = database_url {
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    let pool = super::connect_db().await?;
    let themes_root = Path::new("themes");

    match &site {
        None => {
            // Single-site / global path — theme must exist somewhere on disk.
            let global_src = themes_root.join("global").join(&name);
            if !global_src.is_dir() {
                anyhow::bail!("Theme '{}' not found in themes/global/.", name);
            }

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

        Some(s) => {
            let site_id = resolve_site_id(&pool, s).await?;

            let global_src  = themes_root.join("global").join(&name);
            let site_dest   = themes_root.join("sites").join(site_id.to_string()).join(&name);

            if site_dest.is_dir() {
                // Already has a local copy — just update the DB setting.
                println!("Site already has a local copy of '{}' — skipping copy.", name);
            } else {
                // Copy from global into the site folder, exactly as the web UI does.
                if !global_src.is_dir() {
                    anyhow::bail!(
                        "Theme '{}' not found in themes/global/. \
                         Only global themes can be copied to a site.", name
                    );
                }
                copy_dir_all(&global_src, &site_dest)
                    .map_err(|e| anyhow::anyhow!("Failed to copy theme '{}': {e}", name))?;
                println!("Copied '{}' from global → themes/sites/{}/{}.", name, site_id, name);
            }

            // Update site_settings for this site.
            sqlx::query(
                "INSERT INTO site_settings (site_id, key, value) VALUES ($1, 'active_theme', $2)
                 ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL DO UPDATE SET value = EXCLUDED.value"
            )
            .bind(site_id)
            .bind(&name)
            .execute(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update active_theme for site: {e}"))?;

            println!("Theme '{}' activated for site '{}'.", name, s);
        }
    }

    signal_reload(&pid_file, &name);
    Ok(())
}

async fn remove(
    name: String,
    site: Option<String>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    // Reject obviously unsafe names.
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        anyhow::bail!("Invalid theme name.");
    }

    if let Some(url) = database_url {
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    let pool = super::connect_db().await?;
    let themes_root = Path::new("themes");

    let site_id = match &site {
        Some(s) => resolve_site_id(&pool, s).await?,
        None => {
            // Try to infer the site if there is exactly one.
            let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM sites")
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
            match rows.as_slice() {
                [(id,)] => *id,
                [] => anyhow::bail!("No sites found. Run 'synap-cli site init' first."),
                _ => anyhow::bail!(
                    "Multiple sites found — use --site <hostname> to specify which one."
                ),
            }
        }
    };

    let site_path = themes_root.join("sites").join(site_id.to_string()).join(&name);

    if !site_path.is_dir() {
        anyhow::bail!(
            "Theme '{}' not found in the site's local folder. \
             Only site-local copies can be removed — the global original is never touched.",
            name
        );
    }

    // Guard: refuse to remove the currently active theme.
    let active: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'active_theme'"
    )
    .bind(site_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if active.as_deref() == Some(&name) {
        anyhow::bail!(
            "Cannot remove the active theme '{}'. Activate a different theme first.", name
        );
    }

    // Path traversal guard: confirm site_path is a direct child of the site's theme dir.
    let expected_parent = themes_root
        .join("sites")
        .join(site_id.to_string())
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Site theme directory not found."))?;

    let canonical = site_path.canonicalize()
        .map_err(|_| anyhow::anyhow!("Theme path could not be resolved."))?;

    if canonical.parent() != Some(expected_parent.as_path()) {
        anyhow::bail!("Invalid theme path — path traversal detected.");
    }

    std::fs::remove_dir_all(&canonical)
        .map_err(|e| anyhow::anyhow!("Failed to remove theme '{}': {e}", name))?;

    let id_str = site_id.to_string();
    let label = site.as_deref().unwrap_or(&id_str);
    println!("Theme '{}' removed from site '{}'. The global original is untouched.", name, label);
    Ok(())
}

/// Read the PID file and send SIGUSR1 to the server process.
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
