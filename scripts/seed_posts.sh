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
COMMENTS=0
REPLIES=0
CLEAR=0
FORCE=0

usage() {
    cat <<EOF
Usage: $(basename "$0") -domain <hostname> -user <email> [options]
       $(basename "$0") -domain <hostname> -clear [-force]

Seed or clear content for a specific site.

Required (seed mode):
  -domain <hostname>        Site hostname (e.g. beth.com)
  -user   <email>           Author email address (e.g. beth@beth.com)

Required (clear mode):
  -domain <hostname>        Site hostname to clear
  -clear                    Delete all posts, pages, comments, taxonomies,
                            form submissions, and media rows for this site.
                            Users and site settings are NOT affected.
                            Prompts for confirmation unless -force is also set.
  -force                    Skip the confirmation prompt (use in scripts).

Seed options:
  -number   <n>             Number of items to create (default: 10)
  -type     post|page       Content type to create (default: post)
  -status   published|draft|pending
                            Force all items to a specific status.
                            Without this flag items are randomly mixed
                            (~1/3 published, ~1/3 draft, ~1/3 pending).
  -extras                   Create random categories and tags and assign them
                            to ~50% of the inserted items (randomly chosen).
  -comments <n>             Add n comments to ~50% of inserted published posts.
                            Comments are authored by random site subscribers.
                            Ignored when -type is page.
  -replies                  With -comments: also add a random number of replies
                            to each top-level comment (one level deep).
  -port     <port>          Port to include in the printed URLs (e.g. 3000).
                            Only affects output — not stored in the database.

Examples:
  $(basename "$0") -domain beth.com -user beth@beth.com -number 25
  $(basename "$0") -domain beth.com -user beth@beth.com -number 25 -port 3000
  $(basename "$0") -domain beth.com -user beth@beth.com -number 10 -status published -port 3000
  $(basename "$0") -domain beth.com -user beth@beth.com -number 5  -status draft
  $(basename "$0") -domain beth.com -user beth@beth.com -number 5  -type page
  $(basename "$0") -domain beth.com -user beth@beth.com -number 50 -extras
  $(basename "$0") -domain beth.com -user beth@beth.com -number 20 -comments 5 -port 3000
  $(basename "$0") -domain beth.com -user beth@beth.com -number 20 -comments 8 -replies -port 3000
  $(basename "$0") -domain beth.com -clear
  $(basename "$0") -domain beth.com -clear -force

Notes:
  - Reads DATABASE_URL from .env in the project root if not set in the environment.
  - Slugs include a random 4-character suffix to avoid collisions on repeat runs.
  - Published posts are assigned a random date within the last 12 months.
  - Comments require at least one subscriber user on the site (-comments is skipped otherwise).
  - -clear removes DB rows only; uploaded files in uploads/ are not deleted.
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
        -comments) COMMENTS="$2";      shift 2 ;;
        -replies)  REPLIES=1;          shift 1 ;;
        -clear)    CLEAR=1;            shift 1 ;;
        -force)    FORCE=1;            shift 1 ;;
        *) usage ;;
    esac
done

if [[ -n "$FORCE_STATUS" && "$FORCE_STATUS" != "published" && "$FORCE_STATUS" != "draft" && "$FORCE_STATUS" != "pending" ]]; then
    echo "ERROR: -status must be 'published', 'draft', or 'pending'" >&2
    exit 1
fi

if [[ "$TYPE" != "post" && "$TYPE" != "page" ]]; then
    echo "ERROR: -type must be 'post' or 'page'" >&2
    exit 1
fi

if [[ "$COMMENTS" != "0" ]]; then
    if ! [[ "$COMMENTS" =~ ^[0-9]+$ ]] || [[ "$COMMENTS" -lt 1 ]]; then
        echo "ERROR: -comments must be a positive integer" >&2
        exit 1
    fi
    if [[ "$TYPE" == "page" ]]; then
        echo "WARNING: -comments is ignored for -type page" >&2
        COMMENTS=0
    fi
fi

if [[ "$REPLIES" == "1" && "$COMMENTS" == "0" ]]; then
    echo "WARNING: -replies has no effect without -comments — ignoring" >&2
    REPLIES=0
