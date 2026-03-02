#!/usr/bin/env bash
# Synaptic Signals — one-liner installer
#
# Usage:
#   curl -sSL https://get.synaptic.rs | bash
#
# Environment variable overrides (all optional except where noted):
#   SYNAPTIC_VERSION   — release tag to install (default: latest)
#   INSTALL_DIR        — where to install (default: /opt/synaptic-signals)
#   PORT               — port Axum listens on (default: 3000)
#   SYNAPTIC_USER      — OS user to run the service (default: www-data)
#   SYNAPTIC_DOMAIN    — domain name (prompted if not set)
#   ADMIN_EMAIL        — admin login email (prompted if not set)
#   ADMIN_USERNAME     — admin username (default: admin)
#   ADMIN_PASSWORD     — admin password (generated if not set)
#   APP_NAME           — admin panel brand name (default: Synaptic Signals)
#   NOTIFICATION_EMAIL — reply-to for system emails (default: ADMIN_EMAIL)
#
# Fully non-interactive example:
#   SYNAPTIC_DOMAIN=example.com ADMIN_EMAIL=me@example.com \
#     curl -sSL https://get.synaptic.rs | bash

set -euo pipefail

# ── Help ───────────────────────────────────────────────────────────────────────
usage() {
  cat <<EOF

Synaptic Signals — Installer

USAGE
  sudo bash install.sh [--help]
  curl -sSL https://get.synaptic.rs | sudo bash

  Must be run as root. The app itself runs as a dedicated non-root user.

ENVIRONMENT VARIABLES
  All variables are optional unless marked required.

  SYNAPTIC_DOMAIN      Domain name for the site, e.g. example.com          [required if non-interactive]
  ADMIN_EMAIL          Admin login email address                            [required if non-interactive]
  ADMIN_USERNAME       Admin username                                       (default: admin)
  ADMIN_PASSWORD       Admin password — generated and printed if not set    (default: auto-generated)
  APP_NAME             Admin panel brand name                               (default: Synaptic Signals)
  NOTIFICATION_EMAIL   Reply-to address for system emails                   (default: ADMIN_EMAIL)

  SYNAPTIC_USER        OS user to run the service — must not be root        (default: www-data)
  INSTALL_DIR          Installation directory                               (default: <current-dir>/synaptic-signals)
  PORT                 Port the app listens on                              (default: 3000)
  SYNAPTIC_VERSION     Release tag to install, e.g. v0.1.0-alpha11         (default: latest)

  SKIP_CADDY           Set to any value to skip Caddy setup entirely
  GITHUB_TOKEN         GitHub personal access token (required for private repos)
  DB_NAME              PostgreSQL database name                             (default: synaptic_signals)
  DB_USER              PostgreSQL role name                                 (default: synaptic)

EXAMPLES
  # Interactive install (prompts for domain and admin email):
  sudo bash install.sh

  # Fully non-interactive:
  SYNAPTIC_DOMAIN=example.com ADMIN_EMAIL=me@example.com sudo bash install.sh

  # Custom user and install dir:
  SYNAPTIC_USER=bill INSTALL_DIR=/home/bill/synaptic sudo bash install.sh

  # Skip Caddy (configure manually later):
  SKIP_CADDY=1 SYNAPTIC_DOMAIN=example.com ADMIN_EMAIL=me@example.com sudo bash install.sh

POST-INSTALL
  SSL for additional sites can be provisioned from the admin panel at:
    https://<domain>/admin/sites
  This requires the Caddy permissions set up by the installer. To re-run:
    sudo synaptic-cli caddy setup --app-user <SYNAPTIC_USER>

  To uninstall:
    systemctl disable --now synaptic-signals caddy
    rm -rf \$INSTALL_DIR /etc/caddy/Caddyfile /etc/systemd/system/synaptic-signals.service
    sudo -u postgres psql -c "DROP DATABASE synaptic_signals; DROP ROLE synaptic;"

EOF
}

for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    *) die "Unknown argument: $arg\nRun 'bash install.sh --help' for usage." ;;
  esac
done

# ── Configuration ──────────────────────────────────────────────────────────────
SYNAPTIC_VERSION="${SYNAPTIC_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-${PWD}/synaptic-signals}"
PORT="${PORT:-3000}"
SYNAPTIC_USER="${SYNAPTIC_USER:-www-data}"
GITHUB_REPO="billckr/ssrcms"

