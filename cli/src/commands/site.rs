//! CLI commands for multi-site management.
//!
//! Usage:
//!   synap-cli site create --hostname <domain>  # add a new empty site
//!   synap-cli site list                        # list all sites
//!   synap-cli site delete --id <uuid>          # remove a site and all its content
//!   synap-cli site maintenance on [--hostname <domain>] [--message <text>]
//!   synap-cli site maintenance off [--hostname <domain>]
//!   synap-cli site maintenance status [--hostname <domain>]
//!   synap-cli site allow-ip on [--hostname <domain>] --ip <cidr> [--ip <cidr> ...]
//!   synap-cli site allow-ip off [--hostname <domain>]
//!   synap-cli site allow-ip add --hostname <domain> --ip <cidr>
//!   synap-cli site allow-ip remove --hostname <domain> --ip <cidr>
//!   synap-cli site allow-ip status [--hostname <domain>]
//!   synap-cli site block-ip on [--hostname <domain>] --ip <cidr> [--ip <cidr> ...]
//!   synap-cli site block-ip off [--hostname <domain>]
//!   synap-cli site block-ip add --hostname <domain> --ip <cidr>
//!   synap-cli site block-ip remove --hostname <domain> --ip <cidr>
//!   synap-cli site block-ip status [--hostname <domain>]

use clap::Subcommand;
use sqlx::PgPool;
use uuid::Uuid;

const DEFAULT_MAINTENANCE_MESSAGE: &str =
    "This site is currently undergoing scheduled maintenance. Please check back soon.";

