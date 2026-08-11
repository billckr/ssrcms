#!/usr/bin/env bash
# install-vps.sh — Interactive installer/deploy wizard for SynapCMS
# on a VPS: builds locally, ships to the VPS over ssh/scp, and configures it.
#
# Welcome screen, a defaults-vs-interactive mode choice, validated field
# prompts, an upfront requirements check (rendered as a pass/fail table
# before anything destructive happens), per-step progress instead of a wall
# of log lines, and a final summary with the admin login details and next
# steps. --defaults skips all prompts for scripted/CI use (automatic
# whenever stdin isn't a TTY, regardless of flags).
#
# Run './scripts/install-vps.sh --help' for full usage and examples.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Defaults (overridable via env, or via the interactive wizard) ──────────
VPS_HOST="${VPS_HOST:-178.156.176.60}"
VPS_USER="${VPS_USER:-root}"
VPS_PORT="${VPS_PORT:-22}"
VPS_PASSWORD="${VPS_PASSWORD:-}"
VPS_DOMAIN="${VPS_DOMAIN:-synapcms.dev}"
INSTALL_DIR="${INSTALL_DIR:-/var/www/bckr.dev}"
SYNAPTIC_USER="${SYNAPTIC_USER:-www-data}"
APP_PORT="${APP_PORT:-3000}"
ADMIN_EMAIL="${ADMIN_EMAIL:-bill.coker@gmail.com}"
ADMIN_USERNAME="${ADMIN_USERNAME:-admin}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-}"
APP_NAME="${APP_NAME:-SynapCMS}"
DB_NAME="${DB_NAME:-synapcms}"
DB_USER="${DB_USER:-synap}"

CLEAN=0
UPDATE_ONLY=0
MODE=""

print_help() {
  cat <<EOF
install-vps.sh — Interactive installer/deploy wizard for SynapCMS.

Builds locally, ships to a VPS over ssh/scp, and configures it. Welcome
screen, defaults-vs-interactive mode choice, validated field prompts, an
upfront requirements check, per-step progress, and a final summary with
admin login details. --defaults skips all prompts for scripted/CI use.

USAGE
  ./scripts/install-vps.sh [--defaults|--interactive] [--update] [--clean]
  ./scripts/install-vps.sh --help

FLAGS
  --defaults     Skip the welcome menu, use default/env-supplied settings.
                 Automatic if stdin isn't a TTY (e.g. piped input, CI).
  --interactive  Skip the welcome menu, go straight to the prompts.
  --update       Push a code update to an already-running install: rebuild,
                 re-ship files, apply any new migrations, restart the
                 service. Does not touch existing sites/admins/data, and
                 does not create a site — use this for redeploying a code
                 change. On a brand-new install, this leaves the app
                 running with no site/admin yet; the summary screen prints
                 the 'synap install' command to run by hand afterward.
  --clean        Also drop+recreate the database and role, and wipe
                 INSTALL_DIR, before deploying. DESTRUCTIVE to whatever is
                 currently at INSTALL_DIR/DB_NAME — never touches other
                 sites/databases sharing the VPS. Requires Postgres 13+.
  -h, --help     Show this help and exit. No build or network calls happen.

EXAMPLES
  First-time install, walking through every prompt yourself:
    VPS_PASSWORD='...' ./scripts/install-vps.sh --interactive

  First-time install, fully automated with the built-in defaults:
    VPS_PASSWORD='...' ./scripts/install-vps.sh --defaults

  Full reset — wipe INSTALL_DIR + drop/recreate the DB, then install fresh
  (use when the VPS is in an unknown/broken state):
    VPS_PASSWORD='...' ./scripts/install-vps.sh --defaults --clean

  Push a code update to an already-running install — rebuild, re-ship,
  migrate, restart, but don't touch existing sites/admins/data:
    VPS_PASSWORD='...' ./scripts/install-vps.sh --defaults --update

  Get the binary running as a service with no site/admin yet, then answer
  'synap install's prompts yourself on the VPS afterward (mirrors how
  the production installer, scripts/install.sh, is meant to be used):
    VPS_PASSWORD='...' ./scripts/install-vps.sh --interactive --update

  Deploy a second, different site to the same VPS (override the target —
  see CONFIG below; --clean here only ever touches THIS INSTALL_DIR/DB_NAME,
  never the other site's):
    VPS_PASSWORD='...' VPS_DOMAIN=example.com \\
      INSTALL_DIR=/var/www/example.com DB_NAME=example_com DB_USER=example \\
      ./scripts/install-vps.sh --defaults

  Using SSH key/agent auth instead of a password (omit VPS_PASSWORD):
    ./scripts/install-vps.sh --defaults

CONFIG (environment variable overrides — used as defaults in both modes)
  VPS_HOST        ${VPS_HOST}
  VPS_USER        ${VPS_USER}
  VPS_PORT        ${VPS_PORT}
  VPS_PASSWORD    (unset — falls back to SSH key/agent auth)
  VPS_DOMAIN      ${VPS_DOMAIN}
  INSTALL_DIR     ${INSTALL_DIR}
  SYNAPTIC_USER   ${SYNAPTIC_USER}
  APP_PORT        ${APP_PORT}
  ADMIN_EMAIL     ${ADMIN_EMAIL}
  ADMIN_USERNAME  ${ADMIN_USERNAME}
  ADMIN_PASSWORD  (unset — a compliant password is generated if omitted)
  APP_NAME        ${APP_NAME}
  DB_NAME         ${DB_NAME}
  DB_USER         ${DB_USER}

REQUIREMENTS / LIMITS
  Local: cargo/rustc/ssh/scp/openssl (+ sshpass if using VPS_PASSWORD).
  Remote: a systemd + Caddy host with Postgres 13+ and passwordless sudo
  for postgres/\${SYNAPTIC_USER}. All checked upfront in the requirements
  screen before anything is built or touched.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --clean) CLEAN=1 ;;
    --update) UPDATE_ONLY=1 ;;
    --defaults) MODE="defaults" ;;
    --interactive) MODE="interactive" ;;
    -h|--help) print_help; exit 0 ;;
    *) echo "Unknown argument: $arg" >&2; echo "Run with --help for usage." >&2; exit 1 ;;
  esac
