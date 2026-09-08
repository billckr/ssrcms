//! POST /admin/self-update — in-app self-updater. Downloads the latest
//! GitHub Release tarball for this host's architecture, checksum-verifies
//! it, swaps in the new `synapcms`/`synap`/`VERSION`, and exits so
//! systemd's `Restart=always` brings the process back up on the new build.
//!
//! Scope is deliberately narrow: only the two binaries and the VERSION
//! file are ever touched. Release tarballs ship `themes/global/` etc, but
//! `themes/sites/`/`plugins/sites/` inside them are always empty (stripped
//! by the release workflow) — naively syncing those directories would
//! delete every site's custom themes/plugins, so this never touches them.
//! Asset updates stay on the existing manual deploy path
//! (docs/deployment-guide.md).
//!
//! Integrity: verified against the release's published SHA256 checksum
//! only, not a cryptographic signature — that proves the download wasn't
//! corrupted/tampered in transit, not that the release itself is
//! legitimate (a compromised publishing pipeline could publish a matching
//! checksum for a malicious build). Documented as a known limitation
//! rather than silently assumed away.
//!
//! Responds with JSON, not a redirect — the admin/src/pages/whats_new.rs
//! frontend drives this via `fetch()` into a progress-bar modal, not a
//! normal form submit, since a plain fetch would otherwise silently follow
//! a redirect and look identical whether the update succeeded or failed.

use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Form, Json};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

const REPO: &str = "billckr/ssrcms";

/// Architectures the release pipeline actually builds — see
/// .github/workflows/release.yml. Checked before constructing a download
/// URL so an unsupported host fails with a clear message instead of a bare
/// 404 from GitHub.
const SUPPORTED_ARCHES: &[&str] = &["x86_64", "aarch64"];

/// Candidate absolute paths for `tar`, checked in order before falling back
/// to a bare `PATH` lookup — same defensiveness as the existing Caddy-reload
/// call (`sites.rs`) using an absolute path rather than trusting `PATH`
/// under the hardened systemd unit, but as a list rather than one hardcoded
/// path since `tar`'s location varies more by distro than Caddy's does.
const TAR_CANDIDATES: &[&str] = &["/usr/bin/tar", "/bin/tar"];

#[derive(Deserialize)]
pub struct SelfUpdateForm {
    pub current_password: String,
}

pub async fn apply(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<SelfUpdateForm>,
) -> impl IntoResponse {
    if !admin.caps.is_global_admin || admin.caps.is_impersonating {
        return Json(json!({"error": "forbidden"})).into_response();
    }
    if !state.config.self_update_enabled {
        return Json(json!({"error": "disabled"})).into_response();
    }
    if !admin.user.verify_password(&form.current_password) {
        return Json(json!({"error": "wrong_password"})).into_response();
    }
    if crate::version::is_source_build(&state.current_version) {
        return Json(json!({"error": "source_build"})).into_response();
    }
    if !SUPPORTED_ARCHES.contains(&std::env::consts::ARCH) {
        tracing::error!(
            "self-update: unsupported architecture {:?} — no matching release asset exists",
            std::env::consts::ARCH
        );
        return Json(json!({"error": "unsupported_arch"})).into_response();
    }

    let tag = {
        let latest = state.latest_release.read().ok();
        match latest.as_ref().and_then(|r| r.as_ref()) {
            Some(release) if release.tag_name != state.current_version => {
                release.tag_name.clone()
            }
            _ => return Json(json!({"error": "up_to_date"})).into_response(),
        }
    };

    // Defense in depth: this is interpolated into a URL below. It already
    // comes from GitHub's own API over HTTPS, not user input, but validate
    // the shape anyway before trusting it.
    let valid_tag = tag.len() <= 64
        && tag.starts_with('v')
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if !valid_tag {
        tracing::error!("self-update: refusing to use malformed tag {:?}", tag);
        return Json(json!({"error": "apply_failed"})).into_response();
    }

    match run_update(&tag).await {
        Ok(()) => {
            crate::handlers::admin::audit(
                &state,
                &admin,
                "apply_update",
                "system",
                None,
                &tag,
                None,
            )
            .await;

            // Return the response first, then exit after a short delay so
            // it actually reaches the browser — systemd's Restart=always
            // brings the process back up running the new binary.
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(750)).await;
                std::process::exit(0);
            });

            Json(json!({"ok": true, "tag": tag})).into_response()
        }
        Err(e) => {
            tracing::error!("self-update: failed to apply {}: {:?}", tag, e);
            Json(json!({"error": "apply_failed"})).into_response()
        }
    }
}

fn find_tar() -> PathBuf {
    TAR_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("tar")) // fall back to a PATH lookup
}

