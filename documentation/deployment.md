---
title: Deployment & Build Toolchain
group: system
updated_by: claude
last_updated: 2026-09-08
---
# Deployment & Build Toolchain

> Last updated: 2026-09-08 | Updated by: claude

## Overview

VPS deploy binaries (`synapcms`, `synap`) are built against `x86_64-unknown-linux-musl` instead of the host's native glibc target, producing a fully static binary with zero dynamic dependencies — see `scripts/install-vps.sh`'s `do_build()`. This avoids a binary built on a newer-glibc dev machine being unrunnable on an older-glibc VPS (glibc only guarantees forward compatibility). The tradeoff is that the local dev machine needs a musl C toolchain (`musl-gcc`) in addition to the `rustup` musl target, and that toolchain's quirks can produce binaries that build cleanly but crash at runtime.

## Issue: segfault in `_start_c` on first run (AlmaLinux dev machine)

AlmaLinux 9 (and RHEL-family distros generally) have no `musl-tools`/`musl-gcc` package via dnf or EPEL — that package name is Debian/Ubuntu-only. To get `musl-gcc` there, musl libc 1.2.5 was built from source (`./configure --prefix=/usr/local/musl && make && make install`) and symlinked onto `PATH`.

Binaries linked with this from-source `musl-gcc` built and ran the version-print path locally in some quick checks, but segfaulted immediately (before printing anything) both locally and on the actual VPS (178.156.176.60) once actually invoked in the real install flow (`sudo -u www-data ./synap install ...` failed with `Segmentation fault`, exit 139).

**Root cause:** rustc's musl target defaults to a PIE relocation model, which links in the self-relocating `rcrt1.o` crt startup object (from rustc's own bundled musl sysroot at `~/.rustup/toolchains/.../lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/`). This object expects to run as a real PIE (`ET_DYN`) binary and self-relocate at load time using an auxv-provided base address. But the from-source `musl-gcc`'s generated specs file doesn't pass `-pie`/`-static-pie` to the linker, so the actual output was a plain non-PIE `ET_EXEC` binary (`file` reported "statically linked", not "shared object"). The crt's self-relocation code then ran against a binary layout it didn't match, segfaulting in `_start_c` on the very first instructions before `main` is ever reached.

**Fix:** force rustc to use the plain (non-PIE) `crt1.o` instead, so the crt object and the actual link output agree. Added to `.cargo/config.toml` (checked into the repo root, applies to any machine building this project):

```toml
[target.x86_64-unknown-linux-musl]
linker = "musl-gcc"
rustflags = ["-C", "relocation-model=static"]
```

This is checked into the repo (not a per-machine `~/.cargo/config.toml`) specifically so both dev machines (this AlmaLinux box and a separate Arch Linux machine) build identically without needing to remember which one needed which workaround. `relocation-model=static` is the standard, most-compatible static-linking mode — safe on any musl toolchain, including a correctly-configured one (e.g. Arch's `musl` package, which wasn't actually hitting this bug) — so bringing it to a machine that didn't need it causes no regression.

## Troubleshooting: musl-target binary crashes/segfaults immediately

1. Check the binary's link type: `file target/x86_64-unknown-linux-musl/release/<bin>` should say "statically linked" (`ET_EXEC`), not "shared object".
2. Try running it directly: `./target/x86_64-unknown-linux-musl/release/<bin> --version`. A crash before any output at all (not a normal panic/error) points at crt/startup, not application code.
3. Confirm with a backtrace: `gdb -q --batch -ex run -ex bt --args ./<bin> --version`. A crash frame `#0` inside `_start_c` (rather than anywhere in Rust code) confirms the PIE/`ET_EXEC` crt mismatch described above.
4. Verify `.cargo/config.toml`'s `[target.x86_64-unknown-linux-musl]` `rustflags` is actually in effect — a closer `.cargo/config.toml` further down a directory tree, or a stray per-machine `~/.cargo/config.toml`, can override it (arrays like `rustflags` don't merge across Cargo config files; the closest one to the build directory wins outright for that key).
5. Verbose-link if still unclear: `RUSTFLAGS="-C link-arg=-v" cargo build --release --target x86_64-unknown-linux-musl --bin <bin>` and inspect the `collect2`/`ld` invocation for which crt object (`crt1.o` vs `rcrt1.o`) actually got linked, and whether `-static`/`-pie`/`-static-pie` appear.

## Deployment target compatibility

Because the shipped binary is a fully static musl build, it has no libc/glibc-version dependency at all and will run on essentially any Linux distro on the same CPU architecture — Ubuntu, Debian, RHEL/AlmaLinux/Rocky, Fedora, Arch, even musl-native distros like Alpine.

Two things are **not** covered by that static-binary compatibility and would currently block a deploy regardless of distro flavor:

