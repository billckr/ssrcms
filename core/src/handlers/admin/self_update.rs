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

use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

use admin::html_escape;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

const REPO: &str = "billckr/ssrcms";

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
        return Redirect::to("/admin/whats-new?error=forbidden").into_response();
    }
    if !state.config.self_update_enabled {
        return Redirect::to("/admin/whats-new?error=disabled").into_response();
    }
    if !admin.user.verify_password(&form.current_password) {
        return Redirect::to("/admin/whats-new?error=wrong_password").into_response();
    }
    if crate::version::is_source_build(&state.current_version) {
        return Redirect::to("/admin/whats-new?error=source_build").into_response();
    }

    let tag = {
        let latest = state.latest_release.read().ok();
        match latest.as_ref().and_then(|r| r.as_ref()) {
            Some(release) if release.tag_name != state.current_version => {
                release.tag_name.clone()
            }
            _ => return Redirect::to("/admin/whats-new?error=up_to_date").into_response(),
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
        return Redirect::to("/admin/whats-new?error=apply_failed").into_response();
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

            Html(format!(
                r#"<!doctype html><html><head><meta http-equiv="refresh" content="8;url=/admin/whats-new">
<title>Updating&hellip;</title></head>
<body style="font-family:system-ui;padding:3rem;text-align:center;color:#1e293b">
<h1>Applying {tag}&hellip;</h1>
<p>The server is restarting now. This page will reload automatically in a few seconds.</p>
</body></html>"#,
                tag = html_escape(&tag)
            ))
            .into_response()
        }
        Err(e) => {
            tracing::error!("self-update: failed to apply {}: {:?}", tag, e);
            Redirect::to("/admin/whats-new?error=apply_failed").into_response()
        }
    }
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
    std::fs::create_dir_all(&staging)?;

    let tarball_path = staging.join(&tarball_name);
    std::fs::write(&tarball_path, &tarball_bytes)?;

    let tar_status = std::process::Command::new("tar")
        .args(["xzf", tarball_name.as_str()])
        .current_dir(&staging)
        .status()?;
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
    copy_with_exec_bit(&install_dir.join("synapcms"), &install_dir.join("synapcms.bak")).ok();
    copy_with_exec_bit(&install_dir.join("synap"), &install_dir.join("synap.bak")).ok();

    // Stage the new files on the same filesystem as the live path, then
    // rename() into place — atomic, no window where the binary is a
    // partially-written file.
    let staged_server = install_dir.join("synapcms.new");
    let staged_cli = install_dir.join("synap.new");
    copy_with_exec_bit(&new_server_bin, &staged_server)?;
    copy_with_exec_bit(&new_cli_bin, &staged_cli)?;
    std::fs::rename(&staged_server, install_dir.join("synapcms"))?;
    std::fs::rename(&staged_cli, install_dir.join("synap"))?;
    std::fs::copy(&new_version_file, install_dir.join("VERSION"))?;

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