#[derive(Subcommand)]
pub enum SiteAction {
    /// Create a new empty site.
    Create {
        /// Hostname for the new site (e.g. client.example.com)
        #[arg(long)]
        hostname: String,
        /// Path to the install directory (e.g. /opt/synaptic-signals) so the
        /// default theme can be seeded into sites/{uuid}/themes/default/ and
        /// the uploads directory can be created at uploads/{uuid}/.
        /// If omitted the directory setup is skipped.
        #[arg(long)]
        install_dir: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// List all sites with their post counts.
    List {
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Delete a site and all its content (cascade).
    Delete {
        /// UUID of the site to delete
        #[arg(long)]
        id: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Rename a site's hostname — updates DB records, Caddyfile, embedded post
    /// content URLs, and the hostname symlink in uploads/.
    Rename {
        /// UUID of the site to rename
        #[arg(long)]
        id: String,
        /// New hostname (e.g. newdomain.com)
        #[arg(long)]
        hostname: String,
        /// Path to the Caddyfile to update
        #[arg(long, default_value = "/etc/caddy/Caddyfile")]
        caddyfile: String,
        /// Install directory containing the uploads/ folder (for symlink update).
        /// Defaults to the INSTALL_DIR environment variable set by synap-cli install.
        #[arg(long, env = "INSTALL_DIR")]
        install_dir: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Toggle a WordPress-style maintenance page for a site.
    /// Checked live (no cache, no restart needed) on every public request —
    /// see core/src/middleware/maintenance.rs. /admin/* is always exempt so
    /// you can still log in to turn it back off.
    Maintenance {
        #[command(subcommand)]
        state: MaintenanceState,
    },
    /// Block all traffic to a site except from specific IPs/CIDRs — like an
    /// .htaccess Allow/Deny list. Checked live (no cache, no restart needed)
    /// on every request — see core/src/middleware/ip_allowlist.rs. Unlike
    /// maintenance mode, /admin is NOT exempt: if you lock yourself out,
    /// you need shell access to the server to turn it back off.
    AllowIp {
        #[command(subcommand)]
        state: AllowIpState,
    },
    /// Block specific IPs/CIDRs from a site while leaving it open to
    /// everyone else — the inverse of `allow-ip`. Checked live (no cache,
    /// no restart needed) on every request — see
    /// core/src/middleware/ip_denylist.rs. Nothing is exempt: a blocked IP
    /// is blocked from /admin too.
    BlockIp {
        #[command(subcommand)]
        state: BlockIpState,
    },
}

#[derive(Subcommand)]
pub enum MaintenanceState {
    /// Turn maintenance mode on.
    On {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Message shown on the maintenance page. Reuses the last message
        /// (or a default) if omitted.
        #[arg(long)]
        message: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Turn maintenance mode off.
    Off {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Show whether maintenance mode is currently on, and the stored message.
    Status {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AllowIpState {
    /// Turn the IP allowlist on. All traffic is blocked except from --ip.
    On {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Allowed IP or CIDR (e.g. 203.0.113.9 or 203.0.113.0/24). Repeat
        /// to allow more than one. Reuses the previous list if omitted.
        #[arg(long = "ip")]
        ips: Vec<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Turn the IP allowlist off (site reachable by everyone again).
    Off {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Add a single IP/CIDR to the allowlist without replacing the rest of
    /// the list. Turns the allowlist on if it wasn't already.
    Add {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// IP or CIDR to allow (e.g. 203.0.113.9 or 203.0.113.0/24)
        #[arg(long)]
        ip: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Remove a single IP/CIDR from the allowlist, leaving the rest in place.
    Remove {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// IP or CIDR to remove — must match an existing entry exactly.
        #[arg(long)]
        ip: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Show whether the IP allowlist is on, and the stored list.
    Status {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum BlockIpState {
    /// Turn the IP denylist on. Everyone can reach the site except --ip.
    On {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Blocked IP or CIDR (e.g. 203.0.113.9 or 203.0.113.0/24). Repeat
        /// to block more than one. Reuses the previous list if omitted.
        #[arg(long = "ip")]
        ips: Vec<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Turn the IP denylist off (previously blocked IPs can reach the site again).
    Off {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Add a single IP/CIDR to the denylist without replacing the rest of
    /// the list. Turns the denylist on if it wasn't already.
    Add {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// IP or CIDR to block (e.g. 203.0.113.9 or 203.0.113.0/24)
        #[arg(long)]
        ip: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Remove a single IP/CIDR from the denylist, leaving the rest in place.
    Remove {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// IP or CIDR to unblock — must match an existing entry exactly.
        #[arg(long)]
        ip: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Show whether the IP denylist is on, and the stored list.
    Status {
        /// Hostname of the site (required if more than one site exists)
        #[arg(long)]
        hostname: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

pub async fn run(action: SiteAction) -> anyhow::Result<()> {
    match action {
        SiteAction::Create { hostname, install_dir, database_url } => create(hostname, install_dir, database_url).await,
        SiteAction::List   { database_url } => list(database_url).await,
        SiteAction::Delete { id, database_url } => delete(id, database_url).await,
        SiteAction::Rename { id, hostname, caddyfile, install_dir, database_url } =>
            rename(id, hostname, caddyfile, install_dir, database_url).await,
        SiteAction::Maintenance { state } => match state {
            MaintenanceState::On     { hostname, message, database_url } => maintenance_on(hostname, message, database_url).await,
            MaintenanceState::Off    { hostname, database_url } => maintenance_off(hostname, database_url).await,
            MaintenanceState::Status { hostname, database_url } => maintenance_status(hostname, database_url).await,
        },
        SiteAction::AllowIp { state } => match state {
            AllowIpState::On     { hostname, ips, database_url } => allow_ip_on(hostname, ips, database_url).await,
            AllowIpState::Off    { hostname, database_url } => allow_ip_off(hostname, database_url).await,
            AllowIpState::Add    { hostname, ip, database_url } => allow_ip_add(hostname, ip, database_url).await,
            AllowIpState::Remove { hostname, ip, database_url } => allow_ip_remove(hostname, ip, database_url).await,
            AllowIpState::Status { hostname, database_url } => allow_ip_status(hostname, database_url).await,
        },
        SiteAction::BlockIp { state } => match state {
            BlockIpState::On     { hostname, ips, database_url } => block_ip_on(hostname, ips, database_url).await,
            BlockIpState::Off    { hostname, database_url } => block_ip_off(hostname, database_url).await,
            BlockIpState::Add    { hostname, ip, database_url } => block_ip_add(hostname, ip, database_url).await,
            BlockIpState::Remove { hostname, ip, database_url } => block_ip_remove(hostname, ip, database_url).await,
            BlockIpState::Status { hostname, database_url } => block_ip_status(hostname, database_url).await,
        },
    }
}

/// Resolve a site by hostname, or auto-pick the only site if none is given.
async fn resolve_site(pool: &PgPool, hostname: Option<String>) -> anyhow::Result<(Uuid, String)> {
    if let Some(h) = hostname {
        let h = h.trim().to_lowercase();
        let id: Uuid = sqlx::query_scalar("SELECT id FROM sites WHERE hostname = $1")
            .bind(&h)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No site found with hostname '{h}'"))?;
        Ok((id, h))
    } else {
        let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, hostname FROM sites ORDER BY created_at")
            .fetch_all(pool)
            .await?;
        match rows.len() {
            0 => anyhow::bail!("No sites found."),
            1 => Ok(rows.into_iter().next().unwrap()),
            _ => {
                let list = rows.into_iter().map(|(_, h)| h).collect::<Vec<_>>().join(", ");
                anyhow::bail!("Multiple sites found — specify --hostname. Available: {list}")
            }
        }
    }
}

async fn set_site_setting(pool: &PgPool, site_id: Uuid, key: &str, value: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO site_settings (site_id, key, value) VALUES ($1, $2, $3)
         ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL DO UPDATE SET value = EXCLUDED.value"
    )
    .bind(site_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to set {key}: {e}"))?;
    Ok(())
}

async fn get_site_setting(pool: &PgPool, site_id: Uuid, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE site_id = $1 AND key = $2")
        .bind(site_id)
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn maintenance_on(hostname: Option<String>, message: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let message = match message {
        Some(m) => m,
        None => get_site_setting(&pool, site_id, "maintenance_message")
            .await
            .unwrap_or_else(|| DEFAULT_MAINTENANCE_MESSAGE.to_string()),
    };

    set_site_setting(&pool, site_id, "maintenance_mode", "true").await?;
    set_site_setting(&pool, site_id, "maintenance_message", &message).await?;

    println!("Maintenance mode is now ON for '{hostname}'.");
    println!("Message: {message}");
    println!("Takes effect immediately — no restart needed. /admin/* stays reachable.");
    Ok(())
}

async fn maintenance_off(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    set_site_setting(&pool, site_id, "maintenance_mode", "false").await?;

    println!("Maintenance mode is now OFF for '{hostname}'.");
    Ok(())
}

async fn maintenance_status(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let mode = get_site_setting(&pool, site_id, "maintenance_mode").await.unwrap_or_else(|| "false".to_string());
    let message = get_site_setting(&pool, site_id, "maintenance_message").await;

    println!("Site: {hostname}");
    println!("Maintenance mode: {}", if mode == "true" { "ON" } else { "OFF" });
    if let Some(m) = message {
        println!("Message: {m}");
    }
    Ok(())
}

async fn allow_ip_add(hostname: Option<String>, ip: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let ip = ip.trim().to_string();
    validate_ip_entry(&ip)?;

    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let existing = get_site_setting(&pool, site_id, "ip_allowlist").await.unwrap_or_default();
    let mut entries = split_list(&existing);

    if entries.iter().any(|e| e == &ip) {
        println!("'{ip}' is already on the allowlist for '{hostname}'.");
    } else {
        entries.push(ip.clone());
        set_site_setting(&pool, site_id, "ip_allowlist", &entries.join(",")).await?;
        println!("Added '{ip}' to the allowlist for '{hostname}'.");
    }

    set_site_setting(&pool, site_id, "ip_allowlist_enabled", "true").await?;
    println!("Allowed: {}", entries.join(", "));
    println!("Takes effect immediately — no restart needed.");
    println!("WARNING: unlike maintenance mode, /admin is blocked too for anyone not on this list.");
    Ok(())
}

async fn allow_ip_remove(hostname: Option<String>, ip: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let ip = ip.trim().to_string();

    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let existing = get_site_setting(&pool, site_id, "ip_allowlist").await.unwrap_or_default();
    let mut entries = split_list(&existing);

    let before = entries.len();
    entries.retain(|e| e != &ip);

    if entries.len() == before {
        println!("'{ip}' was not on the allowlist for '{hostname}' — nothing to remove.");
        return Ok(());
    }

    if entries.is_empty() {
        anyhow::bail!(
            "Refusing to remove the last allowed IP — that would leave the allowlist \
             enabled with nobody able to reach '{hostname}', including /admin. \
             Run 'site allow-ip off' instead if you want to open the site back up."
        );
    }

    set_site_setting(&pool, site_id, "ip_allowlist", &entries.join(",")).await?;
    println!("Removed '{ip}' from the allowlist for '{hostname}'.");
    println!("Allowed: {}", entries.join(", "));
    Ok(())
}

async fn allow_ip_on(hostname: Option<String>, ips: Vec<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let list = if ips.is_empty() {
        get_site_setting(&pool, site_id, "ip_allowlist").await.unwrap_or_default()
    } else {
        for ip in &ips {
            let entry = ip.trim();
            let (addr_part, _) = entry.split_once('/').unwrap_or((entry, ""));
            if addr_part.parse::<std::net::IpAddr>().is_err() {
                anyhow::bail!("'{entry}' is not a valid IP or CIDR (e.g. 203.0.113.9 or 203.0.113.0/24).");
            }
        }
        ips.join(",")
    };

    if list.is_empty() {
        anyhow::bail!("No IPs on file yet — pass at least one --ip <cidr> the first time you turn this on.");
    }

    set_site_setting(&pool, site_id, "ip_allowlist", &list).await?;
    set_site_setting(&pool, site_id, "ip_allowlist_enabled", "true").await?;

    println!("IP allowlist is now ON for '{hostname}'.");
    println!("Allowed: {list}");
    println!("Takes effect immediately — no restart needed.");
    println!("WARNING: unlike maintenance mode, /admin is blocked too. If none of the");
    println!("allowed IPs is yours, you'll need shell access to the server to undo this.");
    Ok(())
}

async fn allow_ip_off(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    set_site_setting(&pool, site_id, "ip_allowlist_enabled", "false").await?;

    println!("IP allowlist is now OFF for '{hostname}' — site reachable by everyone again.");
    Ok(())
}

async fn allow_ip_status(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let enabled = get_site_setting(&pool, site_id, "ip_allowlist_enabled").await.unwrap_or_else(|| "false".to_string());
    let list = get_site_setting(&pool, site_id, "ip_allowlist").await;

    println!("Site: {hostname}");
    println!("IP allowlist: {}", if enabled == "true" { "ON" } else { "OFF" });
    if let Some(l) = list {
        println!("Allowed: {l}");
    }
    Ok(())
}

/// Validate an "1.2.3.4" or "1.2.3.0/24" entry (IPv4 or IPv6).
fn validate_ip_entry(entry: &str) -> anyhow::Result<()> {
    let (addr_part, _) = entry.split_once('/').unwrap_or((entry, ""));
    if addr_part.parse::<std::net::IpAddr>().is_err() {
        anyhow::bail!("'{entry}' is not a valid IP or CIDR (e.g. 203.0.113.9 or 203.0.113.0/24).");
    }
    Ok(())
}

/// Parse a comma-separated site_settings list into entries, trimmed and
/// with blanks dropped.
fn split_list(list: &str) -> Vec<String> {
    list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

async fn block_ip_add(hostname: Option<String>, ip: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let ip = ip.trim().to_string();
    validate_ip_entry(&ip)?;

    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let existing = get_site_setting(&pool, site_id, "ip_denylist").await.unwrap_or_default();
    let mut entries = split_list(&existing);

    if entries.iter().any(|e| e == &ip) {
        println!("'{ip}' is already on the denylist for '{hostname}'.");
    } else {
        entries.push(ip.clone());
        set_site_setting(&pool, site_id, "ip_denylist", &entries.join(",")).await?;
        println!("Added '{ip}' to the denylist for '{hostname}'.");
    }

    set_site_setting(&pool, site_id, "ip_denylist_enabled", "true").await?;
    println!("Blocked: {}", entries.join(", "));
    println!("Takes effect immediately — no restart needed.");
    Ok(())
}

async fn block_ip_remove(hostname: Option<String>, ip: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let ip = ip.trim().to_string();

    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let existing = get_site_setting(&pool, site_id, "ip_denylist").await.unwrap_or_default();
    let mut entries = split_list(&existing);

    let before = entries.len();
    entries.retain(|e| e != &ip);

    if entries.len() == before {
        println!("'{ip}' was not on the denylist for '{hostname}' — nothing to remove.");
        return Ok(());
    }

    set_site_setting(&pool, site_id, "ip_denylist", &entries.join(",")).await?;
    println!("Removed '{ip}' from the denylist for '{hostname}'.");

    if entries.is_empty() {
        set_site_setting(&pool, site_id, "ip_denylist_enabled", "false").await?;
        println!("Denylist is now empty — turned OFF automatically.");
    } else {
        println!("Blocked: {}", entries.join(", "));
    }
    Ok(())
}

async fn block_ip_on(hostname: Option<String>, ips: Vec<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let list = if ips.is_empty() {
        get_site_setting(&pool, site_id, "ip_denylist").await.unwrap_or_default()
    } else {
        for ip in &ips {
            let entry = ip.trim();
            let (addr_part, _) = entry.split_once('/').unwrap_or((entry, ""));
            if addr_part.parse::<std::net::IpAddr>().is_err() {
                anyhow::bail!("'{entry}' is not a valid IP or CIDR (e.g. 203.0.113.9 or 203.0.113.0/24).");
            }
        }
        ips.join(",")
    };

    if list.is_empty() {
        anyhow::bail!("No IPs on file yet — pass at least one --ip <cidr> the first time you turn this on.");
    }

    set_site_setting(&pool, site_id, "ip_denylist", &list).await?;
    set_site_setting(&pool, site_id, "ip_denylist_enabled", "true").await?;

    println!("IP denylist is now ON for '{hostname}'.");
    println!("Blocked: {list}");
    println!("Everyone else can still reach the site. Takes effect immediately — no restart needed.");
    Ok(())
}

async fn block_ip_off(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    set_site_setting(&pool, site_id, "ip_denylist_enabled", "false").await?;

    println!("IP denylist is now OFF for '{hostname}' — previously blocked IPs can reach the site again.");
    Ok(())
}

async fn block_ip_status(hostname: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let (site_id, hostname) = resolve_site(&pool, hostname).await?;

    let enabled = get_site_setting(&pool, site_id, "ip_denylist_enabled").await.unwrap_or_else(|| "false".to_string());
    let list = get_site_setting(&pool, site_id, "ip_denylist").await;

    println!("Site: {hostname}");
    println!("IP denylist: {}", if enabled == "true" { "ON" } else { "OFF" });
    if let Some(l) = list {
        println!("Blocked: {l}");
    }
    Ok(())
}

async fn create(hostname: String, install_dir: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let hostname = hostname.trim().to_lowercase();

    let site_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sites (hostname) VALUES ($1) RETURNING id"
    )
    .bind(&hostname)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
            anyhow::anyhow!("A site with hostname '{}' already exists.", hostname)
        } else {
            anyhow::anyhow!("Failed to create site: {e}")
        }
    })?;

    println!("Created site '{}' with id {}", hostname, site_id);

    // Auto-assign the protected super_admin as owner.
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    if let Some(owner_id) = owner {
        sqlx::query(
            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL"
        )
        .bind(owner_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set site owner: {e}"))?;
        println!("Site owner set to protected super_admin ({}).", owner_id);
        // Set the owner's default_site_id if not already set.
        sqlx::query(
            "UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2 AND default_site_id IS NULL"
        )
        .bind(site_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set default site: {e}"))?;
    } else {
        println!("No protected super_admin found — owner_user_id left NULL.");
        println!("Backfill with: UPDATE sites SET owner_user_id = '<user-uuid>' WHERE id = '{}'", site_id);
    }

    // Create the site's directories and seed the default theme.
    if let Some(ref base) = install_dir {
        let site_themes_dst = std::path::Path::new(base)
            .join("sites").join(site_id.to_string()).join("themes").join("default");
        let site_uploads_dst = std::path::Path::new(base)
            .join("uploads").join(site_id.to_string());

        if let Err(e) = std::fs::create_dir_all(&site_uploads_dst) {
            println!("Warning: could not create uploads/{}: {}", site_id, e);
        } else {
            println!("Created uploads/{}/", site_id);
        }

        // Create hostname symlink: uploads/{hostname} → uploads/{uuid}/
        let sym_path = std::path::Path::new(base).join("uploads").join(&hostname);
        if !sym_path.exists() {
            match std::os::unix::fs::symlink(&site_uploads_dst, &sym_path) {
                Ok(()) => println!("Created symlink uploads/{} -> uploads/{}/", hostname, site_id),
                Err(e) => println!("Warning: could not create upload symlink: {}", e),
            }
        }

        let theme_src = std::path::Path::new(base).join("themes").join("global").join("default");
        if theme_src.is_dir() {
            match copy_dir_all(&theme_src, &site_themes_dst) {
                Ok(()) => println!("Default theme seeded to sites/{}/themes/default/", site_id),
                Err(e) => println!(
                    "Warning: could not copy default theme ({}). \
                     Copy themes/global/default/ to sites/{}/themes/default/ manually.",
                    e, site_id
                ),
            }
        } else {
            println!(
                "Note: themes/global/default/ not found. \
                 Copy it to sites/{}/themes/default/ manually.",
                site_id
            );
        }
    } else {
        println!(
            "Note: pass --install-dir <path> to automatically create site directories \
             and seed the default theme."
        );
    }

    Ok(())
}

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

async fn rename(
    id_str: String,
    new_hostname: String,
    caddyfile: String,
    install_dir: Option<String>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let id: Uuid = id_str.parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid UUID.", id_str))?;

    // Fetch current hostname.
    let old_hostname: Option<String> = sqlx::query_scalar(
        "SELECT hostname FROM sites WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    let old_hostname = old_hostname
        .ok_or_else(|| anyhow::anyhow!("No site with id '{}' found.", id))?;

    let new_hostname = new_hostname.trim().to_lowercase();
    if old_hostname == new_hostname {
        println!("Site '{}' already uses that hostname — nothing to do.", old_hostname);
        return Ok(());
    }

    // Check the new hostname isn't already taken by another site.
    let conflict: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM sites WHERE hostname = $1"
    )
    .bind(&new_hostname)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    if let Some(other) = conflict {
        if other != id {
            anyhow::bail!("Hostname '{}' is already used by another site ({}).", new_hostname, other);
        }
    }

    println!();
    println!("  Rename site:");
    println!("    ID:           {}", id);
    println!("    Old hostname: {}", old_hostname);
    println!("    New hostname: {}", new_hostname);
    println!();
    println!("  This will update:");
    println!("    • sites.hostname");
    println!("    • site_settings site_url");
    println!("    • posts.content — /uploads/{} → /uploads/{}", old_hostname, new_hostname);
    println!("    • Caddyfile block header: {}", caddyfile);
    if let Some(ref dir) = install_dir {
        println!("    • uploads/ symlink in: {}/uploads/", dir);
    }
    println!();
    println!("  Note: hostname text manually typed into post body content (not via");
    println!("  the media picker) will NOT be automatically updated.");
    println!();

    print!("  Proceed? [y/N] ");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    // 1. Update sites.hostname.
    sqlx::query("UPDATE sites SET hostname = $1, updated_at = NOW() WHERE id = $2")
        .bind(&new_hostname)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update hostname: {e}"))?;
    println!("  ✓ Updated sites.hostname");

    // 2. Update site_settings site_url.
    let new_url = format!("http://{}", new_hostname);
    sqlx::query(
        "UPDATE site_settings SET value = $1 WHERE site_id = $2 AND key = 'site_url'"
    )
    .bind(&new_url)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update site_url: {e}"))?;
    println!("  ✓ Updated site_settings site_url to {}", new_url);

    // 3. Update embedded media upload URLs in posts.content.
    let old_prefix = format!("/uploads/{}/", old_hostname);
    let new_prefix = format!("/uploads/{}/", new_hostname);
    let updated_posts = sqlx::query(
        "UPDATE posts SET content = REPLACE(content, $1, $2) \
         WHERE site_id = $3 AND content LIKE '%' || $1 || '%'"
    )
    .bind(&old_prefix)
    .bind(&new_prefix)
    .bind(id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);
    if updated_posts > 0 {
        println!("  ✓ Updated {} post(s) with embedded upload URLs", updated_posts);
    } else {
        println!("  ✓ No embedded upload URLs to update in posts");
    }

    // 4. Update Caddyfile.
    match update_caddyfile(&caddyfile, &old_hostname, &new_hostname) {
        Ok(true) => {
            println!("  ✓ Updated Caddyfile: {} → {}", old_hostname, new_hostname);
            // Reload Caddy.
            let reload = std::process::Command::new("sudo")
                .args(["caddy", "reload", "--config", &caddyfile, "--adapter", "caddyfile"])
                .status();
            match reload {
                Ok(s) if s.success() => println!("  ✓ Caddy reloaded"),
                Ok(s) => println!("  Warning: caddy reload exited with {}", s),
                Err(e) => println!("  Warning: could not reload Caddy: {}", e),
            }
        }
        Ok(false) => println!(
            "  ✓ Caddyfile: no block for '{}' found (may not be configured yet)",
            old_hostname
        ),
        Err(e) => {
            println!("  Warning: could not update Caddyfile: {}", e);
            println!("    Manually replace '{}' with '{}' in {}", old_hostname, new_hostname, caddyfile);
        }
    }

    // 5. Update hostname symlink in uploads/.
    if let Some(ref dir) = install_dir {
        let uploads = std::path::Path::new(dir).join("uploads");
        let old_sym = uploads.join(&old_hostname);
        let new_sym = uploads.join(&new_hostname);
        let target  = uploads.join(id.to_string());

        if old_sym.is_symlink() {
            if let Err(e) = std::fs::remove_file(&old_sym) {
                println!("  Warning: could not remove old symlink: {}", e);
            }
        }
        if target.is_dir() && !new_sym.exists() {
            match std::os::unix::fs::symlink(&target, &new_sym) {
                Ok(()) => println!("  ✓ Symlink: uploads/{} -> uploads/{}/", new_hostname, id),
                Err(e) => {
                    println!("  Warning: could not create new symlink: {}", e);
                    println!("    Manually: ln -s {}/{} {}/{}", uploads.display(), id, uploads.display(), new_hostname);
                }
            }
        }
    } else {
        println!("  ℹ  --install-dir not provided — skipping symlink update.");
        println!("     Pass --install-dir <path> or set INSTALL_DIR to update the symlink.");
    }

    println!();
    println!("Rename complete. Restart Synaptic Signals to apply the new hostname.");
    println!();
    println!("Note: hostname text manually typed into post body content was not");
    println!("automatically updated. Review posts for any references to '{}'.", old_hostname);

    Ok(())
}

fn update_caddyfile(path: &str, old: &str, new_host: &str) -> std::io::Result<bool> {
    let content = std::fs::read_to_string(path)?;
    // Replace the site block header and log file path.
    let updated = content
        .replace(&format!("{} {{", old), &format!("{} {{", new_host))
        .replace(&format!("{}.log", old), &format!("{}.log", new_host));
    if updated == content {
        return Ok(false);
    }
    std::fs::write(path, &updated)?;
    Ok(true)
}

async fn list(database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        r#"SELECT s.id, s.hostname,
              (SELECT COUNT(*) FROM posts p WHERE p.site_id = s.id AND p.post_type = 'post') AS post_count
           FROM sites s
           ORDER BY s.created_at"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to list sites: {e}"))?;

    if rows.is_empty() {
        println!("No sites found. Run 'synap-cli install' to set up the first site.");
        return Ok(());
    }

    println!("\n{:<38} {:<30} {}", "ID", "Hostname", "Posts");
    println!("{}", "-".repeat(74));
    for (id, hostname, posts) in &rows {
        println!("{:<38} {:<30} {}", id, hostname, posts);
    }
    println!();

    Ok(())
}

async fn delete(id_str: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let id: Uuid = id_str.parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid UUID.", id_str))?;

    // Confirm the site exists.
    let hostname: Option<String> = sqlx::query_scalar(
        "SELECT hostname FROM sites WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    let hostname = hostname.ok_or_else(|| anyhow::anyhow!("No site with id '{}' found.", id))?;

    // Prompt for confirmation.
    print!("Delete site '{}' ({}) and ALL its content? [y/N] ", hostname, id);
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete site: {e}"))?;

    println!("Site '{}' deleted.", hostname);
    Ok(())
}
