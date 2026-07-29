//! CLI commands for site-level IP access control.
//!
//! Usage:
//!   synap-cli security allow-ip on [--hostname <domain>] --ip <cidr> [--ip <cidr> ...]
//!   synap-cli security allow-ip off [--hostname <domain>]
//!   synap-cli security allow-ip add --hostname <domain> --ip <cidr>
//!   synap-cli security allow-ip remove --hostname <domain> --ip <cidr>
//!   synap-cli security allow-ip status [--hostname <domain>]
//!   synap-cli security block-ip on [--hostname <domain>] --ip <cidr> [--ip <cidr> ...]
//!   synap-cli security block-ip off [--hostname <domain>]
//!   synap-cli security block-ip add --hostname <domain> --ip <cidr>
//!   synap-cli security block-ip remove --hostname <domain> --ip <cidr>
//!   synap-cli security block-ip status [--hostname <domain>]

use clap::Subcommand;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum SecurityAction {
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

pub async fn run(action: SecurityAction) -> anyhow::Result<()> {
    match action {
        SecurityAction::AllowIp { state } => match state {
            AllowIpState::On     { hostname, ips, database_url } => allow_ip_on(hostname, ips, database_url).await,
            AllowIpState::Off    { hostname, database_url } => allow_ip_off(hostname, database_url).await,
            AllowIpState::Add    { hostname, ip, database_url } => allow_ip_add(hostname, ip, database_url).await,
            AllowIpState::Remove { hostname, ip, database_url } => allow_ip_remove(hostname, ip, database_url).await,
            AllowIpState::Status { hostname, database_url } => allow_ip_status(hostname, database_url).await,
        },
        SecurityAction::BlockIp { state } => match state {
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
             Run 'security allow-ip off' instead if you want to open the site back up."
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
