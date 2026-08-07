//! Rebuilds and reinstalls `synap` itself — the Rust equivalent of app.sh's
//! `update-cli` command (`cargo install --path cli --force`), so CLI source
//! changes can be picked up without going back to app.sh.
//!
//! Must be run from the project root (same convention as `synap migrate` /
//! `synap app <action>`), since `cargo install --path cli` resolves relative
//! to the current directory.

use std::process::Command;

pub fn run() -> anyhow::Result<()> {
    if !std::path::Path::new("cli/Cargo.toml").exists() {
        anyhow::bail!(
            "cli/Cargo.toml not found in the current directory.\n\
             Run this from the project root (same as `synap migrate` / `synap app`)."
        );
    }

    println!("Reinstalling synap...");
    let status = Command::new("cargo")
        .args(["install", "--path", "cli", "--force"])
        .status()?;

    if !status.success() {
        anyhow::bail!("cargo install failed.");
    }

    // `current_exe()` would resolve to the old binary's now-unlinked path (cargo install
    // replaced the file this process is still running from) — `which` reflects the new one.
    let path = Command::new("which")
        .arg("synap")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "synap".to_string());
    println!("synap updated: {path}");
    Ok(())
}
