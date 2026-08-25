//! CLI commands for managing Caddy file-write permissions and sudoers entries.
//!
//! Usage:
//!   synap caddy setup   --app-user <user> [--caddyfile <path>]
//!   synap caddy teardown --app-user <user> [--caddyfile <path>]

use clap::Subcommand;
use std::process::Command;

const SUDOERS_FILE: &str = "/etc/sudoers.d/synaptic-caddy";

#[derive(Subcommand)]
pub enum CaddyAction {
    /// Set up Caddy write permission and caddy-reload sudoers entry.
    /// Adds app-user to the caddy group, makes the Caddyfile group-writable,
    /// and writes /etc/sudoers.d/synaptic-caddy. Idempotent — safe to run again
    /// on reinstall without breaking anything.
    /// Must be run as root (or via sudo).
    Setup {
        /// System user the app runs as (e.g. www-data, synaptic)
        #[arg(long)]
        app_user: String,
        /// Path to the Caddyfile to make group-writable
        #[arg(long, default_value = "/etc/caddy/Caddyfile")]
        caddyfile: String,
    },
    /// Reverse the changes made by `caddy setup`:
    /// removes the sudoers drop-in, restores Caddyfile to 640, removes
    /// app-user from the caddy group.
    /// Must be run as root (or via sudo).
    Teardown {
        /// System user the app runs as
        #[arg(long)]
        app_user: String,
        /// Path to the Caddyfile to restore permissions on
        #[arg(long, default_value = "/etc/caddy/Caddyfile")]
        caddyfile: String,
    },
    /// Add a Caddy site block for a domain that does NOT resolve to this
    /// server yet — e.g. still just an /etc/hosts loopback entry for local
    /// dev, or public DNS that hasn't propagated. Issues a locally-trusted
    /// self-signed certificate (`tls internal`) instead of attempting real
    /// ACME, which would fail for an unreachable domain anyway.
    ///
    /// Gated behind the super-admin password: this deliberately bypasses the
    /// DNS-ownership check the admin panel's "Enable SSL" button enforces
    /// (see `provision_ssl`/`dns_points_here` in
    /// core/src/handlers/admin/sites.rs), so DB/server access alone
    /// shouldn't be enough to force a domain onto self-signed TLS.
    ///
    /// Idempotent — a no-op if a block for the hostname already exists.
    /// Must be run as root (or a user in the `caddy` group — see `setup`).
    ProvisionLocal {
        /// Hostname to add (e.g. staging.example.com)
        #[arg(long)]
        hostname: String,
        /// Port the app listens on (defaults to PORT/synaptic.toml, else 3000)
        #[arg(long)]
        port: Option<u16>,
        /// Path to the Caddyfile
        #[arg(long, default_value = "/etc/caddy/Caddyfile")]
        caddyfile: String,
        /// Super-admin password (skips interactive prompt — use only in scripts)
        #[arg(long)]
        password: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

pub async fn run(action: CaddyAction) -> anyhow::Result<()> {
    match action {
        CaddyAction::Setup { app_user, caddyfile } => setup(&app_user, &caddyfile),
        CaddyAction::Teardown { app_user, caddyfile } => teardown(&app_user, &caddyfile),
        CaddyAction::ProvisionLocal { hostname, port, caddyfile, password, database_url } =>
            provision_local(hostname, port, caddyfile, password, database_url).await,
    }
}

async fn provision_local(
    hostname: String,
    port: Option<u16>,
    caddyfile: String,
    password: Option<String>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    // Gate behind the super-admin password before touching Caddy config —
    // see the doc comment on `ProvisionLocal` for why this can't be left to
    // OS-level permissions alone.
    super::verify_super_admin_password(&pool, password).await?;

    let hostname = hostname.trim().to_lowercase();
    if hostname.is_empty() {
        anyhow::bail!("Hostname cannot be empty.");
    }

    let config = synaptic_core::config::AppConfig::load()
        .map_err(|e| anyhow::anyhow!("Failed to load app config: {e}"))?;
    let port = port.unwrap_or(config.port);

    let existing = std::fs::read_to_string(&caddyfile)
        .map_err(|e| anyhow::anyhow!("Cannot read {caddyfile}: {e}"))?;

    if synaptic_core::caddy::caddy_block_exists(&existing, &hostname) {
        println!("Caddy already has a block for '{hostname}' — nothing to do.");
        return Ok(());
    }

    let block = synaptic_core::caddy::build_caddy_block(&hostname, port, &config.uploads_dir, true);
    let new_content = format!("{}\n{}\n", existing.trim_end(), block);

    std::fs::write(&caddyfile, &new_content).map_err(|e| {
        anyhow::anyhow!(
            "Cannot write {caddyfile}: {e}\n\
             Run this as root, or run 'synap caddy setup --app-user <user>' first."
        )
    })?;

    println!("Added Caddy block for '{hostname}' (proxying to localhost:{port}, self-signed TLS).");

    // Deliberately no separate `caddy validate` call here: this whole
    // command typically runs under `sudo` (root), and `validate` doesn't
    // just check syntax — it transiently instantiates the full config,
    // which opens/creates any referenced log files under the CALLER's uid.
    // That leaves e.g. a fresh `{hostname}.log` owned by root, which the
    // real daemon (running as the unprivileged `caddy` user) can then never
    // write to. `caddy reload` already validates via its own client-side
    // adapt step before it ever POSTs to the running instance, and the
    // instance itself (already running as `caddy`) is what actually opens
    // new log files — so nothing is lost by skipping the standalone check.
    match std::process::Command::new("caddy")
        .args(["reload", "--config", &caddyfile, "--adapter", "caddyfile"])
        .status()
    {
        Ok(s) if s.success() => println!("Caddy reloaded."),
        Ok(s) => anyhow::bail!("caddy reload failed (exit {s}). Check: journalctl -u caddy -n 50"),
        Err(e) => anyhow::bail!("Failed to run 'caddy reload': {e}"),
    }

    println!();
    println!("Done. Make sure '{hostname}' is in /etc/hosts pointing at 127.0.0.1");
    println!("(or wherever this server is actually reachable), then visit https://{hostname}");
    println!();
    println!("First time only, so browsers trust Caddy's local certificate authority:");
    println!("  sudo caddy trust --config {caddyfile}");

    Ok(())
}

/// Set up Caddy write permissions for the given app user.
/// Called both by the `caddy setup` subcommand and by the installer when
/// `--app-user` is provided.  Idempotent — safe to call on reinstall.
pub fn setup_caddy_permissions(app_user: &str, caddyfile_path: &str) -> anyhow::Result<()> {
    setup(app_user, caddyfile_path)
}

fn setup(app_user: &str, caddyfile_path: &str) -> anyhow::Result<()> {
    // 1. Add app user to the caddy group (usermod -aG is idempotent).
    println!("  Adding '{}' to the 'caddy' group...", app_user);
    let status = Command::new("usermod")
        .args(["-aG", "caddy", app_user])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run usermod: {e}\nIs this running as root?"))?;
    if !status.success() {
        anyhow::bail!(
            "usermod -aG caddy {} failed (exit {}). \
             Ensure the 'caddy' group exists and you are running as root.",
            app_user, status
        );
    }

    // 2. Make the Caddyfile group-writable so the app user can append blocks.
    println!("  Making {} group-writable...", caddyfile_path);
    let status = Command::new("chmod")
        .args(["g+w", caddyfile_path])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run chmod: {e}"))?;
    if !status.success() {
        anyhow::bail!(
            "chmod g+w {} failed (exit {}). Does the file exist?",
            caddyfile_path, status
        );
    }

    // 3. Ensure /var/log/caddy/ exists and is owned by caddy:caddy so that
    //    Caddy can create per-site log files without permission errors.
    let log_dir = "/var/log/caddy";
    println!("  Ensuring {} exists with caddy:caddy ownership...", log_dir);
    std::fs::create_dir_all(log_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create {}: {e}", log_dir))?;
    let status = Command::new("chown")
        .args(["caddy:caddy", log_dir])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run chown on {}: {e}", log_dir))?;
    if !status.success() {
        anyhow::bail!("chown caddy:caddy {} failed (exit {})", log_dir, status);
    }
    let status = Command::new("chmod")
        .args(["755", log_dir])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run chmod on {}: {e}", log_dir))?;
    if !status.success() {
        anyhow::bail!("chmod 755 {} failed (exit {})", log_dir, status);
    }

    println!("  Caddy permissions configured for '{}'.", app_user);
    println!(
        "  Note: group membership takes effect on the next login/session for '{}'.",
        app_user
    );
    Ok(())
}

fn teardown(app_user: &str, caddyfile_path: &str) -> anyhow::Result<()> {
    // 1. Remove the sudoers drop-in.
    println!("  Removing {}...", SUDOERS_FILE);
    match std::fs::remove_file(SUDOERS_FILE) {
        Ok(())                                                   => println!("  Removed {}.", SUDOERS_FILE),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound      => println!("  {} not found — skipped.", SUDOERS_FILE),
        Err(e)                                                   => anyhow::bail!("Failed to remove {}: {e}", SUDOERS_FILE),
    }

    // 2. Restore Caddyfile to 640 (group-readable only, not writable).
    println!("  Restoring {} permissions to 640...", caddyfile_path);
    let status = Command::new("chmod")
        .args(["640", caddyfile_path])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run chmod: {e}"))?;
    if !status.success() {
        println!(
            "  Warning: chmod 640 {} failed (exit {}) — file may not exist.",
            caddyfile_path, status
        );
    }

    // 3. Remove app user from the caddy group.
    println!("  Removing '{}' from the 'caddy' group...", app_user);
    let status = Command::new("gpasswd")
        .args(["-d", app_user, "caddy"])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run gpasswd: {e}"))?;
    if !status.success() {
        println!(
            "  Warning: gpasswd -d {} caddy failed — user may not have been in the group.",
            app_user
        );
    }

    println!("  Caddy permissions removed for '{}'.", app_user);
    println!("  Run 'caddy reload' if needed to pick up any pending Caddyfile changes.");
    Ok(())
}
