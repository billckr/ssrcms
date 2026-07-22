#!/usr/bin/env bash
# seed_users.sh — Create fake users and assign them a role on a given site.
#
# Usage:
#   ./scripts/seed_users.sh -domain example.com -role admin -number 5
#
# Reads DATABASE_URL from .env in the project root if not already set.
# Password hashing is delegated to `synap-cli user hash-password` so the
# stored hash matches exactly what the app produces (Argon2).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# ── Load .env if DATABASE_URL not already in environment ──────────────────────
if [[ -z "${DATABASE_URL:-}" && -f "$PROJECT_ROOT/.env" ]]; then
    export DATABASE_URL
    DATABASE_URL=$(grep -E '^DATABASE_URL=' "$PROJECT_ROOT/.env" | cut -d= -f2-)
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set and could not be read from .env" >&2
    exit 1
fi

# ── Argument parsing ───────────────────────────────────────────────────────────
DOMAIN=""
ROLE=""
NUMBER=5
PASSWORD=""
PORT=""

usage() {
    cat <<EOF
Usage: $(basename "$0") -domain <hostname> -role <role> [options]

Create fake users (random username, display name, email, password) and
assign them a role on a specific site.

Required:
  -domain <hostname>   Site hostname (e.g. example.com)
  -role   <role>       admin | editor | author | subscriber

Options:
  -number   <n>        Number of users to create (default: 5)
  -password <pw>       Use this password for every created user instead of a
                        random one per user. Must satisfy the same rules as
                        the admin UI: 8-12 chars, 1 uppercase, 1 number,
                        1 symbol (! @ # \$ % &).
  -port     <port>     Port to include in the printed login URL (e.g. 3000).
                        Only affects output — not stored in the database.

Examples:
  $(basename "$0") -domain example.com -role admin -number 5
  $(basename "$0") -domain example.com -role author -number 20 -port 3000
  $(basename "$0") -domain example.com -role subscriber -number 10 -password Passw0rd!

Notes:
  - Reads DATABASE_URL from .env in the project root if not set in the environment.
  - Usernames/emails include a random suffix to avoid collisions with existing users.
  - Emails are generated as <username>@<domain>.
  - "admin" here means the site_users role; the underlying users.role is stored
    as "site_admin" to match how the admin UI creates site admins.
  - Requires a built synap-cli binary (debug or release) — built automatically
    if missing.
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)  usage ;;
        -domain)    DOMAIN="$2";   shift 2 ;;
        -role)      ROLE="$2";     shift 2 ;;
        -number)    NUMBER="$2";   shift 2 ;;
        -password)  PASSWORD="$2"; shift 2 ;;
        -port)      PORT="$2";     shift 2 ;;
        *) usage ;;
    esac
done

[[ -z "$DOMAIN" ]] && usage
[[ -z "$ROLE" ]] && usage

if [[ "$ROLE" != "admin" && "$ROLE" != "editor" && "$ROLE" != "author" && "$ROLE" != "subscriber" ]]; then
    echo "ERROR: -role must be one of: admin, editor, author, subscriber" >&2
    exit 1
fi

if ! [[ "$NUMBER" =~ ^[0-9]+$ ]] || [[ "$NUMBER" -lt 1 ]]; then
    echo "ERROR: -number must be a positive integer" >&2
    exit 1
fi

# ── Resolve synap-cli binary (used only for Argon2 password hashing) ──────────
CLI_BIN="$PROJECT_ROOT/target/release/synap-cli"
if [[ ! -x "$CLI_BIN" ]]; then
    CLI_BIN="$PROJECT_ROOT/target/debug/synap-cli"
fi
if [[ ! -x "$CLI_BIN" ]]; then
    echo "synap-cli binary not found — building it (debug profile)..." >&2
    (cd "$PROJECT_ROOT" && cargo build -p synap-cli --quiet) || {
        echo "ERROR: failed to build synap-cli" >&2
        exit 1
    }
    CLI_BIN="$PROJECT_ROOT/target/debug/synap-cli"
fi

if [[ -n "$PASSWORD" ]]; then
    if ! "$CLI_BIN" user hash-password "$PASSWORD" > /dev/null 2>&1; then
        echo "ERROR: -password does not meet requirements (8-12 chars, 1 uppercase, 1 number, 1 symbol: ! @ # \$ % &)" >&2
        exit 1
    fi
fi

# ── Resolve site_id ─────────────────────────────────────────────────────────────
psql() { command psql "$DATABASE_URL" --tuples-only --no-align "$@"; }

SITE_ID=$(psql -c "SELECT id FROM sites WHERE hostname = '$DOMAIN' LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$SITE_ID" ]]; then
    echo "ERROR: No site found with hostname '$DOMAIN'" >&2
    exit 1
fi

