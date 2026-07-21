#!/usr/bin/env bash
# deploy-vps.sh — Build locally and deploy Synaptic Signals to a test VPS.
#
# Replaces the old deploy-vps-setup.sh + deploy-service-setup.sh pair.
# Always rebuilds fresh before shipping so the deployed binary's embedded
# migrations (sqlx::migrate!) exactly match the current migrations/ dir —
# migrations are compiled in, not read from disk at runtime.
#
# Usage:
#   ./scripts/deploy-vps.sh              # deploy + auto-configure (site/admin seeded
#                                         #   non-interactively) — fast path for dev testing
#   ./scripts/deploy-vps.sh --no-install # deploy infra only: build, ship, migrate,
#                                         #   start the service — no site/admin created.
#                                         #   Run `synap-cli install` by hand afterwards.
#   ./scripts/deploy-vps.sh --clean      # also drop+recreate the DB and remove
#                                         #   orphaned installs from prior attempts
#
# Config (env var overrides, defaults match the current test VPS):
#   VPS_HOST, VPS_USER, VPS_PORT, VPS_PASSWORD (or rely on SSH key/agent)
#   VPS_DOMAIN, INSTALL_DIR, SYNAPTIC_USER, APP_PORT
#   ADMIN_EMAIL, ADMIN_USERNAME, APP_NAME

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VPS_HOST="${VPS_HOST:-178.156.176.60}"
VPS_USER="${VPS_USER:-root}"
VPS_PORT="${VPS_PORT:-22}"
VPS_DOMAIN="${VPS_DOMAIN:-bckr.dev}"
INSTALL_DIR="${INSTALL_DIR:-/var/www/bckr.dev}"
SYNAPTIC_USER="${SYNAPTIC_USER:-www-data}"
APP_PORT="${APP_PORT:-3000}"
ADMIN_EMAIL="${ADMIN_EMAIL:-bill.coker@gmail.com}"
ADMIN_USERNAME="${ADMIN_USERNAME:-admin}"
APP_NAME="${APP_NAME:-Synaptic Signals}"
DB_NAME="${DB_NAME:-synaptic_signals}"
DB_USER="${DB_USER:-synaptic}"

CLEAN=0
NO_INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --clean) CLEAN=1 ;;
    --no-install) NO_INSTALL=1 ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

log()  { echo -e "\033[1m[deploy]\033[0m $*"; }
ok()   { echo -e "\033[0;32m[deploy]\033[0m $*"; }
warn() { echo -e "\033[1;33m[deploy] WARNING:\033[0m $*"; }
die()  { echo -e "\033[0;31m[deploy] ERROR:\033[0m $*" >&2; exit 1; }

SSH_OPTS=(-o StrictHostKeyChecking=accept-new -p "$VPS_PORT")
if [[ -n "${VPS_PASSWORD:-}" ]]; then
  command -v sshpass >/dev/null || die "sshpass required when VPS_PASSWORD is set"
  ssh_run()  { sshpass -e ssh "${SSH_OPTS[@]}" "${VPS_USER}@${VPS_HOST}" "$@"; }
  scp_run()  { sshpass -e scp -P "$VPS_PORT" -o StrictHostKeyChecking=accept-new "$@"; }
  export SSHPASS="$VPS_PASSWORD"
else
  ssh_run()  { ssh "${SSH_OPTS[@]}" "${VPS_USER}@${VPS_HOST}" "$@"; }
  scp_run()  { scp -P "$VPS_PORT" -o StrictHostKeyChecking=accept-new "$@"; }
fi

# ── 1. glibc compatibility preflight ────────────────────────────────────────
log "Checking glibc compatibility (local build machine vs VPS)..."
LOCAL_GLIBC=$(ldd --version | head -1 | grep -oE '[0-9]+\.[0-9]+$')
REMOTE_GLIBC=$(ssh_run "ldd --version | head -1 | grep -oE '[0-9]+\.[0-9]+\$'")
log "  local glibc:  $LOCAL_GLIBC"
log "  VPS glibc:    $REMOTE_GLIBC"
if [[ "$(printf '%s\n' "$LOCAL_GLIBC" "$REMOTE_GLIBC" | sort -V | tail -1)" != "$REMOTE_GLIBC" ]]; then
  die "Local glibc ($LOCAL_GLIBC) is newer than the VPS's ($REMOTE_GLIBC). \
A binary built here will fail to run there (GLIBC_x.y not found). \
Build on the VPS itself instead, or use an older/matching build environment."
fi
ok "glibc compatible — binary built locally will run on the VPS."