fi

[[ -z "$DOMAIN" ]] && usage
[[ "$CLEAR" == "0" && -z "$USER_EMAIL" ]] && usage

if [[ "$CLEAR" == "0" ]] && { ! [[ "$NUMBER" =~ ^[0-9]+$ ]] || [[ "$NUMBER" -lt 1 ]]; }; then
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

if [[ "$CLEAR" == "0" ]]; then
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
fi

# ── Clear mode ────────────────────────────────────────────────────────────────
if [[ "$CLEAR" == "1" ]]; then
    POST_COUNT=$(psql -c "SELECT COUNT(*) FROM posts WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    COMMENT_COUNT=$(psql -c "SELECT COUNT(*) FROM comments WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    TAX_COUNT=$(psql -c "SELECT COUNT(*) FROM taxonomies WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    FORM_COUNT=$(psql -c "SELECT COUNT(*) FROM form_submissions WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    MEDIA_COUNT=$(psql -c "SELECT COUNT(*) FROM media WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')

    echo ""
    echo "  ── Content to delete for: $DOMAIN ──────────────────────"
    echo "  Posts / Pages    : $POST_COUNT  (comments and taxonomy links cascade)"
    echo "  Comments         : $COMMENT_COUNT"
    echo "  Categories / Tags: $TAX_COUNT"
    echo "  Form submissions : $FORM_COUNT"
    echo "  Media rows       : $MEDIA_COUNT  (files in uploads/ are NOT removed)"
    echo ""
    echo "  Users and site settings are NOT affected."
    echo ""

    if [[ "$FORCE" == "0" ]]; then
        printf "  Type 'yes' to delete or anything else to abort: "
        read -r CONFIRM
        if [[ "$CONFIRM" != "yes" ]]; then
            echo "Aborted."
            exit 0
        fi
    fi

    # Delete in dependency order. Posts cascade to post_meta, post_taxonomies,
    # and comments, so deleting posts is sufficient — but we also clean up
    # taxonomies, form_submissions, and media rows independently.
    command psql "$DATABASE_URL" -c "DELETE FROM posts            WHERE site_id = '$SITE_ID';" > /dev/null
    command psql "$DATABASE_URL" -c "DELETE FROM taxonomies       WHERE site_id = '$SITE_ID';" > /dev/null
    command psql "$DATABASE_URL" -c "DELETE FROM form_submissions WHERE site_id = '$SITE_ID';" > /dev/null
    command psql "$DATABASE_URL" -c "DELETE FROM media            WHERE site_id = '$SITE_ID';" > /dev/null
    command psql "$DATABASE_URL" -c "DELETE FROM media_folders    WHERE site_id = '$SITE_ID';" > /dev/null

    echo "Cleared. All content removed for $DOMAIN."
    exit 0
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
STATUSES=("published" "draft" "pending")

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

    # Random publish date+time within the last 90 days (only for published posts).
    # Pending posts get a submitted_at date; drafts get nothing.
    # Time is randomised to avoid navigation collisions when multiple posts land on the same day.
    DAYS_AGO=$(( RANDOM % 90 ))
    HOURS=$(( RANDOM % 24 ))
    MINUTES=$(( RANDOM % 60 ))
    SECONDS=$(( RANDOM % 60 ))
    PUBLISHED_AT="NOW() - INTERVAL '$DAYS_AGO days $HOURS hours $MINUTES minutes $SECONDS seconds'"
    SUBMITTED_AT="NULL"
    if [[ "$STATUS" == "draft" ]]; then
        PUBLISHED_AT="NULL"
    elif [[ "$STATUS" == "pending" ]]; then
        PUBLISHED_AT="NULL"
        SUBMITTED_AT="NOW() - INTERVAL '$(( RANDOM % 30 )) days'"
    fi

    CONTENT="<p>This is a sample post about <strong>$TOPIC</strong>. $(
        echo "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris. \
This article explores the $ADJ aspects of $TOPIC from a fresh perspective."
    )</p>
<p>Pellentesque habitant morbi tristique senectus et netus et malesuada fames. \
Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.</p>"

    EXCERPT="A $ADJ look at $TOPIC — exploring $NOUN and beyond."

    SQL="INSERT INTO posts (title, slug, content, excerpt, status, post_type, author_id, site_id, published_at, submitted_at)
         VALUES (
             '$TITLE',
             '$SLUG',
             '$CONTENT',
             '$EXCERPT',
             '$STATUS',
             '$TYPE',
             '$USER_ID',
             '$SITE_ID',
             $PUBLISHED_AT,
             $SUBMITTED_AT
         ) RETURNING id;"

    POST_ID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c "$SQL" 2>/dev/null | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')
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

# ── Comments ────────────────────────────────────────────────────────────────────
if [[ "$COMMENTS" -gt 0 && "$SUCCESS" -gt 0 ]]; then
    echo ""
    echo "Seeding comments..."

    # Collect only the published post IDs (drafts don't show comments).
    # Build a quoted IN list from the array, e.g. 'uuid1','uuid2',...
    IN_LIST=$(printf "'%s'," "${POST_IDS[@]}")
    IN_LIST="${IN_LIST%,}"   # strip trailing comma
    mapfile -t PUB_IDS < <(
        command psql "$DATABASE_URL" --tuples-only --no-align \
            -c "SELECT id FROM posts WHERE id IN ($IN_LIST) AND status = 'published';" \
            2>/dev/null
    )

    PUB_COUNT=${#PUB_IDS[@]}
    if [[ "$PUB_COUNT" -eq 0 ]]; then
        echo "  No published posts in this run — skipping comments."
    else
        # Look up subscribers for this site.
        mapfile -t SUB_IDS < <(
            command psql "$DATABASE_URL" --tuples-only --no-align \
                -c "SELECT u.id FROM users u
                    JOIN site_users su ON su.user_id = u.id
                    WHERE su.site_id = '$SITE_ID'
                      AND u.role = 'subscriber'
                      AND u.deleted_at IS NULL;" \
                2>/dev/null
        )

        SUB_COUNT=${#SUB_IDS[@]}
        if [[ "$SUB_COUNT" -eq 0 ]]; then
            echo "  WARNING: No subscribers found for '$DOMAIN' — skipping comments."
            echo "           Add subscriber users to the site and re-run with -comments."
        else
            echo "  Found $SUB_COUNT subscriber(s) to use as comment authors."

            # Word bank for comment bodies.
            COMMENT_BODIES=(
                "Really enjoyed this post — thanks for sharing!"
                "Great writeup. Learned something new today."
                "I've been thinking about this topic for a while. Well said."
                "Interesting perspective. I'd love to hear more on this."
                "This is exactly what I was looking for. Bookmarked."
                "Good points here. Have you considered the flip side?"
                "Solid article. The examples really helped it click."
                "Thanks for breaking this down so clearly."
                "I shared this with my team — very relevant to what we're working on."
                "Not sure I fully agree, but you've given me a lot to think about."
                "This reminded me of a similar issue I ran into last year."
                "Very well written. Looking forward to more posts like this."
                "The detail in this post is impressive. Nice work."
                "I've seen this come up a lot lately. Good to have a clear take on it."
                "Came here from a search and glad I did — great content."
                "This changed how I think about the subject. Thanks."
                "Would love a follow-up post going deeper on the second point."
                "Agreed on most counts. The last section especially resonated."
                "First time commenting here. Really quality stuff."
                "Short and to the point. Exactly what I needed."
            )

            REPLY_BODIES=(
                "Good point — I hadn't thought of it that way."
                "Totally agree with you on this."
                "That's a fair take. Thanks for weighing in."
                "Interesting — did you find that worked well in practice?"
                "Yeah, I had the same reaction when I first read it."
                "Thanks for adding that context."
                "Worth expanding on if you get the chance!"
                "That lines up with my experience too."
                "I was wondering about that — glad someone brought it up."
                "Makes sense. Appreciate the extra perspective."
            )

            # Select ~50% of published posts randomly.
            HALF=$(( (PUB_COUNT + 1) / 2 ))
            mapfile -t SHUFFLED < <(printf '%s\n' "${PUB_IDS[@]}" | shuf)
            COMMENT_TARGETS=("${SHUFFLED[@]:0:$HALF}")

            TOTAL_COMMENTS=0
            TOTAL_REPLIES=0
            COMMENTED_URLS=()

            for PID in "${COMMENT_TARGETS[@]}"; do
                # Enable comments on this post.
                command psql "$DATABASE_URL" -c \
                    "UPDATE posts SET comments_enabled = TRUE WHERE id = '$PID';" \
                    > /dev/null 2>&1

                TOP_LEVEL_IDS=()

                for (( c = 1; c <= COMMENTS; c++ )); do
                    SUB_ID="${SUB_IDS[$((RANDOM % SUB_COUNT))]}"
                    BODY="${COMMENT_BODIES[$((RANDOM % ${#COMMENT_BODIES[@]}))]}"
                    # Spread created_at over the last 30 days for realism.
                    MINS_AGO=$(( RANDOM % 43200 ))

                    CID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c \
                        "INSERT INTO comments (post_id, site_id, author_id, body, created_at, updated_at)
                         VALUES ('$PID', '$SITE_ID', '$SUB_ID', \$\$${BODY}\$\$,
                                 NOW() - INTERVAL '$MINS_AGO minutes',
                                 NOW() - INTERVAL '$MINS_AGO minutes')
                         RETURNING id;" \
                        2>/dev/null | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')

                    if [[ -n "$CID" ]]; then
                        TOP_LEVEL_IDS+=("$CID")
                        TOTAL_COMMENTS=$((TOTAL_COMMENTS + 1))
                    fi
                done

                # Replies: for each top-level comment, randomly add 0–3 replies.
                if [[ "$REPLIES" == "1" && "${#TOP_LEVEL_IDS[@]}" -gt 0 ]]; then
                    for PARENT_ID in "${TOP_LEVEL_IDS[@]}"; do
                        NUM_REPLIES=$(( RANDOM % 4 ))   # 0, 1, 2, or 3
                        for (( r = 0; r < NUM_REPLIES; r++ )); do
                            SUB_ID="${SUB_IDS[$((RANDOM % SUB_COUNT))]}"
                            BODY="${REPLY_BODIES[$((RANDOM % ${#REPLY_BODIES[@]}))]}"
                            MINS_AGO=$(( RANDOM % 43200 ))

                            RID=$(command psql "$DATABASE_URL" --tuples-only --no-align -c \
                                "INSERT INTO comments (post_id, site_id, author_id, parent_id, body, created_at, updated_at)
                                 VALUES ('$PID', '$SITE_ID', '$SUB_ID', '$PARENT_ID', \$\$${BODY}\$\$,
                                         NOW() - INTERVAL '$MINS_AGO minutes',
                                         NOW() - INTERVAL '$MINS_AGO minutes')
                                 RETURNING id;" \
                                2>/dev/null | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' || true)

                            [[ -n "$RID" ]] && TOTAL_REPLIES=$((TOTAL_REPLIES + 1))
                        done
                    done
                fi

                # Build the URL for this post.
                SLUG=$(command psql "$DATABASE_URL" --tuples-only --no-align \
                    -c "SELECT slug FROM posts WHERE id = '$PID';" 2>/dev/null | tr -d '[:space:]')
                PORT_PART="${PORT:+:$PORT}"
                COMMENTED_URLS+=("http://${DOMAIN}${PORT_PART}/blog/${SLUG}")
            done

            echo ""
            echo "Posts with comments:"
            for URL in "${COMMENTED_URLS[@]}"; do
                echo "  $URL"
            done
            echo ""
            if [[ "$REPLIES" == "1" ]]; then
                echo "Comments done. $TOTAL_COMMENTS top-level, $TOTAL_REPLIES replies across ${#COMMENT_TARGETS[@]} post(s)."
            else
                echo "Comments done. $TOTAL_COMMENTS comment(s) across ${#COMMENT_TARGETS[@]} post(s)."
            fi
        fi
    fi
fi
