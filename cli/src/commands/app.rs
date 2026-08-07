//! Local dev process management — a Rust port of app.sh's start/stop/restart/status/logs
//! commands, for running `synap app <action>` instead of `./app.sh <action>`.
//!
//! Assumes it's run from the project root (same convention as `synap migrate` / `synap dev
//! reset`, which app.sh itself always `cd`s into before invoking). Paths (PID file, log file,
//! binary, search index) are relative to the current directory for that reason.
//!
//! Build/rebuild/test/clean-* commands are intentionally NOT ported here — those are cargo
//! workflows tied to the source tree, not process lifecycle, and stay in app.sh.

use clap::Subcommand;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Subcommand)]
pub enum AppAction {
    /// Build (if needed) and start the server in the background
    Start {
        /// Use the release binary (target/release/synaptic) instead of debug
        #[arg(long)]
        release: bool,
    },
    /// Stop the running server
    Stop,
    /// Stop then start (no rebuild)
    Restart {
        /// Use the release binary (target/release/synaptic) instead of debug
        #[arg(long)]
        release: bool,
    },
    /// Show whether the server is running
    Status,
    /// Tail live server logs (Ctrl+C to exit)
    Logs,
}

/// Verify the current directory actually is the project root before touching
/// any PID/log/binary paths (all resolved relative to cwd below) — running
/// from the wrong directory previously failed silently: it would write/read
/// PID and log files in the wrong place, report "Not running" for an
/// instance that was actually up, and (via `restart`/`free_port`) kill a
/// real running server before discovering the replacement couldn't even
/// find its binary.
fn require_project_root() -> anyhow::Result<()> {
    let looks_right = Path::new("Cargo.toml").is_file()
        && Path::new("core").is_dir()
        && Path::new("cli").is_dir();
    if !looks_right {
        let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
        anyhow::bail!(
            "synap app must be run from the project root (expected to find Cargo.toml, \
             core/, and cli/ in the current directory — got '{cwd}').\n\
             cd to the project root and try again."
        );
    }
    Ok(())
}

fn pid_file() -> PathBuf {
    PathBuf::from(".synaptic.pid")
}

fn log_file() -> PathBuf {
    PathBuf::from("logs/synapcms.log")
}

fn binary_path(release: bool) -> PathBuf {
    if release {
        PathBuf::from("target/release/synaptic")
    } else {
        PathBuf::from("target/debug/synaptic")
    }
}

fn port() -> String {
    std::env::var("PORT").unwrap_or_else(|_| "3000".to_string())
}

fn read_pid() -> Option<u32> {
    fs::read_to_string(pid_file()).ok()?.trim().parse().ok()
}

fn is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Returns the running PID, cleaning up a stale PID file if the process is gone.
fn running_pid() -> Option<u32> {
    let pid = read_pid()?;
    if is_alive(pid) {
        Some(pid)
    } else {
        let _ = fs::remove_file(pid_file());
        None
    }
}