# ── 2. Fresh local build ────────────────────────────────────────────────────
log "Building release binaries locally (always fresh — never reuse a stale binary)..."
cd "$REPO_DIR"
cargo build --release --bin synaptic --bin synap-cli
BIN_SYNAPTIC="$REPO_DIR/target/release/synaptic"
BIN_CLI="$REPO_DIR/target/release/synap-cli"
[[ -f "$BIN_SYNAPTIC" && -f "$BIN_CLI" ]] || die "Build did not produce expected binaries."
ok "Build complete."

MIGRATION_COUNT=$(find "$REPO_DIR/migrations" -name '*.sql' | wc -l | tr -d ' ')
log "This binary has $MIGRATION_COUNT migrations embedded (from $REPO_DIR/migrations)."

# ── 3. Clean stale VPS state (only this app's footprint) ───────────────────
log "Stopping any existing/crash-looping synaptic-signals service..."
ssh_run "systemctl stop synaptic-signals 2>/dev/null; systemctl disable synaptic-signals 2>/dev/null; true"

if [[ "$CLEAN" -eq 1 ]]; then
  log "Cleaning previous installs and recreating the database fresh (--clean)..."
  ssh_run "rm -rf /var/www/synaptic-signals /opt/ssrcms-dev; true"
  # The app's own connection pool holds open sessions against this DB even
  # after the service is "stopped" (systemd stop can race the pool closing),
  # so force-drop rather than a plain DROP DATABASE which errors if anything
  # is still connected. WITH (FORCE) requires Postgres 13+.
  DROP_OUT=$(ssh_run "sudo -u postgres psql -tAc \"SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'\" | grep -q 1 && sudo -u postgres psql -c 'DROP DATABASE ${DB_NAME} WITH (FORCE);' 2>&1; true")
  echo "$DROP_OUT"
  if echo "$DROP_OUT" | grep -qi "error"; then
    die "Failed to drop ${DB_NAME} — check for other active connections/replicas above."
  fi
  ROLE_OUT=$(ssh_run "sudo -u postgres psql -tAc \"SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'\" | grep -q 1 && sudo -u postgres psql -c 'DROP ROLE ${DB_USER};' 2>&1; true")
  echo "$ROLE_OUT"
  if echo "$ROLE_OUT" | grep -qi "error"; then
    die "Failed to drop role ${DB_USER} — check for other objects it still owns above."
  fi
  # .env holds the now-invalid old DB password — remove it too so the DB-creds
  # step below regenerates both together instead of reusing a stale password
  # against a freshly recreated role.
  ssh_run "rm -f ${INSTALL_DIR}/.env"
  ok "Old installs, DB, and .env removed."
fi

# Ensure DB role + database exist (idempotent — safe with or without --clean).
DB_PASS_FILE_CHECK=$(ssh_run "test -f ${INSTALL_DIR}/.env && grep -q '^DATABASE_URL=' ${INSTALL_DIR}/.env && echo yes || echo no")
if [[ "$DB_PASS_FILE_CHECK" == "yes" ]]; then
  log "Existing .env found on VPS — reusing DATABASE_URL."
  DATABASE_URL=$(ssh_run "grep '^DATABASE_URL=' ${INSTALL_DIR}/.env | cut -d= -f2-")
else
  DB_PASS=$(openssl rand -hex 16)
  DATABASE_URL="postgres://${DB_USER}:${DB_PASS}@localhost:5432/${DB_NAME}"
  log "Generating fresh database credentials..."
  SQL_TMP="/tmp/ss-deploy-db-$$.sql"
  cat > "$SQL_TMP" <<SQL
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
  scp_run "$SQL_TMP" "${VPS_USER}@${VPS_HOST}:/tmp/ss-deploy-db.sql"
  rm -f "$SQL_TMP"
  ssh_run "sudo -u postgres psql -f /tmp/ss-deploy-db.sql && rm -f /tmp/ss-deploy-db.sql"
  ok "Database ready."
fi

# ── 4. Ship files ────────────────────────────────────────────────────────────
log "Ensuring install directory exists on VPS..."
ssh_run "mkdir -p ${INSTALL_DIR}/uploads ${INSTALL_DIR}/search-index ${INSTALL_DIR}/themes/sites ${INSTALL_DIR}/plugins/sites"

log "Copying binaries..."
scp_run "$BIN_SYNAPTIC" "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synaptic"
scp_run "$BIN_CLI"      "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synap-cli"