# Colours
RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'
BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${BOLD}[install]${RESET} $*"; }
success() { echo -e "${GREEN}[install]${RESET} $*"; }
warn()    { echo -e "${YELLOW}[install] WARNING:${RESET} $*"; }
die()     { echo -e "${RED}[install] ERROR:${RESET} $*" >&2; exit 1; }

# ── Root check ─────────────────────────────────────────────────────────────────
# This script must run as root to install system packages, create users, and
# configure Caddy/systemd. The app itself will run as a dedicated non-root user.
if [[ $EUID -ne 0 ]]; then
  die "This script must be run as root (e.g. sudo bash install.sh)"
fi

# ── Service user check ─────────────────────────────────────────────────────────
# The app must not run as root. Catch the default or explicit SYNAPTIC_USER=root.
if [[ "${SYNAPTIC_USER:-www-data}" == "root" ]]; then
  die "SYNAPTIC_USER cannot be 'root'. Set SYNAPTIC_USER to a dedicated system user, e.g.:\n  SYNAPTIC_USER=synaptic bash install.sh"
fi

# ── OS + architecture detection ────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)          ARCH_SLUG="x86_64" ;;
  aarch64|arm64)   ARCH_SLUG="aarch64" ;;
  *) die "Unsupported architecture: $ARCH (only x86_64 and aarch64 are supported)" ;;
esac

if [[ ! -f /etc/os-release ]]; then
  die "Cannot detect OS (/etc/os-release not found)"
fi
# shellcheck source=/dev/null
. /etc/os-release
OS_ID="${ID}"
OS_ID_LIKE="${ID_LIKE:-}"

is_debian_like() {
  [[ "$OS_ID" == "debian" || "$OS_ID" == "ubuntu" ]] || \
    echo "$OS_ID_LIKE" | grep -qE '(debian|ubuntu)'
}

is_rhel_like() {
  [[ "$OS_ID" == "rhel"   || "$OS_ID" == "fedora"    || \
     "$OS_ID" == "centos" || "$OS_ID" == "rocky"      || \
     "$OS_ID" == "almalinux" ]] || \
    echo "$OS_ID_LIKE" | grep -qE '(rhel|fedora)'
}

if ! is_debian_like && ! is_rhel_like; then
  die "Unsupported OS: $OS_ID. Supported: Debian, Ubuntu, RHEL, Fedora, CentOS, Rocky, AlmaLinux"
fi

info "Detected OS: $PRETTY_NAME | Arch: $ARCH_SLUG"

# ── Service user ───────────────────────────────────────────────────────────────
# On RHEL-family systems www-data may not exist; create a dedicated system user.
if ! id "$SYNAPTIC_USER" &>/dev/null; then
  info "Creating system user '$SYNAPTIC_USER'..."
  useradd --system --no-create-home --shell /sbin/nologin "$SYNAPTIC_USER"
fi

# ── Install dir permissions ────────────────────────────────────────────────────
# Ensure the install dir exists and is owned by the service user.
# Use 750 (rwxr-x---) rather than 755 — Caddy is added to the service group
# so it can read/serve static files without world-readable permissions.
mkdir -p "$INSTALL_DIR"
chown "${SYNAPTIC_USER}:${SYNAPTIC_USER}" "$INSTALL_DIR"
chmod 750 "$INSTALL_DIR"

# Add caddy to the service user's group so it can traverse the install dir
# and serve /uploads/* and /theme/* directly.
if id caddy &>/dev/null; then
  usermod -aG "$SYNAPTIC_USER" caddy
  info "Added 'caddy' to group '${SYNAPTIC_USER}' for static file serving."
fi

# ── Collect install configuration ─────────────────────────────────────────────
echo ""
info "── Installation Configuration ────────────────────────────"

prompt_field() {
  local var_name="$1" prompt_text="$2" default_val="${3:-}"
  local current_val="${!var_name:-}"
  if [[ -z "$current_val" ]]; then
    if [[ -n "$default_val" ]]; then
      read -rp "$(echo -e "${BOLD}${prompt_text}${RESET} [${default_val}]: ")" current_val
      current_val="${current_val:-$default_val}"
    else
      read -rp "$(echo -e "${BOLD}${prompt_text}${RESET}: ")" current_val
    fi
  fi
  printf -v "$var_name" '%s' "$current_val"
}

SYNAPTIC_DOMAIN="${SYNAPTIC_DOMAIN:-}"
ADMIN_EMAIL="${ADMIN_EMAIL:-}"
ADMIN_USERNAME="${ADMIN_USERNAME:-}"
APP_NAME="${APP_NAME:-}"

