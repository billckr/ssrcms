//! Shared Caddyfile block helpers.
//!
//! Used by the admin panel's real-ACME SSL provisioning
//! (`handlers/admin/sites.rs::provision_ssl`/`delete`) and by the CLI
//! installer (`cli/src/commands/install.rs::merge_caddyfile`) for the
//! matching add/remove side. `build_caddy_block` still takes a
//! `tls_internal` flag for self-signed local-dev blocks, but the only
//! callers that used `true` (the `synap caddy provision-local` CLI command
//! and `scripts/add-local-caddy-site.sh`) have been removed.
//!
//! Every block written to the Caddyfile by this app — at install time or at
//! runtime — is wrapped in `# >>> SynapCMS managed block: {hostname} >>>` /
//! `# <<< ... <<<` markers via `wrap_managed_block`, so `strip_caddy_block`
//! can find and remove exactly what was added, regardless of which path
//! added it, without disturbing a hand-written block for the same host.

/// Returns true if the Caddyfile already contains a block for `hostname`.
/// Matches lines where the hostname is the sole token before `{` (bare domain blocks).
pub fn caddy_block_exists(caddyfile: &str, hostname: &str) -> bool {
    caddyfile.lines().any(|line| {
        let t = line.trim();
        t == hostname
            || t.starts_with(&format!("{} ", hostname))
            || t.starts_with(&format!("{},", hostname))
            || t.starts_with(&format!("{}{{", hostname))
    })
}

/// Build the Caddyfile block to append for a site.
///
/// `tls_internal` forces Caddy's local self-signed CA instead of attempting
/// real Let's Encrypt ACME issuance. Only ever pass `true` for a domain that
/// doesn't (yet) resolve to this server — e.g. local dev via `/etc/hosts` —
/// since that's the only case ACME couldn't succeed anyway. The real-ACME
/// caller (`provision_ssl`) always passes `false`.
///
/// `/theme/static/*` deliberately has no theme name in the URL — which
/// theme's files get served is resolved per-request in
/// `handlers/theme_static.rs` (Host header -> site -> active theme). A flat
/// Caddy file_server can't do that resolution, so `/theme/*` must fall
/// through to `reverse_proxy` -> Axum, not be handled here. See
/// deployment/Caddyfile.template for the same rule.
pub fn build_caddy_block(hostname: &str, port: u16, uploads_dir: &str, tls_internal: bool) -> String {
    let tls_line = if tls_internal { "    tls internal\n\n" } else { "" };
    format!(
        r#"{hostname} {{
{tls_line}    # Serve uploads directly — bypass Axum — but ONLY the bare-filename shape
    # (/uploads/{{filename}}, what public pages use via Media::url()). Rooted
    # at THIS site's own uploads/{hostname}/ -> uploads/{{site-uuid}}/ symlink
    # (the app maintains one per site), so a bare filename resolves with no
    # need to repeat the hostname in the path.
    #
    # The admin media UI instead builds UUID-prefixed URLs
    # (/uploads/{{site-uuid}}/{{filename}}), since admin can be browsed via a
    # host that isn't this site's own domain (e.g. a shared dev host).
    # Matching bare filenames only (path_regexp, no further `/` after the
    # filename) means anything with more path segments — including that
    # UUID-prefixed shape — falls through to reverse_proxy -> Axum below,
    # whose handlers/uploads.rs already resolves that shape correctly. A
    # blanket /uploads/* match here would otherwise double up the site's own
    # directory with the UUID segment and 404 every admin-uploaded image.
    # See deployment/Caddyfile.template for the same rule.
    @upload_file {{
        path_regexp ^/uploads/[^/]+$
    }}
    handle @upload_file {{
        uri strip_prefix /uploads
        root * {uploads_dir}/{hostname}
        file_server
    }}

    reverse_proxy localhost:{port}

    encode zstd gzip

    header {{
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "SAMEORIGIN"
        Referrer-Policy "strict-origin-when-cross-origin"
        -Server
    }}

    log {{
        output file /var/log/caddy/{hostname}.log
        format json
    }}
}}"#,
        hostname    = hostname,
        tls_line    = tls_line,
        port        = port,
        uploads_dir = uploads_dir,
    )
}

/// Wrap `block` in the `# >>> SynapCMS managed block: {hostname} >>>` /
/// `# <<< ... <<<` marker comments every writer of the Caddyfile uses, so
/// `strip_caddy_block` can later find and remove exactly this block.
pub fn wrap_managed_block(hostname: &str, block: &str) -> String {
    format!(
        "# >>> SynapCMS managed block: {hostname} >>>\n{}\n# <<< SynapCMS managed block: {hostname} <<<",
        block.trim_end(),
    )
}

/// Remove the marker-delimited block for `hostname` (as wrapped by
/// `wrap_managed_block`) from `caddyfile`, swallowing one trailing newline
/// so repeated add/remove cycles don't accumulate blank lines. Returns
/// `caddyfile` unchanged if no such block is present — safe to call
/// unconditionally, e.g. on site deletion whether or not SSL was ever
/// provisioned for it.
pub fn strip_caddy_block(caddyfile: &str, hostname: &str) -> String {
    let begin = format!("# >>> SynapCMS managed block: {hostname} >>>");
    let end = format!("# <<< SynapCMS managed block: {hostname} <<<");

    let Some(start_idx) = caddyfile.find(&begin) else { return caddyfile.to_string(); };
    let Some(end_rel) = caddyfile[start_idx..].find(&end) else { return caddyfile.to_string(); };
    let end_idx = start_idx + end_rel + end.len();

    let after = caddyfile[end_idx..].strip_prefix('\n').unwrap_or(&caddyfile[end_idx..]);
    format!("{}{}", &caddyfile[..start_idx], after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_block() {
        let caddyfile = "example.com {\n    reverse_proxy localhost:3000\n}\n";
        assert!(caddy_block_exists(caddyfile, "example.com"));
        assert!(!caddy_block_exists(caddyfile, "other.com"));
    }

    #[test]
    fn tls_internal_line_present_only_when_requested() {
        let with_tls = build_caddy_block("beth.com", 3000, "uploads", true);
        assert!(with_tls.contains("tls internal"));

        let without_tls = build_caddy_block("beth.com", 3000, "uploads", false);
        assert!(!without_tls.contains("tls internal"));
    }

    #[test]
    fn wrap_then_strip_round_trips_to_original() {
        let existing = "localhost {\n    reverse_proxy localhost:3000\n}\n";
        let block = build_caddy_block("beth.com", 3000, "uploads", false);
        let wrapped = wrap_managed_block("beth.com", &block);

        let with_block = format!("{}\n{}\n", existing.trim_end(), wrapped);
        assert!(caddy_block_exists(&with_block, "beth.com"));

        let stripped = strip_caddy_block(&with_block, "beth.com");
        assert_eq!(stripped, existing);
    }

    #[test]
    fn strip_is_a_noop_when_no_block_present() {
        let existing = "localhost {\n    reverse_proxy localhost:3000\n}\n";
        assert_eq!(strip_caddy_block(existing, "beth.com"), existing);
    }

    #[test]
    fn strip_only_removes_the_matching_hostname() {
        let block_a = wrap_managed_block("a.com", &build_caddy_block("a.com", 3000, "uploads", false));
        let block_b = wrap_managed_block("b.com", &build_caddy_block("b.com", 3000, "uploads", false));
        let content = format!("{}\n{}\n", block_a, block_b);

        let stripped = strip_caddy_block(&content, "a.com");
        assert!(!caddy_block_exists(&stripped, "a.com"));
        assert!(caddy_block_exists(&stripped, "b.com"));
    }
}