done

# Non-TTY stdin (piped input, CI) always forces non-interactive, regardless of flag.
if [[ ! -t 0 ]]; then
  MODE="defaults"
fi

# ── UI helpers ───────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then TTY_OUT=1; else TTY_OUT=0; fi

C_BOLD=$'\033[1m'; C_RESET=$'\033[0m'
C_RED=$'\033[0;31m'; C_GREEN=$'\033[0;32m'; C_YELLOW=$'\033[1;33m'; C_CYAN=$'\033[0;36m'

WARNINGS=()
log()  { echo -e "${C_BOLD}[install]${C_RESET} $*"; }
ok()   { echo -e "${C_GREEN}[install]${C_RESET} $*"; }
warn() { echo -e "${C_YELLOW}[install] WARNING:${C_RESET} $*"; WARNINGS+=("$*"); }
die()  { echo -e "${C_RED}[install] ERROR:${C_RESET} $*" >&2; exit 1; }

box_header() {
  echo ""
  echo -e "${C_CYAN}════════════════════════════════════════════════════════════${C_RESET}"
  echo -e "${C_BOLD}  $1${C_RESET}"
  echo -e "${C_CYAN}════════════════════════════════════════════════════════════${C_RESET}"
}

section() { echo ""; echo -e "${C_BOLD}── $1 ──${C_RESET}"; }

# ── Step progress (spinner) ─────────────────────────────────────────────────
SPINNER_PID=""
CURRENT_STEP_LABEL=""

spinner_start() {
  CURRENT_STEP_LABEL="$1"
  if [[ "$TTY_OUT" -eq 1 ]]; then
    (
      frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
      i=0
      while :; do
        printf "\r\033[2K  %s%s%s %s" "$C_CYAN" "${frames:$((i%10)):1}" "$C_RESET" "$CURRENT_STEP_LABEL"
        i=$((i+1))
        sleep 0.1
      done
    ) &
    SPINNER_PID=$!
    disown "$SPINNER_PID" 2>/dev/null || true
  else
    log "$CURRENT_STEP_LABEL"
  fi
}

spinner_stop() {
  local rc="$1"
  if [[ "$TTY_OUT" -eq 1 && -n "$SPINNER_PID" ]]; then
    kill "$SPINNER_PID" 2>/dev/null || true
    wait "$SPINNER_PID" 2>/dev/null || true
    SPINNER_PID=""
    printf "\r\033[2K"
    if [[ "$rc" -eq 0 ]]; then
      echo -e "  ${C_GREEN}✓${C_RESET} ${CURRENT_STEP_LABEL}"
    else
      echo -e "  ${C_RED}✗${C_RESET} ${CURRENT_STEP_LABEL}"
    fi
  else
    if [[ "$rc" -eq 0 ]]; then ok "${CURRENT_STEP_LABEL} — done"; else warn "${CURRENT_STEP_LABEL} — failed"; fi
  fi
}

cleanup_spinner() { [[ -n "$SPINNER_PID" ]] && kill "$SPINNER_PID" 2>/dev/null; true; }
trap cleanup_spinner EXIT

# Runs a do_* function with output buffered to a temp file (not a subshell —
# plain redirection — so variables the function sets, e.g. DATABASE_URL,
# stay visible to the caller afterward). Dumps the buffer and exits on
# failure; discards it on success, keeping the screen clean.
run_step() {
  local label="$1"; shift
  spinner_start "$label"
  local logfile rc
  logfile=$(mktemp)
  set +e
  "$@" > "$logfile" 2>&1
  rc=$?
  set -e
  spinner_stop "$rc"
  if [[ "$rc" -ne 0 ]]; then
    echo "----- output -----" >&2
    cat "$logfile" >&2
    rm -f "$logfile"
    die "Step failed: $label"
  fi
  rm -f "$logfile"
}

# ── Validators — each echoes an error string on failure, nothing on success ─
valid_nonempty()          { [[ -n "$1" ]] || echo "This field is required."; }
valid_nonblank_text()     { [[ -n "${1// /}" ]] || echo "This field is required."; }

valid_hostname_or_ip() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  if [[ "$v" =~ ^([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})$ ]]; then
    local o
    for o in "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" "${BASH_REMATCH[4]}"; do
      if (( o > 255 )); then echo "Not a valid IPv4 address (octet > 255)."; return; fi
    done
  elif [[ ! "$v" =~ ^[A-Za-z0-9.-]+$ ]]; then
    echo "Only letters, digits, dots, and hyphens are allowed."
  fi
}

valid_domain() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?)+$ ]] \
    || echo "Doesn't look like a valid domain (e.g. example.com)."
}

valid_unix_user() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[a-z_][a-z0-9_-]*$ ]] \
    || echo "Must start with a lowercase letter or underscore, then lowercase letters/digits/hyphens/underscores."
}

valid_port() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[0-9]+$ ]] || { echo "Must be a number."; return; }
  (( v >= 1 && v <= 65535 )) || echo "Must be between 1 and 65535."
}

valid_absolute_path() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" == /* ]] || { echo "Must be an absolute path (start with /)."; return; }
  [[ "$v" != *' '* ]] || echo "Must not contain spaces."
}