prompt_field SYNAPTIC_DOMAIN "Domain name (e.g. example.com)"
[[ -n "$SYNAPTIC_DOMAIN" ]] || die "Domain name is required."

prompt_field ADMIN_EMAIL "Admin email address"
[[ -n "$ADMIN_EMAIL" ]] || die "Admin email is required."

prompt_field ADMIN_USERNAME "Admin username" "admin"
prompt_field APP_NAME "Site/app name" "Synaptic Signals"

export SYNAPTIC_DOMAIN ADMIN_EMAIL ADMIN_USERNAME APP_NAME
export NOTIFICATION_EMAIL="${NOTIFICATION_EMAIL:-$ADMIN_EMAIL}"

# ── PostgreSQL ─────────────────────────────────────────────────────────────────
echo ""
info "── PostgreSQL ────────────────────────────────────────────"

install_postgres_debian() {
  info "Installing PostgreSQL 16 (PGDG)..."
  apt-get update -qq
  apt-get install -y -qq curl ca-certificates gnupg lsb-release

  curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
    | gpg --dearmor -o /usr/share/keyrings/postgresql.gpg

  local distro
  distro=$(lsb_release -cs)
  echo "deb [signed-by=/usr/share/keyrings/postgresql.gpg] \
https://apt.postgresql.org/pub/repos/apt ${distro}-pgdg main" \
    > /etc/apt/sources.list.d/pgdg.list

  apt-get update -qq
  apt-get install -y -qq postgresql-16
  systemctl enable --now postgresql
}

install_postgres_rhel() {
  info "Installing PostgreSQL 16 (PGDG)..."
  local rhel_ver
  rhel_ver=$(rpm -E %{rhel})
  dnf install -y -q \
    "https://download.postgresql.org/pub/repos/yum/reporpms/EL-${rhel_ver}-x86_64/pgdg-redhat-repo-latest.noarch.rpm" \
    || true
  dnf -qy module disable postgresql 2>/dev/null || true
  dnf install -y -q postgresql16-server
  /usr/pgsql-16/bin/postgresql-16-setup initdb
  systemctl enable --now postgresql-16
}

if command -v psql &>/dev/null; then
  info "PostgreSQL already installed — skipping."
else
  if is_debian_like; then install_postgres_debian
  else                    install_postgres_rhel
  fi
fi

# ── Create database and user ───────────────────────────────────────────────────
DB_NAME="${DB_NAME:-synaptic_signals}"
DB_USER="${DB_USER:-synaptic}"

# Reuse existing credentials if .env already has them (idempotent re-run).
ENV_FILE="${INSTALL_DIR}/.env"
if [[ -f "$ENV_FILE" ]] && grep -q "^DATABASE_URL=" "$ENV_FILE"; then
  info "Existing .env found — reusing DATABASE_URL."
  DATABASE_URL=$(grep "^DATABASE_URL=" "$ENV_FILE" | cut -d= -f2-)
  # Extract the password from the existing DATABASE_URL so we can keep Postgres in sync.
  DB_PASS=$(echo "$DATABASE_URL" | sed 's|.*://[^:]*:\([^@]*\)@.*|\1|')
else
  DB_PASS=$(openssl rand -hex 16)
  DATABASE_URL="postgres://${DB_USER}:${DB_PASS}@localhost:5432/${DB_NAME}"
fi

# Always ensure the Postgres role exists and its password matches DATABASE_URL.
info "Syncing PostgreSQL user '${DB_USER}'..."
sudo -u postgres psql <<SQL
DO \$\$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '${DB_USER}') THEN
    EXECUTE format('CREATE ROLE ${DB_USER} LOGIN PASSWORD %L', '${DB_PASS}');
  ELSE
    EXECUTE format('ALTER ROLE ${DB_USER} WITH LOGIN PASSWORD %L', '${DB_PASS}');
  END IF;
END \$\$;
SELECT 'CREATE DATABASE ${DB_NAME} OWNER ${DB_USER}'
  WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '${DB_NAME}') \gexec
GRANT ALL PRIVILEGES ON DATABASE ${DB_NAME} TO ${DB_USER};
SQL
success "Database ready."

export DATABASE_URL

# ── Caddy ──────────────────────────────────────────────────────────────────────
echo ""
info "── Caddy ─────────────────────────────────────────────────"

