//! Shared Caddyfile block helpers.
//!
//! Used by both the admin panel's real-ACME SSL provisioning
//! (`handlers/admin/sites.rs::provision_ssl`) and the `synap caddy
//! provision-local` CLI command (self-signed, for domains that don't
//! resolve to this server yet — see that command for why the two paths
//! must never be merged into one).

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
}