valid_email() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[^@[:space:]]+@[^@[:space:]]+\.[^@[:space:]]+$ ]] || echo "Doesn't look like a valid email address."
}

valid_username() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[A-Za-z0-9_-]{3,32}$ ]] || echo "3-32 characters: letters, digits, underscores, hyphens."
}

valid_db_identifier() {
  local v="$1"
  [[ -n "$v" ]] || { echo "This field is required."; return; }
  [[ "$v" =~ ^[a-z_][a-z0-9_]*$ ]] \
    || echo "Must start with a lowercase letter or underscore, then lowercase letters/digits/underscores only."
}

# Mirrors cli/src/commands/install.rs::validate_password exactly.
valid_admin_password() {
  local v="$1" len=${#1}
  (( len >= 8 )) || { echo "Must be at least 8 characters."; return; }
  (( len <= 12 )) || { echo "Must be no more than 12 characters."; return; }
  [[ "$v" =~ [A-Z] ]] || { echo "Must contain at least one uppercase letter."; return; }
  [[ "$v" =~ [0-9] ]] || { echo "Must contain at least one number."; return; }
  [[ "$v" =~ [\!\@\#\$\%\&\*_+-] ]] || echo "Must contain at least one symbol: ! @ # \$ % & * - _ +"
}

# ── Prompt helpers ───────────────────────────────────────────────────────────
# prompt_field <varname> <prompt text> <default> <validator fn>
prompt_field() {
  local __var="$1" __prompt="$2" __default="$3" __validator="$4"
  local __input __err
  while true; do
    if [[ -n "$__default" ]]; then
      read -rp "  ${__prompt} [${__default}]: " __input
    else
      read -rp "  ${__prompt}: " __input
    fi
    [[ -z "$__input" ]] && __input="$__default"
    __err=$("$__validator" "$__input")
    if [[ -z "$__err" ]]; then
      printf -v "$__var" '%s' "$__input"
      return 0
    fi
    echo -e "    ${C_RED}✗${C_RESET} ${__err}"
  done
}

# prompt_yes_no <prompt text> <default: y|n> — returns 0 for yes, 1 for no
prompt_yes_no() {
  local __prompt="$1" __default="${2:-y}" __input __suffix
  if [[ "$__default" == "y" ]]; then __suffix="[Y/n]"; else __suffix="[y/N]"; fi
  while true; do
    read -rp "  ${__prompt} ${__suffix}: " __input
    __input="${__input:-$__default}"
    case "$__input" in
      [Yy]|[Yy][Ee][Ss]) return 0 ;;
      [Nn]|[Nn][Oo]) return 1 ;;
      *) echo -e "    ${C_RED}✗${C_RESET} Please answer y or n." ;;
    esac
  done
}

# prompt_password_confirm <varname> <prompt text> <validator fn>
prompt_password_confirm() {
  local __var="$1" __prompt="$2" __validator="$3"
  local __p1 __p2 __err
  while true; do
    read -rsp "  ${__prompt}: " __p1; echo ""
    __err=$("$__validator" "$__p1")
    if [[ -n "$__err" ]]; then
      echo -e "    ${C_RED}✗${C_RESET} ${__err}"
      continue
    fi
    read -rsp "  Confirm: " __p2; echo ""
    if [[ "$__p1" != "$__p2" ]]; then
      echo -e "    ${C_RED}✗${C_RESET} Passwords don't match — try again."
      continue
    fi
    printf -v "$__var" '%s' "$__p1"
    return 0
  done
}

valid_nonempty_password() { [[ -n "$1" ]] || echo "Password cannot be empty."; }

# ── Welcome / wizard ─────────────────────────────────────────────────────────
welcome_screen() {
  echo -e "${C_CYAN}"
  cat <<'BANNER'
   SynapCMS — VPS Installer
BANNER
  echo -e "${C_RESET}"
  echo "  Welcome. Let's get the app deployed."
  echo ""
  if prompt_yes_no "Use default settings? (no = interactive setup)" "y"; then
    MODE="defaults"
  else
    MODE="interactive"
  fi
}

run_interactive_wizard() {
  echo ""
  echo "  Press Enter to accept the shown default for any field."

  section "VPS Connection"
  prompt_field VPS_HOST "VPS hostname or IP" "$VPS_HOST" valid_hostname_or_ip
  prompt_field VPS_USER "SSH user" "$VPS_USER" valid_unix_user
  prompt_field VPS_PORT "SSH port" "$VPS_PORT" valid_port
  if prompt_yes_no "Use SSH key/agent auth?" "y"; then
    VPS_PASSWORD=""
  else
    prompt_password_confirm VPS_PASSWORD "SSH password" valid_nonempty_password
  fi

  section "Site"
  prompt_field VPS_DOMAIN "Domain (e.g. example.com)" "$VPS_DOMAIN" valid_domain
  prompt_field INSTALL_DIR "Install directory on the VPS" "/var/www/${VPS_DOMAIN}" valid_absolute_path
  prompt_field SYNAPTIC_USER "Service user on the VPS" "$SYNAPTIC_USER" valid_unix_user
  prompt_field APP_PORT "Internal app port" "$APP_PORT" valid_port
  if [[ "$APP_PORT" == "$VPS_PORT" ]]; then
    warn "APP_PORT and the SSH port are both ${APP_PORT} — this will likely conflict."
  fi
  prompt_field APP_NAME "App display name" "$APP_NAME" valid_nonblank_text

  if [[ "$UPDATE_ONLY" -eq 0 ]]; then
    section "Admin Account"
    prompt_field ADMIN_EMAIL "Admin email" "$ADMIN_EMAIL" valid_email
    prompt_field ADMIN_USERNAME "Admin username" "$ADMIN_USERNAME" valid_username
    if prompt_yes_no "Auto-generate a secure admin password? (recommended)" "y"; then
      ADMIN_PASSWORD=""
    else
      prompt_password_confirm ADMIN_PASSWORD "Admin password (8-12 chars, upper+digit+symbol)" valid_admin_password
    fi
  fi

  section "Database"
  prompt_field DB_NAME "Database name" "$DB_NAME" valid_db_identifier
  prompt_field DB_USER "Database user" "$DB_USER" valid_db_identifier
}

review_and_confirm() {
  box_header "Review Settings"
  printf "  %-16s %s\n" "VPS host:"      "$VPS_HOST"
  printf "  %-16s %s\n" "SSH port:"      "$VPS_PORT"
  printf "  %-16s %s\n" "Auth:"          "$( [[ -n "$VPS_PASSWORD" ]] && echo "password" || echo "SSH key/agent" )"
  printf "  %-16s %s\n" "Domain:"        "$VPS_DOMAIN"
  printf "  %-16s %s\n" "Install dir:"   "$INSTALL_DIR"
  printf "  %-16s %s\n" "Service user:"  "$SYNAPTIC_USER"
  printf "  %-16s %s\n" "App port:"      "$APP_PORT"
  printf "  %-16s %s\n" "App name:"      "$APP_NAME"
  if [[ "$UPDATE_ONLY" -eq 0 ]]; then
    printf "  %-16s %s\n" "Admin email:"    "$ADMIN_EMAIL"
    printf "  %-16s %s\n" "Admin username:" "$ADMIN_USERNAME"
    printf "  %-16s %s\n" "Admin password:" "$( [[ -n "$ADMIN_PASSWORD" ]] && echo "custom (hidden)" || echo "auto-generate" )"
  fi
  printf "  %-16s %s\n" "DB name:"       "$DB_NAME"
  printf "  %-16s %s\n" "DB user:"       "$DB_USER"
  local flags_display=""
  [[ "$CLEAN" -eq 1 ]] && flags_display+="--clean "
  [[ "$UPDATE_ONLY" -eq 1 ]] && flags_display+="--update "
  [[ -z "$flags_display" ]] && flags_display="(none)"
  printf "  %-16s %s\n" "Flags:" "$flags_display"
  echo ""
  # No TTY on stdin (piped input, CI) — there's no one to ask, so proceed
  # without prompting rather than hanging on a read() that will never return.
  if [[ ! -t 0 ]]; then
    return 0
  fi
  if ! prompt_yes_no "Proceed with these settings?" "y"; then
    echo ""
    log "Aborted — nothing was changed. Re-run to adjust settings."
    exit 0
  fi
}

# ── SSH helpers (defined after mode/wizard resolve VPS_PASSWORD) ───────────
define_ssh_helpers() {
  SSH_OPTS=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -p "$VPS_PORT")
  if [[ -n "${VPS_PASSWORD:-}" ]]; then
    command -v sshpass >/dev/null || die "sshpass required when using password auth. Install it and re-run."
    ssh_run()  { sshpass -e ssh "${SSH_OPTS[@]}" "${VPS_USER}@${VPS_HOST}" "$@"; }
    scp_run()  { sshpass -e scp -P "$VPS_PORT" -o StrictHostKeyChecking=accept-new "$@"; }
    export SSHPASS="$VPS_PASSWORD"
  else
    ssh_run()  { ssh "${SSH_OPTS[@]}" "${VPS_USER}@${VPS_HOST}" "$@"; }
    scp_run()  { scp -P "$VPS_PORT" -o StrictHostKeyChecking=accept-new "$@"; }
  fi
}