install_caddy_debian() {
  info "Installing Caddy (official repo)..."
  apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | tee /etc/apt/sources.list.d/caddy-stable.list > /dev/null
  apt-get update -qq
  apt-get install -y -qq caddy
}

install_caddy_rhel() {
  info "Installing Caddy (COPR)..."
  dnf install -y -q 'dnf-command(copr)'
  dnf copr enable -y @caddy/caddy
  dnf install -y -q caddy
}

if command -v caddy &>/dev/null; then
  info "Caddy already installed — skipping."
else
  if is_debian_like; then install_caddy_debian
  else                    install_caddy_rhel
  fi
fi

# Warn if port 80 is already occupied (Let's Encrypt HTTP-01 challenge needs it).
if ss -tlnp 2>/dev/null | grep -q ':80 '; then
  warn "Port 80 is in use by another process."
  warn "Caddy needs port 80 for Let's Encrypt. Stop any web servers on port 80 first."
fi

# ── Download release tarball ───────────────────────────────────────────────────
echo ""
info "── Synaptic Signals ──────────────────────────────────────"

if [[ "$SYNAPTIC_VERSION" == "latest" ]]; then
  info "Fetching latest release version..."
  # Pure-bash JSON tag extractor — no external tools required.
  # Strips to the first "tag_name" value; works for both minified and pretty-printed JSON.
  _extract_tag() {
    local s="$1"
    s="${s#*\"tag_name\"}"   # strip everything before "tag_name"
    s="${s#*\"}"             # strip to the opening quote of the value
    s="${s%%\"*}"            # strip from the closing quote onwards
    [[ "$s" =~ ^v[0-9] ]] && printf '%s' "$s"
  }
  for _url in \
    "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases?per_page=10"; do
    _json=$(curl -sSL \
      ${GITHUB_TOKEN:+-H "Authorization: Bearer ${GITHUB_TOKEN}"} \
      "$_url" 2>/dev/null) || true
    SYNAPTIC_VERSION=$(_extract_tag "$_json") || true
    [[ -n "$SYNAPTIC_VERSION" ]] && break
  done
  if [[ -z "$SYNAPTIC_VERSION" ]]; then
    warn "Could not auto-detect latest version (GitHub API may require authentication for this repo)."
    read -rp "$(echo -e "${BOLD}Enter version to install (e.g. v0.1.0-alpha13):${RESET} ")" SYNAPTIC_VERSION
    [[ -n "$SYNAPTIC_VERSION" ]] || die "Version is required."
  fi
fi

info "Installing Synaptic Signals ${SYNAPTIC_VERSION}..."

TARBALL="synaptic-signals-${SYNAPTIC_VERSION}-${ARCH_SLUG}-linux.tar.gz"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

info "Downloading ${TARBALL}..."

DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/${SYNAPTIC_VERSION}/${TARBALL}"

# Try direct download first (works for public repos).
# Fall back to GitHub API asset download if GITHUB_TOKEN is set (private repos).
if curl -fsSL --progress-bar "$DOWNLOAD_URL" -o "${TMP_DIR}/${TARBALL}" 2>/dev/null; then
  if curl -fsSL "${DOWNLOAD_URL}.sha256" -o "${TMP_DIR}/${TARBALL}.sha256" 2>/dev/null; then
    (cd "$TMP_DIR" && sha256sum -c "${TARBALL}.sha256") \
      || die "Checksum verification failed — download may be corrupt."
    success "Checksum verified."
  fi
