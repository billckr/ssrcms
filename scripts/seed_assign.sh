#!/usr/bin/env bash
# seed_assign.sh — Randomly assign existing categories/tags to existing posts/pages.
#
# Usage:
#   ./scripts/seed_assign.sh -domain beth.com
#   ./scripts/seed_assign.sh -domain beth.com -type page -percent 75
#
# Reads DATABASE_URL from .env in the project root if not already set.

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
TYPE="both"
PERCENT=50

usage() {
    cat <<EOF
Usage: $(basename "$0") -domain <hostname> [options]

Randomly assign existing categories and tags to existing posts/pages for a site.
Pulls whatever taxonomies already exist in the DB for the site — run
seed_posts.sh -extras first if you need to create them.

Required:
  -domain <hostname>        Site hostname (e.g. beth.com)

Options:
  -type   post|page|both    Limit to a specific content type (default: both)
  -percent <n>              Percentage of eligible items to assign to (default: 50)

Examples:
  $(basename "$0") -domain beth.com
  $(basename "$0") -domain beth.com -type post -percent 75
  $(basename "$0") -domain beth.com -type page -percent 100

Notes:
  - Only assigns to items that do not already have a taxonomy attached.
  - Each selected item gets one randomly chosen category or tag.
  - ON CONFLICT DO NOTHING — safe to run multiple times.
  - Reads DATABASE_URL from .env in the project root if not set in the environment.
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help) usage ;;
        -domain)   DOMAIN="$2";   shift 2 ;;
        -type)     TYPE="$2";     shift 2 ;;
        -percent)  PERCENT="$2";  shift 2 ;;
        *) usage ;;
    esac
done

[[ -z "$DOMAIN" ]] && usage

if [[ "$TYPE" != "post" && "$TYPE" != "page" && "$TYPE" != "both" ]]; then
    echo "ERROR: -type must be 'post', 'page', or 'both'" >&2
    exit 1
fi

if ! [[ "$PERCENT" =~ ^[0-9]+$ ]] || [[ "$PERCENT" -lt 1 || "$PERCENT" -gt 100 ]]; then
    echo "ERROR: -percent must be an integer between 1 and 100" >&2
    exit 1
fi

# ── Helpers ────────────────────────────────────────────────────────────────────
psql() { command psql "$DATABASE_URL" --tuples-only --no-align "$@"; }

# ── Resolve site ───────────────────────────────────────────────────────────────
SITE_ID=$(psql -c "SELECT id FROM sites WHERE hostname = '$DOMAIN' LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$SITE_ID" ]]; then
    echo "ERROR: No site found with hostname '$DOMAIN'" >&2
    exit 1
fi

echo "Site: $DOMAIN  ($SITE_ID)"
echo ""

# ── Load existing taxonomies for this site ─────────────────────────────────────
mapfile -t TAX_ROWS < <(psql -c \
    "SELECT id || ':' || taxonomy || ':' || name
     FROM taxonomies
     WHERE site_id = '$SITE_ID'
     ORDER BY taxonomy, name;")

TAX_COUNT=${#TAX_ROWS[@]}
if [[ "$TAX_COUNT" -eq 0 ]]; then
    echo "ERROR: No categories or tags found for '$DOMAIN'." >&2
    echo "       Run seed_posts.sh -extras first to create some." >&2
    exit 1
fi

echo "Found $TAX_COUNT taxonomies:"
for ROW in "${TAX_ROWS[@]}"; do
    TTYPE="${ROW#*:}"; TTYPE="${TTYPE%%:*}"
    TNAME="${ROW##*:}"
    printf "  %-10s  %s\n" "$TTYPE" "$TNAME"
done
echo ""

# ── Load posts/pages that have no taxonomy assigned yet ───────────────────────
if [[ "$TYPE" == "both" ]]; then
    TYPE_FILTER="post_type IN ('post', 'page')"
else
    TYPE_FILTER="post_type = '$TYPE'"
fi

mapfile -t POST_IDS < <(psql -c \
    "SELECT p.id
     FROM posts p
     WHERE p.site_id = '$SITE_ID'
       AND $TYPE_FILTER
       AND NOT EXISTS (
           SELECT 1 FROM post_taxonomies pt WHERE pt.post_id = p.id
       )
     ORDER BY p.created_at;")

TOTAL=${#POST_IDS[@]}
if [[ "$TOTAL" -eq 0 ]]; then
    echo "No unassigned items found for type '$TYPE' — nothing to do."
    exit 0
fi

TARGET=$(( (TOTAL * PERCENT + 99) / 100 ))   # ceiling division

echo "Eligible items (no taxonomy yet): $TOTAL"
echo "Assigning to $TARGET ($PERCENT%)..."
echo ""

# ── Shuffle and assign ─────────────────────────────────────────────────────────
mapfile -t SHUFFLED < <(printf '%s\n' "${POST_IDS[@]}" | shuf)
TARGETS=("${SHUFFLED[@]:0:$TARGET}")

ASSIGNED=0
for PID in "${TARGETS[@]}"; do
    ROW="${TAX_ROWS[$((RANDOM % TAX_COUNT))]}"
    TID="${ROW%%:*}"
    TTYPE="${ROW#*:}"; TTYPE="${TTYPE%%:*}"
    TNAME="${ROW##*:}"

    SQL="INSERT INTO post_taxonomies (post_id, taxonomy_id)
         VALUES ('$PID', '$TID')
         ON CONFLICT DO NOTHING;"
    if command psql "$DATABASE_URL" -c "$SQL" > /dev/null 2>&1; then
        printf "  assigned  %-10s  %s\n" "$TTYPE" "$TNAME"
        ASSIGNED=$((ASSIGNED + 1))
    fi
done

echo ""
echo "Done. $ASSIGNED assignments made."