# ── Requirements check ───────────────────────────────────────────────────────
REQ_RESULTS=()
req_check() { REQ_RESULTS+=("$2|$1|$3"); }  # req_check <name> <PASS|FAIL|SKIP> <detail>

check_requirements() {
  echo ""
  log "Checking requirements..."
  REQ_RESULTS=()

  if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
    req_check "Local: Rust toolchain" PASS "$(rustc --version 2>/dev/null | head -1)"
  else
    req_check "Local: Rust toolchain" FAIL "cargo/rustc not found — install from https://rustup.rs"
  fi
  command -v ssh  >/dev/null 2>&1 && req_check "Local: ssh"     PASS "" || req_check "Local: ssh"     FAIL "not found"
  command -v scp  >/dev/null 2>&1 && req_check "Local: scp"     PASS "" || req_check "Local: scp"     FAIL "not found"
  command -v openssl >/dev/null 2>&1 && req_check "Local: openssl" PASS "" || req_check "Local: openssl" FAIL "not found"
  if [[ -n "${VPS_PASSWORD:-}" ]]; then
    command -v sshpass >/dev/null 2>&1 \
      && req_check "Local: sshpass" PASS "" \
      || req_check "Local: sshpass" FAIL "not found — required for password auth"
  fi

  local ssh_ok=1
  if ssh_run "true" >/dev/null 2>&1; then
    req_check "Remote: SSH connectivity" PASS "${VPS_USER}@${VPS_HOST}:${VPS_PORT}"
  else
    req_check "Remote: SSH connectivity" FAIL "could not connect — check host/port/credentials"
    ssh_ok=0
  fi

  if [[ "$ssh_ok" -eq 1 ]]; then
    ssh_run "command -v systemctl" >/dev/null 2>&1 \
      && req_check "Remote: systemd" PASS "" \
      || req_check "Remote: systemd" FAIL "systemctl not found"
    ssh_run "command -v caddy" >/dev/null 2>&1 \
      && req_check "Remote: Caddy" PASS "" \
      || req_check "Remote: Caddy" FAIL "caddy not found on PATH"

    local pg_ver
    pg_ver=$(ssh_run "sudo -u postgres psql -tAc 'SHOW server_version_num' 2>/dev/null" || true)
    pg_ver="$(echo "$pg_ver" | tr -d '[:space:]')"
    if [[ "$pg_ver" =~ ^[0-9]+$ ]] && (( pg_ver >= 130000 )); then
      req_check "Remote: PostgreSQL >= 13" PASS "version $pg_ver"
    else
      req_check "Remote: PostgreSQL >= 13" FAIL "detected: ${pg_ver:-none/unreachable}"
    fi

    ssh_run "sudo -n -u postgres true" >/dev/null 2>&1 \
      && req_check "Remote: passwordless sudo (postgres)" PASS "" \
      || req_check "Remote: passwordless sudo (postgres)" FAIL "sudo -u postgres requires a password"
    ssh_run "sudo -n -u ${SYNAPTIC_USER} true" >/dev/null 2>&1 \
      && req_check "Remote: passwordless sudo (${SYNAPTIC_USER})" PASS "" \
      || req_check "Remote: passwordless sudo (${SYNAPTIC_USER})" FAIL "sudo -u ${SYNAPTIC_USER} requires a password"

    local local_glibc remote_glibc
    local_glibc=$(ldd --version | head -1 | grep -oE '[0-9]+\.[0-9]+$')
    remote_glibc=$(ssh_run "ldd --version | head -1 | grep -oE '[0-9]+\.[0-9]+\$'" || true)
    if [[ -n "$remote_glibc" ]] && [[ "$(printf '%s\n' "$local_glibc" "$remote_glibc" | sort -V | tail -1)" == "$remote_glibc" ]]; then
      req_check "glibc compatibility" PASS "local $local_glibc <= VPS $remote_glibc"
    else
      req_check "glibc compatibility" FAIL "local $local_glibc > VPS ${remote_glibc:-unknown} — build on the VPS instead"
    fi
  else
    req_check "Remote: systemd" SKIP "skipped — SSH connectivity failed above"
    req_check "Remote: Caddy" SKIP "skipped — SSH connectivity failed above"
    req_check "Remote: PostgreSQL >= 13" SKIP "skipped — SSH connectivity failed above"
    req_check "Remote: passwordless sudo (postgres)" SKIP "skipped — SSH connectivity failed above"
    req_check "Remote: passwordless sudo (${SYNAPTIC_USER})" SKIP "skipped — SSH connectivity failed above"
    req_check "glibc compatibility" SKIP "skipped — SSH connectivity failed above"
  fi
}

