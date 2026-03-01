#!/usr/bin/env bash
# seed_posts.sh — Insert random posts for a given site and author.
#
# Usage:
#   ./scripts/seed_posts.sh -domain beth.com -user beth@beth.com -number 25
#
# Reads DATABASE_URL from .env in the project root if not already set.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# ── Load .env if DATABASE_URL not already in environment ──────────────────────
if [[ -z "${DATABASE_URL:-}" && -f "$PROJECT_ROOT/.env" ]]; then
    # Export only the DATABASE_URL line, ignoring comments.
    export DATABASE_URL
    DATABASE_URL=$(grep -E '^DATABASE_URL=' "$PROJECT_ROOT/.env" | cut -d= -f2-)
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set and could not be read from .env" >&2
    exit 1
fi

# ── Argument parsing ───────────────────────────────────────────────────────────
DOMAIN=""
USER_EMAIL=""
NUMBER=10
PORT=""
FORCE_STATUS=""
TYPE="post"
EXTRAS=0

usage() {
    cat <<EOF
Usage: $(basename "$0") -domain <hostname> -user <email> [options]

Create random seed posts for a specific site and author.

Required:
  -domain <hostname>        Site hostname (e.g. beth.com)
  -user   <email>           Author email address (e.g. beth@beth.com)

Options:
  -number <n>               Number of items to create (default: 10)
  -type   post|page         Content type to create (default: post)
  -status published|draft   Force all items to a specific status.
                            Without this flag items are randomly mixed
                            (~60% published, ~40% draft).
  -extras                   Create random categories and tags and assign them
                            to ~50% of the inserted items (randomly chosen).
  -port   <port>            Port to include in the printed URLs (e.g. 3000).
                            Only affects output — not stored in the database.

Examples:
  $(basename "$0") -domain beth.com -user beth@beth.com -number 25
  $(basename "$0") -domain beth.com -user beth@beth.com -number 25 -port 3000
  $(basename "$0") -domain beth.com -user beth@beth.com -number 10 -status published -port 3000
  $(basename "$0") -domain beth.com -user beth@beth.com -number 5  -status draft
  $(basename "$0") -domain beth.com -user beth@beth.com -number 5  -type page
  $(basename "$0") -domain beth.com -user beth@beth.com -number 50 -extras

Notes:
  - Reads DATABASE_URL from .env in the project root if not set in the environment.
  - Slugs include a random 4-character suffix to avoid collisions on repeat runs.
  - Published posts are assigned a random date within the last 12 months.
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help) usage ;;
        -domain)   DOMAIN="$2";        shift 2 ;;
        -user)     USER_EMAIL="$2";    shift 2 ;;
        -number)   NUMBER="$2";        shift 2 ;;
        -port)     PORT="$2";          shift 2 ;;
        -status)   FORCE_STATUS="$2";  shift 2 ;;
        -type)     TYPE="$2";          shift 2 ;;
        -extras)   EXTRAS=1;           shift 1 ;;
        *) usage ;;
    esac
done

if [[ -n "$FORCE_STATUS" && "$FORCE_STATUS" != "published" && "$FORCE_STATUS" != "draft" ]]; then
    echo "ERROR: -status must be 'published' or 'draft'" >&2
    exit 1
fi

if [[ "$TYPE" != "post" && "$TYPE" != "page" ]]; then
    echo "ERROR: -type must be 'post' or 'page'" >&2
    exit 1
fi

[[ -z "$DOMAIN" || -z "$USER_EMAIL" ]] && usage

if ! [[ "$NUMBER" =~ ^[0-9]+$ ]] || [[ "$NUMBER" -lt 1 ]]; then
    echo "ERROR: -number must be a positive integer" >&2
    exit 1
fi

# ── Resolve site_id and user_id ────────────────────────────────────────────────
psql() { command psql "$DATABASE_URL" --tuples-only --no-align "$@"; }

SITE_ID=$(psql -c "SELECT id FROM sites WHERE hostname = '$DOMAIN' LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$SITE_ID" ]]; then
    echo "ERROR: No site found with hostname '$DOMAIN'" >&2
    exit 1
fi

USER_ID=$(psql -c "SELECT id FROM users WHERE email = '$USER_EMAIL' AND deleted_at IS NULL LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$USER_ID" ]]; then
    echo "ERROR: No user found with email '$USER_EMAIL'" >&2
    exit 1
fi

