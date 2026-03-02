//! CLI commands for managing Caddy file-write permissions and sudoers entries.
//!
//! Usage:
//!   synaptic-cli caddy setup   --app-user <user> [--caddyfile <path>]
//!   synaptic-cli caddy teardown --app-user <user> [--caddyfile <path>]

use clap::Subcommand;
use std::process::Command;

const SUDOERS_FILE: &str = "/etc/sudoers.d/synaptic-caddy";
const SUDOERS_COMMENT: &str = "# Synaptic Signals — allow app user to reload Caddy without a password";

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
}

pub fn run(action: CaddyAction) -> anyhow::Result<()> {
    match action {
        CaddyAction::Setup { app_user, caddyfile } => setup(&app_user, &caddyfile),
        CaddyAction::Teardown { app_user, caddyfile } => teardown(&app_user, &caddyfile),
    }
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

    // 4. Write the sudoers drop-in that allows `sudo caddy reload --config … --adapter caddyfile`
    //    without a password.  The command in the entry must match exactly what the app calls.
    let caddy_reload_cmd = format!(
        "/usr/bin/caddy reload --config {} --adapter caddyfile",
        caddyfile_path
    );
    let sudoers_content = format!(
        "{comment}\n{user} ALL=(root) NOPASSWD: {cmd}\n",
        comment = SUDOERS_COMMENT,
        user    = app_user,
        cmd     = caddy_reload_cmd,
    );
    println!("  Writing {}...", SUDOERS_FILE);
    std::fs::write(SUDOERS_FILE, &sudoers_content)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {e}\nIs this running as root?", SUDOERS_FILE))?;

    // sudoers.d files must be mode 0440 or sudo ignores them.
    let status = Command::new("chmod")
        .args(["0440", SUDOERS_FILE])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to chmod sudoers file: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(SUDOERS_FILE);
        anyhow::bail!("chmod 0440 {} failed — file removed", SUDOERS_FILE);
    }

    // Validate the file before leaving it in place.
    let status = Command::new("visudo")
        .args(["-c", "-f", SUDOERS_FILE])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run visudo: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(SUDOERS_FILE);
        anyhow::bail!("visudo syntax check failed — sudoers file removed to avoid breaking sudo");
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