render_requirements_table() {
  echo ""
  local any_fail=0 row status name detail glyph color
  for row in "${REQ_RESULTS[@]}"; do
    IFS='|' read -r status name detail <<< "$row"
    case "$status" in
      PASS) glyph="✓"; color="$C_GREEN" ;;
      FAIL) glyph="✗"; color="$C_RED"; any_fail=1 ;;
      SKIP) glyph="–"; color="$C_YELLOW" ;;
    esac
    printf "  %s%s%s  %-42s %s\n" "$color" "$glyph" "$C_RESET" "$name" "$detail"
  done
  echo ""
  return $any_fail
}

gate_on_requirements() {
  if ! render_requirements_table; then
    echo -e "${C_RED}${C_BOLD}Some requirements are missing — install/fix them and re-run.${C_RESET}"
    exit 1
  fi
  ok "All requirements met."
}

# ── Install steps — each is a plain function so run_step() can wrap it ─────

do_build() {
  cd "$REPO_DIR"
  cargo build --release --bin synaptic --bin synap
  BIN_SYNAPTIC="$REPO_DIR/target/release/synaptic"
  BIN_CLI="$REPO_DIR/target/release/synap"
  [[ -f "$BIN_SYNAPTIC" && -f "$BIN_CLI" ]] || { echo "Build did not produce expected binaries." >&2; return 1; }
  MIGRATION_COUNT=$(find "$REPO_DIR/migrations" -name '*.sql' | wc -l | tr -d ' ')
  echo "Built. $MIGRATION_COUNT migrations embedded."
}

do_clean() {
  ssh_run "rm -rf ${INSTALL_DIR}; true"
  # The app's own connection pool can hold sessions open past a "stopped"
  # service (systemd stop can race the pool closing), so force-drop rather
  # than a plain DROP DATABASE, which errors if anything is still connected.
  local drop_out
  drop_out=$(ssh_run "sudo -u postgres psql -tAc \"SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'\" | grep -q 1 && sudo -u postgres psql -c 'DROP DATABASE ${DB_NAME} WITH (FORCE);' 2>&1; true")
  echo "$drop_out"
  if echo "$drop_out" | grep -qi "error"; then
    echo "Failed to drop ${DB_NAME} — check for other active connections/replicas above." >&2
    return 1
  fi
  local role_out
  role_out=$(ssh_run "sudo -u postgres psql -tAc \"SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'\" | grep -q 1 && sudo -u postgres psql -c 'DROP ROLE ${DB_USER};' 2>&1; true")
  echo "$role_out"
  if echo "$role_out" | grep -qi "error"; then
    echo "Failed to drop role ${DB_USER} — check for other objects it still owns above." >&2
    return 1
  fi
  # .env holds the now-invalid old DB password — remove it too so the DB
  # bootstrap step regenerates both together instead of reusing a stale
  # password against a freshly recreated role.
  ssh_run "rm -f ${INSTALL_DIR}/.env"
  echo "Old install, DB, and .env removed."
}