# Check user is a super_admin OR has a role on this specific site.
IS_SUPER=$(psql -c "SELECT 1 FROM users WHERE id = '$USER_ID' AND role = 'super_admin' LIMIT 1;" | tr -d '[:space:]')
IS_MEMBER=$(psql -c "SELECT 1 FROM site_users WHERE site_id = '$SITE_ID' AND user_id = '$USER_ID' LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$IS_SUPER" && -z "$IS_MEMBER" ]]; then
    echo "ERROR: User '$USER_EMAIL' has no access to site '$DOMAIN'" >&2
    echo "       Add the user to the site first, or use a super_admin account." >&2
    exit 1
fi

echo "Site:   $DOMAIN  ($SITE_ID)"
echo "Author: $USER_EMAIL  ($USER_ID)"
echo "Type:   $TYPE"
echo "Count:  $NUMBER"
echo ""

# ── Word banks for random content ─────────────────────────────────────────────
ADJECTIVES=(
    "Quick" "Lazy" "Bright" "Dark" "Modern" "Ancient" "Silent" "Loud"
    "Hidden" "Bold" "Clever" "Simple" "Complex" "Fresh" "Wild" "Calm"
    "Sharp" "Soft" "Vast" "Narrow" "Golden" "Silver" "Rustic" "Digital"
)
NOUNS=(
    "Guide" "Journey" "Story" "Vision" "Future" "Secret" "Path" "World"
    "Truth" "Dream" "Plan" "Theory" "Chapter" "Moment" "Change" "Force"
    "Light" "Shadow" "Wave" "Edge" "Bridge" "Signal" "Layer" "Canvas"
)
TOPICS=(
    "Technology" "Design" "Nature" "Travel" "Food" "Music" "Science"
    "History" "Culture" "Business" "Health" "Education" "Art" "Sport"
    "Finance" "Philosophy" "Architecture" "Photography" "Writing" "Code"
)
STATUSES=("published" "published" "published" "draft" "draft")

rand_element() {
    local -n arr=$1
    echo "${arr[$((RANDOM % ${#arr[@]}))]}"
}

# ── Insert posts ───────────────────────────────────────────────────────────────
SUCCESS=0
SKIPPED=0
POST_IDS=()

for ((i = 1; i <= NUMBER; i++)); do
    ADJ=$(rand_element ADJECTIVES)
    NOUN=$(rand_element NOUNS)
    TOPIC=$(rand_element TOPICS)
    STATUS="${FORCE_STATUS:-$(rand_element STATUSES)}"

    TITLE="$ADJ $NOUN of $TOPIC"
    # Slug: lowercase, spaces to hyphens, append random 4-char suffix to avoid collisions
    SUFFIX=$(cat /proc/sys/kernel/random/uuid | tr -d '-' | head -c 4)
    SLUG=$(echo "$TITLE" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g; s/--*/-/g; s/^-//; s/-$//')-$SUFFIX

    # Random publish date within the last 12 months
    DAYS_AGO=$(( RANDOM % 365 ))
    PUBLISHED_AT="NOW() - INTERVAL '$DAYS_AGO days'"
    [[ "$STATUS" == "draft" ]] && PUBLISHED_AT="NULL"

    CONTENT="<p>This is a sample post about <strong>$TOPIC</strong>. $(
        echo "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris. \
This article explores the $ADJ aspects of $TOPIC from a fresh perspective."
    )</p>
<p>Pellentesque habitant morbi tristique senectus et netus et malesuada fames. \
Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.</p>"

    EXCERPT="A $ADJ look at $TOPIC — exploring $NOUN and beyond."

    SQL="INSERT INTO posts (title, slug, content, excerpt, status, post_type, author_id, site_id, published_at)
         VALUES (
             '$TITLE',
             '$SLUG',
             '$CONTENT',
             '$EXCERPT',
             '$STATUS',
             '$TYPE',
             '$USER_ID',
             '$SITE_ID',
             $PUBLISHED_AT
         ) RETURNING id;"

    POST_ID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c "$SQL" 2>/dev/null | tr -d '[:space:]')
    if [[ -n "$POST_ID" ]]; then
        POST_IDS+=("$POST_ID")
        PORT_PART="${PORT:+:$PORT}"
        if [[ "$TYPE" == "page" ]]; then
            URL="http://${DOMAIN}${PORT_PART}/${SLUG}"
        else
            URL="http://${DOMAIN}${PORT_PART}/blog/${SLUG}"
        fi
        printf "  [%3d/%d] %-10s  %s\n" "$i" "$NUMBER" "$STATUS" "$URL"
        SUCCESS=$((SUCCESS + 1))
    else
        printf "  [%3d/%d] SKIPPED (slug collision?) — %s\n" "$i" "$NUMBER" "$TITLE" >&2
        SKIPPED=$((SKIPPED + 1))
    fi