elif [[ -n "${GITHUB_TOKEN:-}" ]]; then
  info "Direct download failed; trying GitHub API (private repo)..."

  RELEASE_JSON=$(curl -fsSL \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases/tags/${SYNAPTIC_VERSION}") \
    || die "Could not fetch release metadata for ${SYNAPTIC_VERSION}."

  # Use python3 for reliable JSON parsing (grep on minified JSON is fragile).
  ASSET_ID=$(python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
for a in data.get('assets', []):
    if a['name'] == '${TARBALL}':
        print(a['id'])
        break
" <<< "$RELEASE_JSON")
  [[ -n "$ASSET_ID" ]] || die "Asset '${TARBALL}' not found in release ${SYNAPTIC_VERSION}."

  curl -fsSL --progress-bar \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "Accept: application/octet-stream" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases/assets/${ASSET_ID}" \
    -o "${TMP_DIR}/${TARBALL}" \
    || die "Download failed for asset ID ${ASSET_ID}."

  # Checksum (also via API).
  SHA_ASSET_ID=$(python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
for a in data.get('assets', []):
    if a['name'] == '${TARBALL}.sha256':
        print(a['id'])
        break
" <<< "$RELEASE_JSON")
  if [[ -n "$SHA_ASSET_ID" ]]; then
    curl -fsSL \
      -H "Authorization: Bearer ${GITHUB_TOKEN}" \
      -H "Accept: application/octet-stream" \
      "https://api.github.com/repos/${GITHUB_REPO}/releases/assets/${SHA_ASSET_ID}" \
      -o "${TMP_DIR}/${TARBALL}.sha256" 2>/dev/null \
      && (cd "$TMP_DIR" && sha256sum -c "${TARBALL}.sha256") \
      && success "Checksum verified." \
      || true
  fi
else
  die "Download failed. Check that release ${SYNAPTIC_VERSION} exists and is public, or set GITHUB_TOKEN for private repos."
fi

mkdir -p "$INSTALL_DIR"
tar xzf "${TMP_DIR}/${TARBALL}" -C "$INSTALL_DIR" --strip-components=1
success "Extracted to ${INSTALL_DIR}."

# Create directories not included in the tarball.
mkdir -p "${INSTALL_DIR}/uploads"
mkdir -p "${INSTALL_DIR}/search-index"
mkdir -p "${INSTALL_DIR}/themes/sites"
mkdir -p "${INSTALL_DIR}/plugins/sites"

# The app serves admin UI static files from admin/static relative to its working directory.
# The tarball places them at admin/static — ensure the path exists.
if [[ -d "${INSTALL_DIR}/admin/static" ]]; then
  info "Admin static assets found."
else
  warn "Admin static assets not found at ${INSTALL_DIR}/admin/static — icons and editor may be missing."
fi

chmod +x "${INSTALL_DIR}/synaptic" "${INSTALL_DIR}/synaptic-cli"

# ── SELinux context (RHEL/AlmaLinux) ──────────────────────────────────────────
# /var/www and similar paths have httpd_sys_content_t which blocks non-httpd
# services. Relabel the install dir so systemd can run the service from there.
if is_rhel_like && command -v chcon &>/dev/null; then
  chcon -Rt var_t "$INSTALL_DIR" 2>/dev/null || true
  chcon -t bin_t "${INSTALL_DIR}/synaptic" "${INSTALL_DIR}/synaptic-cli" 2>/dev/null || true
  info "SELinux context set for ${INSTALL_DIR}."
fi

# ── Write .env ─────────────────────────────────────────────────────────────────
# Write a fresh .env only on first install. On re-runs, preserve the existing file.
if [[ ! -f "$ENV_FILE" ]]; then
  SECRET_KEY=$(openssl rand -hex 32)
  cat > "$ENV_FILE" <<ENVBLOCK
DATABASE_URL=${DATABASE_URL}
SECRET_KEY=${SECRET_KEY}
HOST=0.0.0.0
PORT=${PORT}
LOG_LEVEL=info
INSTALL_DIR=${INSTALL_DIR}
ENVBLOCK
  chmod 600 "$ENV_FILE"
  info ".env written."
else
  info "Existing .env found — preserving credentials."
fi

# ── Hand ownership to the service user before running the CLI ──────────────────
# The CLI runs as $SYNAPTIC_USER so all files it creates are owned correctly.
chown -R "${SYNAPTIC_USER}:${SYNAPTIC_USER}" "$INSTALL_DIR"

# ── Run CLI installer ──────────────────────────────────────────────────────────
echo ""
info "── Running installer wizard ──────────────────────────────"

export PORT INSTALL_DIR APP_NAME NOTIFICATION_EMAIL

# Run as the dedicated service user (not root).
CLI_OUTPUT=$(sudo -u "${SYNAPTIC_USER}" \
  DATABASE_URL="$DATABASE_URL" \
  PORT="$PORT" \
  INSTALL_DIR="$INSTALL_DIR" \
  APP_NAME="$APP_NAME" \
  NOTIFICATION_EMAIL="${NOTIFICATION_EMAIL:-}" \
  SYNAPTIC_DOMAIN="$SYNAPTIC_DOMAIN" \
  ADMIN_EMAIL="$ADMIN_EMAIL" \
  ADMIN_USERNAME="${ADMIN_USERNAME:-}" \
  "${INSTALL_DIR}/synaptic-cli" install \
  --non-interactive \
  --output-dir "${INSTALL_DIR}" 2>&1) || die "synaptic-cli install failed:\n$CLI_OUTPUT"

echo "$CLI_OUTPUT"

# Extract and display generated password if one was produced.
if echo "$CLI_OUTPUT" | grep -q "^GENERATED_ADMIN_PASSWORD="; then
  GENERATED_PW=$(echo "$CLI_OUTPUT" | grep "^GENERATED_ADMIN_PASSWORD=" | cut -d= -f2-)
  echo ""
  warn "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  warn " SAVE YOUR ADMIN PASSWORD NOW — it will not be shown again:"
  warn ""
  warn "   Admin password: ${GENERATED_PW}"
  warn "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""
fi

# ── Install Caddyfile ──────────────────────────────────────────────────────────
echo ""
info "── Configuring Caddy ────────────────────────────────────"

if [[ -n "${SKIP_CADDY:-}" ]]; then
  warn "SKIP_CADDY set — skipping Caddy setup. Access the site directly at http://<server-ip>:${PORT}"
else
  # Caddy needs /var/log/caddy to write access logs.
  mkdir -p /var/log/caddy
  chown caddy:caddy /var/log/caddy 2>/dev/null || chown "${SYNAPTIC_USER}:${SYNAPTIC_USER}" /var/log/caddy || true

  if [[ -f "${INSTALL_DIR}/Caddyfile" ]]; then
    cp "${INSTALL_DIR}/Caddyfile" /etc/caddy/Caddyfile
    if systemctl is-active --quiet caddy; then
      caddy reload --config /etc/caddy/Caddyfile && success "Caddy reloaded."
    else
      systemctl enable --now caddy && success "Caddy started."
    fi
  else
    warn "Caddyfile not found at ${INSTALL_DIR}/Caddyfile — configure Caddy manually."
  fi

  # Set up Caddy write permissions so the admin panel can provision SSL for
  # additional sites without manual intervention.
  info "Setting up Caddy permissions for '${SYNAPTIC_USER}'..."
  "${INSTALL_DIR}/synaptic-cli" caddy setup --app-user "${SYNAPTIC_USER}" \
    && success "Caddy permissions configured." \
    || warn "Caddy permission setup failed — run manually: sudo synaptic-cli caddy setup --app-user ${SYNAPTIC_USER}"
fi

# ── Install systemd service ────────────────────────────────────────────────────
echo ""
info "── Configuring systemd service ──────────────────────────"

if [[ -f "${INSTALL_DIR}/synaptic-signals.service" ]]; then
  cp "${INSTALL_DIR}/synaptic-signals.service" /etc/systemd/system/
  # Ensure the service runs as the correct user — older binaries hardcode www-data.
  sed -i "s/^User=.*/User=${SYNAPTIC_USER}/" /etc/systemd/system/synaptic-signals.service
  sed -i "s/^Group=.*/Group=${SYNAPTIC_USER}/" /etc/systemd/system/synaptic-signals.service
  systemctl daemon-reload
  systemctl enable synaptic-signals
  systemctl restart synaptic-signals

  sleep 3
  if systemctl is-active --quiet synaptic-signals; then
    success "synaptic-signals service is running."
  else
    warn "Service failed to start. Check logs: journalctl -u synaptic-signals -f"
  fi
else
  warn "Service file not found — configure systemd manually."
fi

# ── Summary ────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}══════════════════════════════════════════════════${RESET}"
echo -e "${GREEN}${BOLD}  Synaptic Signals installed successfully         ${RESET}"
echo -e "${GREEN}${BOLD}══════════════════════════════════════════════════${RESET}"
echo ""
echo -e "  Install directory : ${INSTALL_DIR}"
echo -e "  Domain            : ${SYNAPTIC_DOMAIN}"
echo -e "  Site URL          : https://${SYNAPTIC_DOMAIN}"
echo -e "  Admin panel       : https://${SYNAPTIC_DOMAIN}/admin"
echo -e "  Admin email       : ${ADMIN_EMAIL}"
echo ""
echo -e "  Service status    : $(systemctl is-active synaptic-signals 2>/dev/null || echo 'unknown')"
echo -e "  Caddy status      : $(systemctl is-active caddy 2>/dev/null || echo 'unknown')"
echo ""
echo -e "  DB credentials    : ${INSTALL_DIR}/.env"
echo ""
echo -e "  View logs: journalctl -u synaptic-signals -f"
echo ""