do_db_bootstrap() {
  local db_pass_file_check
  db_pass_file_check=$(ssh_run "test -f ${INSTALL_DIR}/.env && grep -q '^DATABASE_URL=' ${INSTALL_DIR}/.env && echo yes || echo no")
  if [[ "$db_pass_file_check" == "yes" ]]; then
    DATABASE_URL=$(ssh_run "grep '^DATABASE_URL=' ${INSTALL_DIR}/.env | cut -d= -f2-")
    echo "Existing .env found on VPS — reusing DATABASE_URL."
  else
    local db_pass
    db_pass=$(openssl rand -hex 16)
    DATABASE_URL="postgres://${DB_USER}:${db_pass}@localhost:5432/${DB_NAME}"
    local sql_tmp="/tmp/ss-install-db-$$.sql"
    cat > "$sql_tmp" <<SQL
DO \$\$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '${DB_USER}') THEN
    EXECUTE format('CREATE ROLE ${DB_USER} LOGIN PASSWORD %L', '${db_pass}');
  ELSE
    EXECUTE format('ALTER ROLE ${DB_USER} WITH LOGIN PASSWORD %L', '${db_pass}');
  END IF;
END \$\$;
SELECT 'CREATE DATABASE ${DB_NAME} OWNER ${DB_USER}'
  WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '${DB_NAME}') \gexec
GRANT ALL PRIVILEGES ON DATABASE ${DB_NAME} TO ${DB_USER};
SQL
    scp_run "$sql_tmp" "${VPS_USER}@${VPS_HOST}:/tmp/ss-install-db.sql"
    rm -f "$sql_tmp"
    ssh_run "sudo -u postgres psql -f /tmp/ss-install-db.sql && rm -f /tmp/ss-install-db.sql"
    echo "Database ready."
  fi
}

do_ship_files() {
  ssh_run "systemctl stop synapcms 2>/dev/null; systemctl disable synapcms 2>/dev/null; true"
  ssh_run "mkdir -p ${INSTALL_DIR}/uploads ${INSTALL_DIR}/search-index ${INSTALL_DIR}/themes/sites ${INSTALL_DIR}/plugins/sites"

  scp_run "$BIN_SYNAPTIC" "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synaptic"
  scp_run "$BIN_CLI"      "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synap"

  ssh_run "mkdir -p ${INSTALL_DIR}/admin"
  local assets_tmp="/tmp/ss-install-assets-$$.tar.gz"
  tar czf "$assets_tmp" -C "$REPO_DIR" themes plugins -C "$REPO_DIR/admin" static
  scp_run "$assets_tmp" "${VPS_USER}@${VPS_HOST}:/tmp/ss-install-assets.tar.gz"
  rm -f "$assets_tmp"
  ssh_run "tar xzf /tmp/ss-install-assets.tar.gz -C ${INSTALL_DIR} --overwrite themes plugins 2>/dev/null; \
           mkdir -p ${INSTALL_DIR}/admin && tar xzf /tmp/ss-install-assets.tar.gz -C ${INSTALL_DIR}/admin --overwrite static 2>/dev/null; \
           rm -f /tmp/ss-install-assets.tar.gz"

  ssh_run "chmod +x ${INSTALL_DIR}/synaptic ${INSTALL_DIR}/synap"
  ssh_run "ln -sf ${INSTALL_DIR}/synap /usr/local/bin/synap"
  ssh_run "chown -R ${SYNAPTIC_USER}:${SYNAPTIC_USER} ${INSTALL_DIR}"

  # SELinux context (AlmaLinux) — matches install.sh behavior.
  ssh_run "command -v chcon >/dev/null && chcon -Rt var_t ${INSTALL_DIR} 2>/dev/null; \
           command -v chcon >/dev/null && chcon -t bin_t ${INSTALL_DIR}/synaptic ${INSTALL_DIR}/synap 2>/dev/null; true"
  echo "Files copied."
}

do_write_env() {
  local env_exists
  env_exists=$(ssh_run "test -f ${INSTALL_DIR}/.env && echo yes || echo no")
  if [[ "$env_exists" == "no" ]]; then
    local secret_key
    secret_key=$(openssl rand -hex 32)
    ssh_run "cat > ${INSTALL_DIR}/.env <<ENVEOF
DATABASE_URL=${DATABASE_URL}
SECRET_KEY=${secret_key}
HOST=0.0.0.0
PORT=${APP_PORT}
LOG_LEVEL=info
INSTALL_DIR=${INSTALL_DIR}
ENVEOF
chown ${SYNAPTIC_USER}:${SYNAPTIC_USER} ${INSTALL_DIR}/.env
chmod 600 ${INSTALL_DIR}/.env"
    echo "Fresh .env written."
  else
    echo "Existing .env preserved."
  fi
}