done

echo ""
echo "Done. $SUCCESS inserted, $SKIPPED skipped."

# ── Extras: create random categories/tags and assign to ~50% of inserts ────────
if [[ "$EXTRAS" == "1" && "$SUCCESS" -gt 0 ]]; then
    echo ""
    echo "Creating extras..."

    CAT_NAMES=("Technology" "Design" "Business" "Lifestyle" "Tutorial")
    TAG_NAMES=("featured" "popular" "tips" "beginner" "advanced")

    TAX_IDS=()

    for CAT in "${CAT_NAMES[@]}"; do
        CAT_SLUG=$(echo "$CAT" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g; s/--*/-/g; s/^-//; s/-$//')
        SQL="INSERT INTO taxonomies (name, slug, taxonomy, site_id)
             VALUES ('$CAT', '$CAT_SLUG', 'category', '$SITE_ID')
             ON CONFLICT (site_id, slug, taxonomy) DO NOTHING
             RETURNING id;"
        ID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c "$SQL" 2>/dev/null | tr -d '[:space:]')
        # ON CONFLICT returns nothing — fetch the existing row
        if [[ -z "$ID" ]]; then
            ID=$(psql -c "SELECT id FROM taxonomies WHERE site_id = '$SITE_ID' AND slug = '$CAT_SLUG' AND taxonomy = 'category' LIMIT 1;" | tr -d '[:space:]')
        fi
        if [[ -n "$ID" ]]; then
            TAX_IDS+=("category:$CAT:$ID")
            printf "  category  %s\n" "$CAT"
        fi
    done

    for TAG in "${TAG_NAMES[@]}"; do
        TAG_SLUG=$(echo "$TAG" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g; s/--*/-/g; s/^-//; s/-$//')
        SQL="INSERT INTO taxonomies (name, slug, taxonomy, site_id)
             VALUES ('$TAG', '$TAG_SLUG', 'tag', '$SITE_ID')
             ON CONFLICT (site_id, slug, taxonomy) DO NOTHING
             RETURNING id;"
        ID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c "$SQL" 2>/dev/null | tr -d '[:space:]')
        if [[ -z "$ID" ]]; then
            ID=$(psql -c "SELECT id FROM taxonomies WHERE site_id = '$SITE_ID' AND slug = '$TAG_SLUG' AND taxonomy = 'tag' LIMIT 1;" | tr -d '[:space:]')
        fi
        if [[ -n "$ID" ]]; then
            TAX_IDS+=("tag:$TAG:$ID")
            printf "  tag       %s\n" "$TAG"
        fi
    done

    TAX_COUNT=${#TAX_IDS[@]}
    if [[ "$TAX_COUNT" -eq 0 ]]; then
        echo "  No taxonomies available — skipping assignments."
    else
        # Shuffle post IDs and pick the first half
        HALF=$(( (SUCCESS + 1) / 2 ))
        mapfile -t SHUFFLED < <(printf '%s\n' "${POST_IDS[@]}" | shuf)
        TARGETS=("${SHUFFLED[@]:0:$HALF}")

        echo ""
        echo "Assigning to $HALF of $SUCCESS items..."
        ASSIGNED=0
        for PID in "${TARGETS[@]}"; do
            # Pick a random taxonomy entry (format: "type:name:uuid")
            ENTRY="${TAX_IDS[$((RANDOM % TAX_COUNT))]}"
            TID="${ENTRY##*:}"
            TLABEL="${ENTRY%:*}"   # "type:name"
            TTYPE="${TLABEL%%:*}"
            TNAME="${TLABEL#*:}"
            SQL="INSERT INTO post_taxonomies (post_id, taxonomy_id)
                 VALUES ('$PID', '$TID')
                 ON CONFLICT DO NOTHING;"
            if command psql "$DATABASE_URL" -c "$SQL" > /dev/null 2>&1; then
                printf "  assigned  %-10s  %s\n" "$TTYPE" "$TNAME"
                ASSIGNED=$((ASSIGNED + 1))
            fi
        done
        echo ""
        echo "Extras done. $ASSIGNED assignments made."
    fi
fi