fn free_port() {
    let port = port();
    let in_use = Command::new("fuser")
        .arg(format!("{port}/tcp"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !in_use {
        return;
    }

    println!("Port {port} is in use — clearing...");
    let systemd_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "synapcms"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if systemd_active {
        println!("Stopping systemd synapcms service...");
        let _ = Command::new("systemctl").args(["stop", "synapcms"]).status();
    }
    let _ = Command::new("fuser").args(["-k", &format!("{port}/tcp")]).status();
    std::thread::sleep(Duration::from_secs(1));
}

fn check_caddy() {
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", "caddy"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !active {
        println!("WARNING: Caddy is not running.");
        println!("  Sites will be unreachable on port 80/443 until Caddy is restored.");
        println!("  Fix:   sudo chown -R caddy:caddy /var/log/caddy && sudo systemctl restart caddy");
        println!("  Check: sudo journalctl -u caddy -n 30");
    }
}

async fn check_postgres() -> anyhow::Result<()> {
    if std::env::var("DATABASE_URL").is_err() {
        println!("WARNING: DATABASE_URL not set — skipping PostgreSQL connectivity check.");
        return Ok(());
    }
    super::connect_db()
        .await
        .map_err(|e| anyhow::anyhow!("PostgreSQL is not reachable: {e}\nStart PostgreSQL before starting the server."))?;
    println!("PostgreSQL is reachable.");
    Ok(())
}

async fn cmd_start(release: bool) -> anyhow::Result<()> {
    if let Some(pid) = running_pid() {
        println!("Already running (PID {pid}). Use 'synap app restart' to restart.");
        return Ok(());
    }

    check_postgres().await?;

    let binary = binary_path(release);
    if !binary.exists() {
        println!("Binary not found — building ({})...", if release { "release" } else { "debug" });
        let mut cmd = Command::new("cargo");
        cmd.arg("build").arg("--bin").arg("synaptic");
        if release {
            cmd.arg("--release");
        }
        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("Build failed.");
        }
    }

    fs::create_dir_all("logs")?;
    free_port();

    // Remove any leftover Tantivy lock files from a previous crash.
    if let Ok(entries) = fs::read_dir("search-index") {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("lock") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    check_caddy();
    println!("Starting SynapCMS...");

    let log = fs::OpenOptions::new().create(true).append(true).open(log_file())?;
    let log_err = log.try_clone()?;

    // nohup execs into the target binary in place, so its PID is the actual server PID —
    // matching what `nohup "$BINARY" & ; echo $!` captures in app.sh.
    let mut child = Command::new("nohup")
        .arg(
            binary
                .canonicalize()
                .unwrap_or(binary.clone()),
        )
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()?;

    let pid = child.id();

    std::thread::sleep(Duration::from_secs(2));

    // Check via the owned Child handle (try_wait), not a bare `kill -0 <pid>`
    // re-check — if nohup fails to exec (e.g. binary missing) it exits almost
    // immediately, and on a busy system (e.g. mid `cargo build`) the kernel
    // can recycle that PID for an unrelated process within the sleep window,
    // making a raw PID liveness check falsely report success. try_wait()
    // tracks our exact child via the OS process table entry, immune to that.
    match child.try_wait()? {
        Some(status) => {
            println!("ERROR: Server failed to start (exited: {status}). Check logs:");
            print_tail(&log_file(), 20);
            anyhow::bail!("Server failed to start.");
        }
        None => {
            fs::write(pid_file(), pid.to_string())?;
            std::mem::forget(child); // detach — tracked via the PID file from here on
            println!("Started (PID {pid}) — listening on port {}", port());
            println!("Logs: {}", log_file().display());
        }
    }
    Ok(())
}

fn cmd_stop() -> anyhow::Result<()> {
    let Some(pid) = running_pid() else {
        println!("Not running.");
        let _ = fs::remove_file(pid_file());
        return Ok(());
    };

    println!("Stopping server (PID {pid})...");
    let _ = Command::new("kill").arg(pid.to_string()).status();

    for _ in 0..10 {
        if !is_alive(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    if is_alive(pid) {
        println!("Force killing...");
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }

    let _ = fs::remove_file(pid_file());
    free_port();
    println!("Stopped.");
    Ok(())
}

async fn cmd_restart(release: bool) -> anyhow::Result<()> {
    cmd_stop()?;
    std::thread::sleep(Duration::from_secs(1));
    cmd_start(release).await
}

fn cmd_status() {
    match running_pid() {
        Some(pid) => println!("Running (PID {pid}) on port {}", port()),
        None => println!("Not running."),
    }
    check_caddy();
}

fn cmd_logs() -> anyhow::Result<()> {
    let log = log_file();
    if !log.exists() {
        anyhow::bail!("No log file found at {} — has the server been started yet?", log.display());
    }
    println!("Tailing {} (Ctrl+C to exit)...", log.display());
    Command::new("tail").args(["-f"]).arg(&log).status()?;
    Ok(())
}

fn print_tail(path: &Path, n: usize) {
    if let Ok(content) = fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(n);
        let mut stdout = std::io::stdout();
        for line in &lines[start..] {
            let _ = writeln!(stdout, "{line}");
        }
    }
}

pub async fn run(action: AppAction) -> anyhow::Result<()> {
    require_project_root()?;
    match action {
        AppAction::Start { release } => cmd_start(release).await,
        AppAction::Stop => cmd_stop(),
        AppAction::Restart { release } => cmd_restart(release).await,
        AppAction::Status => {
            cmd_status();
            Ok(())
        }
        AppAction::Logs => cmd_logs(),
    }
}