- **Architecture**: only `x86_64-unknown-linux-musl` is built. An ARM-based VPS (AWS Graviton, Oracle Cloud's ARM tier, etc.) cannot run this binary — would need an `aarch64-unknown-linux-musl` cross-build added to the toolchain and `install-vps.sh`.
- **No systemd**: `scripts/install-vps.sh`'s requirements gate hard-requires `systemctl` on the remote host (checks `command -v systemctl`, sets up a `.service` unit). Non-systemd distros (Alpine/OpenRC, Void Linux) would fail that check before any build/install step runs, even though the binary itself would run fine there. The script also requires Caddy already installed and reachable, and PostgreSQL 13+ with passwordless sudo for the `postgres` and app-service users — provisioning steps, not distro-compatibility issues, but worth having ready before targeting a new host.

## Release Pipeline (GitHub Actions)

A separate build path from everything above: `.github/workflows/release.yml` builds and publishes versioned, downloadable releases, independent of `install-vps.sh`'s local-rebuild-and-scp deploy flow. Triggered on push of a `v[0-9]*` tag (e.g. `v0.1.0-alpha18`).

- **Two build jobs**: `build-x86_64` runs `cargo build --release --target x86_64-unknown-linux-musl --bin synapcms --bin synap` directly on the `ubuntu-22.04` runner (with `musl-tools` installed via apt for `musl-gcc`); `build-aarch64` cross-compiles the same two binaries for `aarch64-unknown-linux-musl` via the `cross` tool (`continue-on-error: true` — a broken cross-build doesn't block the x86_64 release). Both use the same static-musl approach as `install-vps.sh`'s own build (see Overview above) — picks up the `.cargo/config.toml` `relocation-model=static` fix automatically since that's a repo-root config, not a per-machine one.
- **Fixed 2026-09-08**: this job previously ran `--bin synaptic --bin synaptic-cli` and produced release-glibc, not musl, binaries — both now corrected. The bin-name mismatch was a real latent break (the actual `[[bin]]` names in `core/Cargo.toml`/`cli/Cargo.toml` are `synapcms`/`synap`; they were apparently renamed from `synaptic`/`synaptic-cli` sometime after `v0.1.0-alpha17` was tagged without updating this workflow) — confirmed by downloading alpha17's actual release tarball, which does contain files named `synaptic`/`synaptic-cli`. Caught by actually running `cargo build --release --target x86_64-unknown-linux-musl --bin synapcms --bin synap` locally before trusting the YAML edit; would otherwise have failed outright on the next tag push. The glibc-vs-musl gap noted in earlier versions of this doc is now closed — verified locally that the resulting binaries are correctly statically-linked (`file` reports "statically linked", not "shared object") and run without the PIE/crt segfault described above.
- **Tarball contents**: each job assembles `synaptic-signals-<tag>-<arch>-linux.tar.gz` containing the two binaries, `themes/`, `plugins/`, `admin/static/`, and a `VERSION` file (just the tag name as plain text) — plus a `.sha256` checksum file. Dev/test artifacts (`themes/sites`, `plugins/sites`) are stripped before packaging.
- **Publishing**: `create-release` waits on both build jobs, downloads and merges both architectures' artifacts (`actions/download-artifact@v4` with `pattern: release-*`, `merge-multiple: true`), then publishes a GitHub Release via `softprops/action-gh-release@v2` with `generate_release_notes: true` (GitHub's auto-generated notes — a bulleted PR list plus a compare link — serve as the changelog; currently sparse since this repo pushes straight to `master` without PRs). **Fixed 2026-09-08**: `create-release` previously only depended on and downloaded `build-x86_64`'s artifact, so the aarch64 tarball was built but never actually attached to any release.

## Version Detection & Update Notice

The running app doesn't get its version from `Cargo.toml` — `[workspace.package] version` there is permanently `"0.1.0"` and never bumped; the git tag is the real version identity. Instead:

- `core/src/version.rs::current_version()` reads a `VERSION` file next to the running executable (the one the release tarball ships, see above) at startup. Falls back to `v{CARGO_PKG_VERSION}-source` when absent — true for a local `cargo build` that didn't come from a release tarball, which has nothing meaningful to compare against a tag.
- `core/src/scheduler.rs::spawn_release_check` is a background task (same `tokio::spawn` + `interval` pattern as the other scheduler tasks) that hits `GET https://api.github.com/repos/billckr/ssrcms/releases/latest` every 6 hours (first check fires immediately on startup) and caches the result (`tag_name`, `html_url`, `body`) in `AppState.latest_release`. Gated by `AppConfig.update_check_enabled` (`UPDATE_CHECK_ENABLED` env var, default `true`) for installs with restricted outbound network access. Comparison against `current_version` is a plain string inequality, not semver-aware — a deliberate simplification since this repo only ever tags forward.
- The admin dashboard (super-admin only) shows a persistent "update available" notice when `latest_release.tag_name != current_version`. It uses its own `.update-notice` CSS class rather than the shared `.flash` class — `.flash` elements get auto-faded out 5s after page load by a script in `admin_page` (`admin/src/lib.rs`), which is correct for one-shot save/error messages but was wrong here (this notice needs to persist across page loads until the admin actually updates).
- The notice links to `/admin/whats-new` (`admin/src/pages/whats_new.rs` + `core/src/handlers/admin/whats_new.rs`) rather than straight to GitHub — most self-hosted-CMS admins won't know or care what GitHub is. That page renders the latest release's notes in-app via `pulldown-cmark` (same markdown renderer the Documentation page above uses); GitHub links are kept secondary/opt-in on that page instead of being the primary way to see what changed.
- Applying an update is still a manual step — `docs/deployment-guide.md`'s "Updating" section now downloads and checksum-verifies the tagged release tarball and copies its `VERSION` file into place (so the newly-started process picks up the new version string), instead of the old `cargo build --release` from source.