log "Copying themes, plugins, admin static assets..."
ssh_run "mkdir -p ${INSTALL_DIR}/admin"
tar czf /tmp/ss-deploy-assets.tar.gz -C "$REPO_DIR" themes plugins -C "$REPO_DIR/admin" static
scp_run /tmp/ss-deploy-assets.tar.gz "${VPS_USER}@${VPS_HOST}:/tmp/ss-deploy-assets.tar.gz"
ssh_run "tar xzf /tmp/ss-deploy-assets.tar.gz -C ${INSTALL_DIR} --overwrite themes plugins 2>/dev/null; \
         mkdir -p ${INSTALL_DIR}/admin && tar xzf /tmp/ss-deploy-assets.tar.gz -C ${INSTALL_DIR}/admin --overwrite static 2>/dev/null; \
         rm -f /tmp/ss-deploy-assets.tar.gz"
rm -f /tmp/ss-deploy-assets.tar.gz
ok "Assets copied."

ssh_run "chmod +x ${INSTALL_DIR}/synaptic ${INSTALL_DIR}/synap-cli"
ssh_run "ln -sf ${INSTALL_DIR}/synap-cli /usr/local/bin/synap-cli"
ssh_run "chown -R ${SYNAPTIC_USER}:${SYNAPTIC_USER} ${INSTALL_DIR}"

# SELinux context (AlmaLinux) — matches install.sh behavior.
ssh_run "command -v chcon >/dev/null && chcon -Rt var_t ${INSTALL_DIR} 2>/dev/null; \
         command -v chcon >/dev/null && chcon -t bin_t ${INSTALL_DIR}/synaptic ${INSTALL_DIR}/synap-cli 2>/dev/null; true"

# ── 5. .env ──────────────────────────────────────────────────────────────────
ENV_EXISTS=$(ssh_run "test -f ${INSTALL_DIR}/.env && echo yes || echo no")
if [[ "$ENV_EXISTS" == "no" ]]; then
  SECRET_KEY=$(openssl rand -hex 32)
  log "Writing fresh .env..."
  ssh_run "cat > ${INSTALL_DIR}/.env <<ENVEOF
DATABASE_URL=${DATABASE_URL}
SECRET_KEY=${SECRET_KEY}
HOST=0.0.0.0
PORT=${APP_PORT}
LOG_LEVEL=info
INSTALL_DIR=${INSTALL_DIR}
ENVEOF
chown ${SYNAPTIC_USER}:${SYNAPTIC_USER} ${INSTALL_DIR}/.env
chmod 600 ${INSTALL_DIR}/.env"
else
  log "Existing .env preserved."
fi

# ── 6. Database + app config ────────────────────────────────────────────────
if [[ "$NO_INSTALL" -eq 1 ]]; then
  log "Running migrations only (--no-install — no site/admin will be created)..."
  ssh_run "sudo -u ${SYNAPTIC_USER} DATABASE_URL='${DATABASE_URL}' ${INSTALL_DIR}/synap-cli migrate" \
    || die "synap-cli migrate failed."
  ok "Migrations applied. No site/admin configured yet."

  log "Generating Caddyfile and systemd unit from deployment/ templates..."
  CADDY_TMP="/tmp/ss-deploy-caddyfile-$$"
  SVC_TMP="/tmp/ss-deploy-service-$$"
  sed -e "s#{DOMAIN}#${VPS_DOMAIN}#g" \
      -e "s#{PORT}#${APP_PORT}#g" \
      -e "s#{UPLOADS_DIR}#${INSTALL_DIR}/uploads#g" \
      -e "s#{THEME_DIR}#${INSTALL_DIR}/themes#g" \
      "$REPO_DIR/deployment/Caddyfile.template" > "$CADDY_TMP"
  sed -e "s#{INSTALL_DIR}#${INSTALL_DIR}#g" \
      -e "s#{SERVICE_USER}#${SYNAPTIC_USER}#g" \
      "$REPO_DIR/deployment/synaptic-signals.service" > "$SVC_TMP"
  scp_run "$CADDY_TMP" "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/Caddyfile"
  scp_run "$SVC_TMP"   "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synaptic-signals.service"
  rm -f "$CADDY_TMP" "$SVC_TMP"
  ok "Templates generated."