# Static keyword scan of not-yet-applied migrations, warning about anything
# that looks destructive (DROP/TRUNCATE/DELETE/RENAME) before it actually
# runs. Only meaningful against a database that already has real data —
# skipped on --clean (fresh DB) or a genuinely first-ever install (no
# _sqlx_migrations rows yet), where there's nothing to lose either way.
# This is a heuristic, not a guarantee: it flags statements worth a second
# look, not a certified list of what's actually unsafe (a `DROP TABLE IF
# EXISTS` on an always-empty scratch table is a common, harmless false
# positive).
check_migration_risk() {
  local applied_count
  applied_count=$(ssh_run "sudo -u postgres psql ${DB_NAME} -tAc \"SELECT COUNT(*) FROM _sqlx_migrations\" 2>/dev/null" | tr -d '[:space:]')
  if [[ -z "$applied_count" || "$applied_count" -eq 0 ]]; then
    return 0
  fi

  local applied_versions
  applied_versions=$(ssh_run "sudo -u postgres psql ${DB_NAME} -tAc \"SELECT version FROM _sqlx_migrations\" 2>/dev/null")

  local f base version is_applied v risky_lines
  local -a flagged=()
  for f in "$REPO_DIR"/migrations/*.sql; do
    base=$(basename "$f")
    version=$(echo "$base" | grep -oE '^[0-9]+' | sed 's/^0*//')
    [[ -z "$version" ]] && version=0

    is_applied=0
    while IFS= read -r v; do
      [[ "$v" == "$version" ]] && { is_applied=1; break; }
    done <<< "$applied_versions"
    [[ "$is_applied" -eq 1 ]] && continue

    risky_lines=$(grep -inE 'drop[[:space:]]+table|drop[[:space:]]+column|truncate|delete[[:space:]]+from|rename[[:space:]]+column|rename[[:space:]]+to' "$f" || true)
    if [[ -n "$risky_lines" ]]; then
      flagged+=("$base")
      echo -e "  ${C_YELLOW}${C_BOLD}⚠${C_RESET} ${base}"
      while IFS= read -r line; do
        echo "      $line"
      done <<< "$risky_lines"
    fi
  done

  if [[ "${#flagged[@]}" -gt 0 ]]; then
    echo ""
    echo "  Static keyword scan only — review before proceeding. Common false"
    echo "  positive: 'DROP TABLE IF EXISTS' on a table that's always empty."
    warn "${#flagged[@]} pending migration(s) flagged as potentially destructive: ${flagged[*]}"
    if [[ -t 0 ]]; then
      if ! prompt_yes_no "Continue anyway?" "n"; then
        echo ""
        log "Aborted before applying migrations. Files were already shipped to the VPS,"
        log "but the database was not touched — re-run once you've reviewed the SQL above."
        exit 0
      fi
    fi
  fi
}

do_install_or_migrate() {
  if [[ "$UPDATE_ONLY" -eq 1 ]]; then
    ssh_run "sudo -u ${SYNAPTIC_USER} DATABASE_URL='${DATABASE_URL}' ${INSTALL_DIR}/synap migrate" \
      || { echo "synap migrate failed." >&2; return 1; }
    echo "Migrations applied. No site/admin configured yet."

    local caddy_tmp svc_tmp
    caddy_tmp="/tmp/ss-install-caddyfile-$$"
    svc_tmp="/tmp/ss-install-service-$$"
    sed -e "s#{DOMAIN}#${VPS_DOMAIN}#g" \
        -e "s#{PORT}#${APP_PORT}#g" \
        -e "s#{UPLOADS_DIR}#${INSTALL_DIR}/uploads#g" \
        -e "s#{THEME_DIR}#${INSTALL_DIR}/themes#g" \
        "$REPO_DIR/deployment/Caddyfile.template" > "$caddy_tmp"
    sed -e "s#{INSTALL_DIR}#${INSTALL_DIR}#g" \
        -e "s#{SERVICE_USER}#${SYNAPTIC_USER}#g" \
        "$REPO_DIR/deployment/synapcms.service" > "$svc_tmp"
    scp_run "$caddy_tmp" "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/Caddyfile"
    scp_run "$svc_tmp"   "${VPS_USER}@${VPS_HOST}:${INSTALL_DIR}/synapcms.service"
    rm -f "$caddy_tmp" "$svc_tmp"
    echo "Templates generated."
  else
    local admin_pw_env=""
    [[ -n "$ADMIN_PASSWORD" ]] && admin_pw_env="ADMIN_PASSWORD='${ADMIN_PASSWORD}'"
    # --clean already wiped INSTALL_DIR + the DB, so it means "take over
    # completely" — declare that up front. Without this, a leftover
    # /etc/caddy/Caddyfile block for VPS_DOMAIN from a prior install (one
    # that predates the managed-block markers, or belonging to a different
    # INSTALL_DIR) makes the remote preflight bail non-interactively, since
    # `synap install` never defaults to a destructive choice on its own.
    local on_conflict_flag=""
    [[ "$CLEAN" -eq 1 ]] && on_conflict_flag="--on-conflict=fresh"
    local cli_output
    cli_output=$(ssh_run "sudo -u ${SYNAPTIC_USER} \
      DATABASE_URL='${DATABASE_URL}' \
      PORT='${APP_PORT}' \
      INSTALL_DIR='${INSTALL_DIR}' \
      APP_NAME='${APP_NAME}' \
      SYNAPTIC_DOMAIN='${VPS_DOMAIN}' \
      SITE_URL='https://${VPS_DOMAIN}' \
      ADMIN_EMAIL='${ADMIN_EMAIL}' \
      ADMIN_USERNAME='${ADMIN_USERNAME}' \
      ${admin_pw_env} \
      ${INSTALL_DIR}/synap install --non-interactive --output-dir ${INSTALL_DIR} ${on_conflict_flag} 2>&1") \
      || { echo "synap install failed:"; echo "$cli_output"; return 1; }
    echo "$cli_output"

    if [[ -n "$ADMIN_PASSWORD" ]]; then
      SUMMARY_ADMIN_PASSWORD="$ADMIN_PASSWORD"
    elif echo "$cli_output" | grep -q "^GENERATED_ADMIN_PASSWORD="; then
      SUMMARY_ADMIN_PASSWORD=$(echo "$cli_output" | grep "^GENERATED_ADMIN_PASSWORD=" | cut -d= -f2-)
    else
      SUMMARY_ADMIN_PASSWORD=""
    fi
    echo "Install complete."
  fi
}