async fn run_update(tag: &str) -> anyhow::Result<()> {
    let install_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("could not determine install directory"))?
        .to_path_buf();

    let arch = std::env::consts::ARCH; // "x86_64" or "aarch64" — matches the release asset naming
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    let tarball_name = format!("synaptic-signals-{tag}-{arch}-linux.tar.gz");
    let tarball_url = format!("{base}/{tarball_name}");
    let checksum_url = format!("{tarball_url}.sha256");

    let client = reqwest::Client::builder()
        .user_agent("SynapCMS-SelfUpdate/1.0")
        .timeout(Duration::from_secs(180))
        .build()?;

    let tarball_bytes = client
        .get(&tarball_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let checksum_text = client
        .get(&checksum_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let expected_hex = checksum_text
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty checksum file"))?
        .to_lowercase();

    let mut hasher = Sha256::new();
    hasher.update(&tarball_bytes);
    let actual_hex = hex_encode(&hasher.finalize());
    if actual_hex != expected_hex {
        anyhow::bail!("checksum mismatch: expected {expected_hex}, got {actual_hex}");
    }

    let staging = install_dir.join(".self-update-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .with_context(|| format!("creating staging dir {:?}", staging))?;

    let tarball_path = staging.join(&tarball_name);
    std::fs::write(&tarball_path, &tarball_bytes)
        .with_context(|| format!("writing downloaded tarball to {:?}", tarball_path))?;

    let tar_bin = find_tar();
    let tar_status = std::process::Command::new(&tar_bin)
        .args(["xzf", tarball_name.as_str()])
        .current_dir(&staging)
        .status()
        .with_context(|| format!("spawning {:?} to extract the release", tar_bin))?;
    if !tar_status.success() {
        anyhow::bail!("tar extraction failed with status {tar_status}");
    }

    let extracted_dir = staging.join(format!("synaptic-signals-{tag}"));
    let new_server_bin = extracted_dir.join("synapcms");
    let new_cli_bin = extracted_dir.join("synap");
    let new_version_file = extracted_dir.join("VERSION");
    for p in [&new_server_bin, &new_cli_bin, &new_version_file] {
        if !p.is_file() {
            anyhow::bail!("expected file missing from release tarball: {:?}", p);
        }
    }

    // Backup the currently-running binaries before touching anything live —
    // cheap safety net for a manual SSH rollback if the new build is broken.
    // Non-fatal: a failed backup shouldn't block an otherwise-good update.
    if let Err(e) = copy_with_exec_bit(&install_dir.join("synapcms"), &install_dir.join("synapcms.bak")) {
        tracing::warn!("self-update: could not back up synapcms: {:?}", e);
    }
    if let Err(e) = copy_with_exec_bit(&install_dir.join("synap"), &install_dir.join("synap.bak")) {
        tracing::warn!("self-update: could not back up synap: {:?}", e);
    }

    // Stage the new files on the same filesystem as the live path, then
    // rename() into place — atomic, no window where the binary is a
    // partially-written file.
    let staged_server = install_dir.join("synapcms.new");
    let staged_cli = install_dir.join("synap.new");
    copy_with_exec_bit(&new_server_bin, &staged_server)
        .with_context(|| format!("staging new synapcms binary at {:?}", staged_server))?;
    copy_with_exec_bit(&new_cli_bin, &staged_cli)
        .with_context(|| format!("staging new synap binary at {:?}", staged_cli))?;
    std::fs::rename(&staged_server, install_dir.join("synapcms"))
        .context("renaming synapcms.new over the live synapcms binary")?;
    std::fs::rename(&staged_cli, install_dir.join("synap"))
        .context("renaming synap.new over the live synap binary")?;

    // Same stage-then-rename as the binaries above, not a direct copy over
    // the live path — rename() only needs write permission on the
    // directory, not on the target file itself. A direct fs::copy() over an
    // existing file needs write permission on that specific file, which
    // broke here the first time this ran: VERSION had been left root-owned
    // by an earlier manual deploy, while the service (and this code) runs
    // as a non-root user that owns the directory but not that file.
    let staged_version = install_dir.join("VERSION.new");
    std::fs::copy(&new_version_file, &staged_version)
        .with_context(|| format!("staging new VERSION file at {:?}", staged_version))?;
    std::fs::rename(&staged_version, install_dir.join("VERSION"))
        .context("renaming VERSION.new over the live VERSION file")?;

    let _ = std::fs::remove_dir_all(&staging);

    Ok(())
}

fn copy_with_exec_bit(from: &PathBuf, to: &PathBuf) -> std::io::Result<()> {
    std::fs::copy(from, to)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(to)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(to, perms)?;
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