else
  log "Running synap-cli install --non-interactive on the VPS..."
  CLI_OUTPUT=$(ssh_run "sudo -u ${SYNAPTIC_USER} \
    DATABASE_URL='${DATABASE_URL}' \
    PORT='${APP_PORT}' \
    INSTALL_DIR='${INSTALL_DIR}' \
    APP_NAME='${APP_NAME}' \
    SYNAPTIC_DOMAIN='${VPS_DOMAIN}' \
    ADMIN_EMAIL='${ADMIN_EMAIL}' \
    ADMIN_USERNAME='${ADMIN_USERNAME}' \
    ${INSTALL_DIR}/synap-cli install --non-interactive --output-dir ${INSTALL_DIR} 2>&1") \
    || die "synap-cli install failed:\n$CLI_OUTPUT"
  echo "$CLI_OUTPUT"

  if echo "$CLI_OUTPUT" | grep -q "^GENERATED_ADMIN_PASSWORD="; then
    GENERATED_PW=$(echo "$CLI_OUTPUT" | grep "^GENERATED_ADMIN_PASSWORD=" | cut -d= -f2-)
    warn "SAVE YOUR ADMIN PASSWORD NOW — it will not be shown again: ${GENERATED_PW}"
  fi
  ok "Install script complete."
fi

# ── 7. Caddy + systemd ───────────────────────────────────────────────────────
log "Installing Caddyfile and systemd unit..."
ssh_run "mkdir -p /var/log/caddy && chown caddy:caddy /var/log/caddy 2>/dev/null || true"
ssh_run "test -f ${INSTALL_DIR}/Caddyfile && cp ${INSTALL_DIR}/Caddyfile /etc/caddy/Caddyfile"
ssh_run "systemctl is-active --quiet caddy && caddy reload --config /etc/caddy/Caddyfile || systemctl enable --now caddy"

ssh_run "test -f ${INSTALL_DIR}/synaptic-signals.service && cp ${INSTALL_DIR}/synaptic-signals.service /etc/systemd/system/synaptic-signals.service"
ssh_run "systemctl daemon-reload && systemctl enable synaptic-signals && systemctl restart synaptic-signals"

log "Waiting for service to come up..."
sleep 3

# ── 8. Verify ────────────────────────────────────────────────────────────────
STATUS=$(ssh_run "systemctl is-active synaptic-signals" || true)
if [[ "$STATUS" == "active" ]]; then
  ok "synaptic-signals is active."
else
  warn "synaptic-signals is '$STATUS' — recent logs:"
  ssh_run "journalctl -u synaptic-signals -n 30 --no-pager"
  die "Deployment finished but the service is not running. See logs above."
fi

log "Recent logs:"
ssh_run "journalctl -u synaptic-signals -n 15 --no-pager"

log "Applied migration count on VPS:"
ssh_run "sudo -u postgres psql ${DB_NAME} -tAc 'SELECT count(*) FROM _sqlx_migrations' 2>&1"

log "HTTP check..."
# Send the Host header — this is a multi-site app, so a bare localhost:PORT
# request 404s (no site registered for that host) even when healthy.
HTTP_CODE=$(ssh_run "curl -s -o /dev/null -w '%{http_code}' -H 'Host: ${VPS_DOMAIN}' http://localhost:${APP_PORT}/ 2>&1")
if [[ "$NO_INSTALL" -eq 1 ]]; then
  if [[ "$HTTP_CODE" =~ ^(200|30[0-9]|404)$ ]]; then
    ok "App is up and responding (HTTP $HTTP_CODE — 404 is expected, no site configured yet)."
  else
    warn "App did not respond as expected (got HTTP $HTTP_CODE) — check logs above."
  fi
else
  if [[ "$HTTP_CODE" =~ ^(200|30[0-9])$ ]]; then
    ok "App responding locally on the VPS (port ${APP_PORT}, Host: ${VPS_DOMAIN})."
  else
    warn "Local HTTP check on port ${APP_PORT} did not return 200/3xx (got $HTTP_CODE) — check logs above."
  fi
fi

echo ""
if [[ "$NO_INSTALL" -eq 1 ]]; then
  ok "Deploy complete — app is running, no site/admin configured yet."
  echo ""
  echo "  Next step, on the VPS (must run as ${SYNAPTIC_USER} — synap-cli checks"
  echo "  that it owns \$INSTALL_DIR. Use the full path, not the bare command —"
  echo "  sudo's secure_path on RHEL/AlmaLinux drops /usr/local/bin):"
  echo "    sudo -u ${SYNAPTIC_USER} bash -c 'cd ${INSTALL_DIR} && ./synap-cli install'"
  echo ""
  echo "  (Answer the prompts for domain, admin email/username/password, etc."
  echo "   This also regenerates the Caddyfile/systemd unit for the values you choose —"
  echo "   re-run this deploy script, or 'systemctl reload caddy' + 'systemctl restart"
  echo "   synaptic-signals', afterwards to pick them up.)"
else
  ok "Deploy complete. Site: https://${VPS_DOMAIN}  (admin: https://${VPS_DOMAIN}/admin)"
fi