# users.role: "admin" is a site_users concept, stored as "site_admin" in users.role
# (mirrors core/src/handlers/admin/users.rs). super_admin is CLI-install-only,
# so it's intentionally not an option here.
USERS_ROLE="$ROLE"
[[ "$ROLE" == "admin" ]] && USERS_ROLE="site_admin"

echo "Site:   $DOMAIN  ($SITE_ID)"
echo "Role:   $ROLE  (users.role: $USERS_ROLE)"
echo "Count:  $NUMBER"
echo ""

# ── Word banks for fake identities ─────────────────────────────────────────────
FIRST_NAMES=(
    "James" "Mary" "Robert" "Patricia" "John" "Jennifer" "Michael" "Linda"
    "David" "Elizabeth" "William" "Barbara" "Richard" "Susan" "Joseph" "Jessica"
    "Thomas" "Sarah" "Charles" "Karen" "Daniel" "Nancy" "Matthew" "Lisa"
    "Anthony" "Margaret" "Mark" "Betty" "Paul" "Sandra"
)
LAST_NAMES=(
    "Smith" "Johnson" "Williams" "Brown" "Jones" "Garcia" "Miller" "Davis"
    "Rodriguez" "Martinez" "Hernandez" "Lopez" "Gonzalez" "Wilson" "Anderson"
    "Thomas" "Taylor" "Moore" "Jackson" "Martin" "Lee" "Perez" "Thompson"
    "White" "Harris" "Sanchez" "Clark" "Ramirez" "Lewis" "Robinson"
)

rand_element() {
    local -n arr=$1
    echo "${arr[$((RANDOM % ${#arr[@]}))]}"
}

# Generates an 8-char password satisfying validate_password: 1 upper, 1 digit,
# 1 symbol from !@#$%&, rest lowercase — then shuffles character order.
gen_password() {
    local symbols="!@#\$%&"
    local lower="abcdefghijklmnopqrstuvwxyz"
    local upper_char="${lower:$((RANDOM % 26)):1}"
    upper_char="${upper_char^^}"
    local digit_char=$((RANDOM % 10))
    local symbol_char="${symbols:$((RANDOM % 6)):1}"
    local rest=""
    for _ in 1 2 3 4 5; do
        rest+="${lower:$((RANDOM % 26)):1}"
    done
    echo "${upper_char}${digit_char}${symbol_char}${rest}" | fold -w1 | shuf | tr -d '\n'
}

# ── Insert users ────────────────────────────────────────────────────────────────
SUCCESS=0
SKIPPED=0
declare -a CREATED_LINES=()

for ((i = 1; i <= NUMBER; i++)); do
    FIRST=$(rand_element FIRST_NAMES)
    LAST=$(rand_element LAST_NAMES)
    DISPLAY_NAME="$FIRST $LAST"
    SUFFIX=$(cat /proc/sys/kernel/random/uuid | tr -d '-' | head -c 5)
    USERNAME=$(echo "${FIRST}.${LAST}" | tr '[:upper:]' '[:lower:]')-$SUFFIX
    EMAIL="${USERNAME}@${DOMAIN}"

    USER_PASSWORD="$PASSWORD"
    [[ -z "$USER_PASSWORD" ]] && USER_PASSWORD=$(gen_password)

    HASH=$("$CLI_BIN" user hash-password "$USER_PASSWORD")

    USER_ID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c \
        "INSERT INTO users (username, email, display_name, password_hash, role)
         VALUES ('$USERNAME', '$EMAIL', '$DISPLAY_NAME', '$HASH', '$USERS_ROLE')
         RETURNING id;" \
        2>/dev/null | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')

    if [[ -z "$USER_ID" ]]; then
        printf "  [%3d/%d] SKIPPED (username/email collision?) — %s <%s>\n" "$i" "$NUMBER" "$DISPLAY_NAME" "$EMAIL" >&2
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    if ! command psql "$DATABASE_URL" -c \
        "INSERT INTO site_users (site_id, user_id, role) VALUES ('$SITE_ID', '$USER_ID', '$ROLE');" \
        > /dev/null 2>&1
    then
        printf "  [%3d/%d] SKIPPED (site_users insert failed) — %s <%s>\n" "$i" "$NUMBER" "$DISPLAY_NAME" "$EMAIL" >&2
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    printf "  [%3d/%d] %-20s %-30s %s\n" "$i" "$NUMBER" "$DISPLAY_NAME" "$EMAIL" "$USER_PASSWORD"
    CREATED_LINES+=("$EMAIL / $USER_PASSWORD")
    SUCCESS=$((SUCCESS + 1))
done

echo ""
echo "Done. $SUCCESS created, $SKIPPED skipped."

if [[ "$SUCCESS" -gt 0 ]]; then
    PORT_PART="${PORT:+:$PORT}"
    echo ""
    echo "Login at: http://${DOMAIN}${PORT_PART}/admin"
    echo ""
    echo "Credentials (email / password):"
    for LINE in "${CREATED_LINES[@]}"; do
        echo "  $LINE"
    done
fi
