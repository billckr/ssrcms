//! Runtime version detection.
//!
//! A release tarball built by `.github/workflows/release.yml` ships a
//! `VERSION` file (containing the git tag, e.g. `v0.1.0-alpha18`) alongside
//! the binaries. We read that file at startup so the running process knows
//! which release it actually is — `Cargo.toml`'s `version.workspace` field
//! is not useful for this since it never moves between tags.

/// Resolve the current running version.
///
/// Looks for a `VERSION` file next to the running executable. Falls back to
/// `v{CARGO_PKG_VERSION}-source` when absent — true for local `cargo build`
/// installs that didn't come from a release tarball, which can't be
/// meaningfully compared against a tagged release.
pub fn current_version() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("VERSION")))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("v{}-source", env!("CARGO_PKG_VERSION")))
}

/// True when `current_version()` came from `Cargo.toml` rather than a real
/// release `VERSION` file — nothing meaningful to compare against a tag.
pub fn is_source_build(version: &str) -> bool {
    version.ends_with("-source")
}
