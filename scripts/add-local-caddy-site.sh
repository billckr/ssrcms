#!/usr/bin/env bash
# add-local-caddy-site.sh — add a Caddy block for a local/dev site with a
# self-signed (tls internal) cert, for hostnames that only resolve via
# /etc/hosts (loopback), where the admin panel's "Enable SSL" button will
# always refuse (it requires real public DNS for Let's Encrypt).
#
# Usage:
#   ./scripts/add-local-caddy-site.sh <hostname> [port]
#
# Example:
#   ./scripts/add-local-caddy-site.sh beth.com
#   ./scripts/add-local-caddy-site.sh beth.com 3000

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CADDYFILE="/etc/caddy/Caddyfile"

HOSTNAME="${1:-}"
if [[ -z "$HOSTNAME" ]]; then
    echo "Usage: $0 <hostname> [port]" >&2
    exit 1
fi

PORT="${2:-}"
if [[ -z "$PORT" && -f "$SCRIPT_DIR/.env" ]]; then
    PORT=$(grep -E '^PORT=' "$SCRIPT_DIR/.env" 2>/dev/null | cut -d= -f2 | tr -d '[:space:]' || true)
fi
PORT="${PORT:-3000}"

UPLOADS_DIR="$SCRIPT_DIR/uploads"

if sudo grep -qE "^${HOSTNAME} \{|^${HOSTNAME},|^${HOSTNAME}\{" "$CADDYFILE" 2>/dev/null; then
    echo "Caddy already has a block for '${HOSTNAME}' — nothing to do."
    exit 0
fi

echo "Adding Caddy block for '${HOSTNAME}' (proxying to localhost:${PORT})..."

sudo tee -a "$CADDYFILE" > /dev/null <<BLOCK

# >>> SynapCMS managed block: ${HOSTNAME} >>>
${HOSTNAME} {
    tls internal

    @upload_file {
        path_regexp ^/uploads/[^/]+\$
    }
    handle @upload_file {
        uri strip_prefix /uploads
        root * ${UPLOADS_DIR}/${HOSTNAME}
        file_server
    }

    reverse_proxy localhost:${PORT}

    encode zstd gzip

    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "SAMEORIGIN"
        Referrer-Policy "strict-origin-when-cross-origin"
        -Server
    }

    log {
        output file /var/log/caddy/${HOSTNAME}.log
        format json
    }
}
# <<< SynapCMS managed block: ${HOSTNAME} <<<
BLOCK

sudo caddy validate --config "$CADDYFILE"
sudo systemctl reload caddy

echo "Done. Make sure '${HOSTNAME}' is in /etc/hosts pointing at 127.0.0.1, then visit https://${HOSTNAME}"
