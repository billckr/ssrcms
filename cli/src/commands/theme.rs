use clap::Subcommand;
use std::path::Path;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum ThemeAction {
    /// List themes — all sites overview, or scoped to one site with --site
    #[command(after_help = "Examples:\n  synap-cli theme list\n  synap-cli theme list --site example.com")]
    List {
        /// Filter to a specific site (hostname or UUID)
        #[arg(long)]
        site: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Copy a theme from global to the site folder and activate it
    ///
    /// If the site does not already have a local copy of the theme it is copied
    /// from themes/global/ first, then set as active — matching the behaviour
    /// of the 'Get Theme' button in the admin UI.
    #[command(after_help = "Examples:\n  synap-cli theme activate default --site example.com\n  synap-cli theme activate testing --site example.com")]
    Activate {
        /// Name of the theme to activate (directory name, e.g. default)
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
    ///
    /// The active theme cannot be removed — activate a different theme first.
    #[command(after_help = "Examples:\n  synap-cli theme remove testing --site example.com")]
    Remove {
        /// Name of the theme to remove (directory name, e.g. testing)
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
        ThemeAction::List { site, database_url } => list(site, database_url).await,
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

/// Collect all theme directories under themes/global/ and sites/{uuid}/themes/.
/// Returns (path, source_label) pairs. Falls back to flat themes/ for pre-multisite layouts.
fn collect_theme_dirs() -> Vec<(std::path::PathBuf, String)> {
    let themes_root = Path::new("themes");
    let sites_root  = Path::new("sites");
    let mut dirs: Vec<(std::path::PathBuf, String)> = Vec::new();

    let global_dir = themes_root.join("global");
    if global_dir.is_dir() {
        dirs.push((global_dir, "global".into()));
    }

    // Per-site themes live at sites/{uuid}/themes/.
    if sites_root.is_dir() {
        for entry in std::fs::read_dir(sites_root).into_iter().flatten().flatten() {
            let site_dir = entry.path();
            if !site_dir.is_dir() { continue; }
            let uuid_str = site_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
            let themes_dir = site_dir.join("themes");
            if themes_dir.is_dir() {
                let label = format!("site:{uuid_str}");
                dirs.push((themes_dir, label));
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

async fn list(site: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    let themes = scan_themes()?;

    if themes.is_empty() {
        println!("(no themes found)");
        return Ok(());
    }

    let active_map = load_active_themes(database_url).await;
    let all_site_names: std::collections::HashMap<Uuid, String> = load_all_site_names().await;

    match site {
        // ── Scoped view: one site ─────────────────────────────────────────────
        Some(ref s) => {
            // Resolve site identifier to UUID + hostname.
            let pool = super::connect_db().await?;
            let site_id = resolve_site_id(&pool, s).await?;
            let hostname = all_site_names.get(&site_id).cloned()
                .unwrap_or_else(|| site_id.to_string());
            let active_theme = active_map.get(&Some(site_id)).cloned()
                .unwrap_or_default();

            println!("\nThemes for {} ({})", hostname, site_id);
            println!("{}", "-".repeat(96));
            println!("{:<22} {:<20} {:<10} {:<6} {:<10} {}", "Name", "Slug", "Version", "API", "Status", "Description");
            println!("{}", "-".repeat(96));

            // Show global themes and this site's local copies, deduplicated by dir_name.
            // A site copy supersedes the global copy of the same name.
            let site_prefix = format!("site:{}", site_id);
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            // First pass: collect site-local themes (they take priority).
            let mut rows: Vec<(String, String, String, String, String, bool)> = Vec::new();
            for (dir_name, display_name, version, api, desc, source) in &themes {
                if source == &site_prefix {
                    let is_active = dir_name == &active_theme;
                    rows.push((dir_name.clone(), display_name.clone(), version.clone(), api.clone(), desc.clone(), is_active));
                    seen.insert(dir_name.clone());
                }
            }
            // Second pass: global themes not already installed locally.
            for (dir_name, display_name, version, api, desc, source) in &themes {
                if source == "global" && !seen.contains(dir_name) {
                    rows.push((dir_name.clone(), display_name.clone(), version.clone(), api.clone(), desc.clone(), false));
                }
            }
            rows.sort_by(|a, b| a.0.cmp(&b.0));

            for (dir_name, display_name, version, api, desc, is_active) in &rows {
                let installed = seen.contains(dir_name);
                let status = if *is_active {
                    "active"
                } else if installed {
                    "disabled"
                } else {
                    "available"
                };
                println!("  {:<20} {:<20} {:<10} {:<6} {:<10} {}", display_name, dir_name, version, api, status, desc);
            }
            println!();
        }

        // ── Overview: all sites ───────────────────────────────────────────────
        None => {
            println!("\n{:<22} {:<20} {:<10} {:<6} {:<16} {}", "Name", "Slug", "Version", "API", "Domain", "Description");
            println!("{}", "-".repeat(96));

            for (dir_name, display_name, version, api, desc, source) in &themes {
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

                println!("  {:<20} {:<20} {:<10} {:<6} {:<16} {}", display_name, dir_name, version, api, display_source, desc);
            }
            println!();
        }
    }

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
            let site_dest   = Path::new("sites").join(site_id.to_string()).join("themes").join(&name);

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
                println!("Copied '{}' from global → sites/{}/themes/{}.", name, site_id, name);
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

    let site_path = Path::new("sites").join(site_id.to_string()).join("themes").join(&name);

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

    // Guard: refuse to remove the last local theme — the site must always have at least one.
    let site_themes_dir = Path::new("sites").join(site_id.to_string()).join("themes");
    let local_theme_count = std::fs::read_dir(&site_themes_dir)
        .map(|entries| {
            entries.flatten()
                .filter(|e| e.path().is_dir() && e.path().join("theme.toml").exists())
                .count()
        })
        .unwrap_or(0);

    if local_theme_count <= 1 {
        anyhow::bail!(
            "Cannot remove '{}' — it is the only theme installed for this site. \
             Activate a different theme first (synap-cli theme activate <theme> --site <hostname>), \
             then remove this one.",
            name
        );
    }

    // Path traversal guard: confirm site_path is a direct child of the site's theme dir.
    let expected_parent = Path::new("sites")
        .join(site_id.to_string())
        .join("themes")
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