do_caddy_systemd() {
  ssh_run "mkdir -p /var/log/caddy && chown caddy:caddy /var/log/caddy 2>/dev/null || true"
  ssh_run "test -f ${INSTALL_DIR}/Caddyfile && cp ${INSTALL_DIR}/Caddyfile /etc/caddy/Caddyfile"
  ssh_run "systemctl is-active --quiet caddy && caddy reload --config /etc/caddy/Caddyfile || systemctl enable --now caddy"

  ssh_run "test -f ${INSTALL_DIR}/synapcms.service && cp ${INSTALL_DIR}/synapcms.service /etc/systemd/system/synapcms.service"
  ssh_run "systemctl daemon-reload && systemctl enable synapcms && systemctl restart synapcms"
  sleep 3
  echo "Web server and service configured."
}

do_verify() {
  SUMMARY_SERVICE_STATUS=$(ssh_run "systemctl is-active synapcms" || true)
  if [[ "$SUMMARY_SERVICE_STATUS" != "active" ]]; then
    echo "synapcms is '${SUMMARY_SERVICE_STATUS}' — recent logs:"
    ssh_run "journalctl -u synapcms -n 30 --no-pager"
    return 1
  fi

  SUMMARY_MIGRATION_COUNT=$(ssh_run "sudo -u postgres psql ${DB_NAME} -tAc 'SELECT count(*) FROM _sqlx_migrations' 2>&1" | tr -d '[:space:]')

  SUMMARY_HTTP_CODE=$(ssh_run "curl -s -o /dev/null -w '%{http_code}' -H 'Host: ${VPS_DOMAIN}' http://localhost:${APP_PORT}/ 2>&1")
  if [[ "$UPDATE_ONLY" -eq 1 ]]; then
    if [[ ! "$SUMMARY_HTTP_CODE" =~ ^(200|30[0-9]|404)$ ]]; then
      echo "App did not respond as expected (got HTTP ${SUMMARY_HTTP_CODE})." >&2
    fi
  else
    if [[ ! "$SUMMARY_HTTP_CODE" =~ ^(200|30[0-9])$ ]]; then
      echo "Local HTTP check did not return 200/3xx (got ${SUMMARY_HTTP_CODE})." >&2
    fi
  fi
  echo "Service active. ${SUMMARY_MIGRATION_COUNT} migrations applied. HTTP ${SUMMARY_HTTP_CODE}."
}

# ── Summary ──────────────────────────────────────────────────────────────────
print_summary() {
  box_header "SynapCMS — Installation Complete"
  echo ""
  echo "  Site:        https://${VPS_DOMAIN}"
  echo "  Admin panel: https://${VPS_DOMAIN}/admin"

  if [[ "$UPDATE_ONLY" -eq 1 ]]; then
    section "Next Steps"
    echo "  Site/admin were not configured. On the VPS, run (as ${SYNAPTIC_USER} —"
    echo "  synap checks that it owns \$INSTALL_DIR; use the full path, not the"
    echo "  bare command, since sudo's secure_path on RHEL/AlmaLinux drops"
    echo "  /usr/local/bin):"
    echo ""
    echo "    sudo -u ${SYNAPTIC_USER} bash -c 'cd ${INSTALL_DIR} && SITE_URL=https://${VPS_DOMAIN} ./synap install'"
    echo ""
    echo "  (SITE_URL matters if Caddy fronts this on 443 — without it, site_url"
    echo "   defaults to http://domain:${APP_PORT}, baking the internal port into"
    echo "   permalinks. Answer the prompts for domain, admin email/username/"
    echo "   password, etc. This also regenerates the Caddyfile/systemd unit for"
    echo "   the values you choose — re-run this script, or 'systemctl reload"
    echo "   caddy' + 'systemctl restart synapcms', afterwards to pick"
    echo "   them up.)"
  else
    section "Admin Login"
    echo "  Username: ${ADMIN_USERNAME}"
    echo "  Email:    ${ADMIN_EMAIL}"
    if [[ -n "${SUMMARY_ADMIN_PASSWORD:-}" ]]; then
      echo -e "  Password: ${C_BOLD}${SUMMARY_ADMIN_PASSWORD}${C_RESET}"
      echo ""
      echo -e "  ${C_YELLOW}${C_BOLD}⚠ SAVE THIS PASSWORD NOW — it will not be shown again.${C_RESET}"
    else
      echo "  Password: (not captured — check the install output above)"
    fi
  fi

  section "Status"
  echo "  Service:    ${SUMMARY_SERVICE_STATUS:-unknown}"
  echo "  Migrations: ${SUMMARY_MIGRATION_COUNT:-unknown} applied"
  echo "  HTTP check: ${SUMMARY_HTTP_CODE:-unknown}"

  if [[ "${#WARNINGS[@]}" -gt 0 ]]; then
    section "Warnings"
    local w
    for w in "${WARNINGS[@]}"; do
      echo "  - $w"
    done
  fi
  echo ""
}

# ── Main ─────────────────────────────────────────────────────────────────────
main() {
  if [[ -z "$MODE" ]]; then
    welcome_screen
  fi

  if [[ "$MODE" == "interactive" ]]; then
    run_interactive_wizard
  fi

  review_and_confirm
  define_ssh_helpers

  check_requirements
  gate_on_requirements

  run_step "Building release binaries..." do_build
  if [[ "$CLEAN" -eq 1 ]]; then
    run_step "Cleaning previous install..." do_clean
  fi
  run_step "Setting up the database..." do_db_bootstrap
  run_step "Copying application files..." do_ship_files
  run_step "Writing configuration..." do_write_env

  if [[ "$CLEAN" -eq 0 ]]; then
    log "Checking pending migrations for destructive statements..."
    check_migration_risk
  fi

  if [[ "$UPDATE_ONLY" -eq 1 ]]; then
    run_step "Applying database migrations..." do_install_or_migrate
  else
    run_step "Installing site & admin account..." do_install_or_migrate
  fi
  run_step "Configuring web server & service..." do_caddy_systemd
  run_step "Verifying deployment..." do_verify

  print_summary
}

main
