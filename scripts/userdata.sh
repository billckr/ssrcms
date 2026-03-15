#!/usr/bin/env bash
# userdata.sh — Query all content associated with a Synaptic Signals user or domain.
# Usage:
#   ./userdata.sh beth@beth.com
#   ./userdata.sh fec4c81b-8475-4195-8814-7030f6cf95bb
#   ./userdata.sh --domain bckr.local

DB_URL="${DATABASE_URL:-postgres://synaptic:password@localhost:5432/synaptic_signals}"

# ── Argument parsing ───────────────────────────────────────────────────────────

MODE="user"
ARG=""

if [[ "${1:-}" == "--domain" ]]; then
    MODE="domain"
    ARG="${2:-}"
    if [[ -z "$ARG" ]]; then
        echo "Usage: $0 --domain <hostname>" >&2
        exit 1
    fi
else
    ARG="${1:-}"
    if [[ -z "$ARG" ]]; then
        echo "Usage: $0 <email|user-id|domain>" >&2
        exit 1
    fi
    # Auto-detect domain: not a UUID and no @ means treat as hostname
    if [[ ! "$ARG" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ && "$ARG" != *@* ]]; then
        MODE="domain"
    fi
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

q() {
    psql "$DB_URL" --no-align -t -q -c "$1" 2>/dev/null
}

pad() { printf "  %-22s %s\n" "$1" "$2"; }
hr()  { printf '  %s\n' "$(printf '─%.0s' {1..56})"; }

# ══════════════════════════════════════════════════════════════════════════════
# DOMAIN MODE
# ══════════════════════════════════════════════════════════════════════════════

if [[ "$MODE" == "domain" ]]; then

    SITE_ROW=$(q "SELECT id, hostname, owner_user_id, created_at
                  FROM sites WHERE hostname = '$ARG' LIMIT 1;")

    if [[ -z "$SITE_ROW" ]]; then
        echo "No site found for domain: $ARG" >&2
        exit 1
    fi

    IFS='|' read -r SITE_ID HOSTNAME OWNER_ID CREATED_AT <<< "$SITE_ROW"

    # Resolve owner name
    OWNER_NAME=$(q "SELECT COALESCE(username, email, 'unknown') FROM users WHERE id = '$OWNER_ID';" | tr -d '[:space:]')

    echo ""
    hr
    printf "  DOMAIN DATA REPORT\n"
    hr
    pad "Site ID:"    "$SITE_ID"
    pad "Hostname:"   "$HOSTNAME"
    pad "Owner:"      "${OWNER_NAME:-unknown}"
    pad "Created:"    "$CREATED_AT"

    # ── Users on this site ─────────────────────────────────────────────────

    echo ""
    hr
    printf "  USERS\n"
    hr

    USER_ROWS=$(q "SELECT u.id, u.username, u.email, su.role
                   FROM site_users su
                   JOIN users u ON u.id = su.user_id
                   WHERE su.site_id = '$SITE_ID'
                   ORDER BY su.role, u.email;")

    if [[ -z "$USER_ROWS" ]]; then
        echo "  (none)"
    else
        printf "  %-38s %-20s %-12s %s\n" "USER ID" "EMAIL" "ROLE" "USERNAME"
        while IFS='|' read -r uid uname uemail urole; do
            printf "  %-38s %-20s %-12s %s\n" "$uid" "$uemail" "$urole" "$uname"
        done <<< "$USER_ROWS"
    fi

    # ── Content counts ─────────────────────────────────────────────────────

    echo ""
    hr
    printf "  CONTENT SUMMARY\n"
    hr

    POSTS_COUNT=$(q "SELECT COUNT(*) FROM posts WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    MEDIA_COUNT=$(q "SELECT COUNT(*) FROM media WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    TAX_COUNT=$(q "SELECT COUNT(*) FROM taxonomies WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    FORM_SUB_COUNT=$(q "SELECT COUNT(*) FROM form_submissions WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')
    FORM_BLK_COUNT=$(q "SELECT COUNT(*) FROM form_blocks WHERE site_id = '$SITE_ID';" | tr -d '[:space:]')

    POSTS_COUNT=${POSTS_COUNT:-0}
    MEDIA_COUNT=${MEDIA_COUNT:-0}
    TAX_COUNT=${TAX_COUNT:-0}
    FORM_SUB_COUNT=${FORM_SUB_COUNT:-0}
    FORM_BLK_COUNT=${FORM_BLK_COUNT:-0}

    pad "Posts / Pages:"    "$POSTS_COUNT"
    pad "Media uploads:"    "$MEDIA_COUNT"
    pad "Taxonomies:"       "$TAX_COUNT"
    pad "Form submissions:" "$FORM_SUB_COUNT"
    pad "Form blocks:"      "$FORM_BLK_COUNT"

    # ── Posts breakdown ────────────────────────────────────────────────────

    if [[ "$POSTS_COUNT" -gt 0 ]]; then
        echo ""
        hr
        printf "  POSTS / PAGES BREAKDOWN\n"
        hr
        printf "  %-8s %-12s %s\n" "TYPE" "STATUS" "COUNT"
        q "SELECT post_type, status, COUNT(*)
           FROM posts
           WHERE site_id = '$SITE_ID'
           GROUP BY post_type, status
           ORDER BY post_type, status;" \
        | while IFS='|' read -r ptype status cnt; do
            printf "  %-8s %-12s %s\n" "$ptype" "$status" "$cnt"
        done
    fi

    # ── Media detail ───────────────────────────────────────────────────────

    if [[ "$MEDIA_COUNT" -gt 0 ]]; then
        echo ""
        hr
        printf "  MEDIA\n"
        hr
        printf "  %-38s %-22s %s\n" "ID" "MIME TYPE" "FILENAME"
        q "SELECT id, mime_type, filename FROM media
           WHERE site_id = '$SITE_ID'
           ORDER BY created_at DESC;" \
        | while IFS='|' read -r mid mime fname; do
            printf "  %-38s %-22s %s\n" "$mid" "$mime" "$fname"
        done
    fi

    # ── Taxonomy breakdown ─────────────────────────────────────────────────

    if [[ "$TAX_COUNT" -gt 0 ]]; then
        echo ""
        hr
        printf "  TAXONOMIES BREAKDOWN\n"
        hr
        printf "  %-12s %s\n" "TYPE" "COUNT"
        q "SELECT taxonomy, COUNT(*)
           FROM taxonomies
           WHERE site_id = '$SITE_ID'
           GROUP BY taxonomy
           ORDER BY taxonomy;" \
        | while IFS='|' read -r ttype cnt; do
            printf "  %-12s %s\n" "$ttype" "$cnt"
        done
    fi

    # ── Form submissions detail ────────────────────────────────────────────

    if [[ "$FORM_SUB_COUNT" -gt 0 ]]; then
        echo ""
        hr
        printf "  FORM SUBMISSIONS\n"
        hr
        printf "  %-38s %s\n" "ID" "SUBMITTED AT"
        q "SELECT id, created_at FROM form_submissions
           WHERE site_id = '$SITE_ID'
           ORDER BY created_at DESC
           LIMIT 20;" \
        | while IFS='|' read -r fsid fsat; do
            printf "  %-38s %s\n" "$fsid" "$fsat"
        done
        [[ "$FORM_SUB_COUNT" -gt 20 ]] && echo "  ... and $((FORM_SUB_COUNT - 20)) more"
    fi

    echo ""
    hr
    printf "  Report generated: %s\n" "$(date '+%Y-%m-%d %H:%M:%S')"
    hr
    echo ""
    exit 0
fi

# ══════════════════════════════════════════════════════════════════════════════
# USER MODE
# ══════════════════════════════════════════════════════════════════════════════

# ── Resolve user ──────────────────────────────────────────────────────────────

if [[ "$ARG" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]; then
    WHERE="id = '$ARG'"
else
    WHERE="email = '$ARG'"
fi

USER_ROW=$(q "SELECT id, username, email, role, is_protected, is_active,
                     COALESCE(deleted_at::text, 'active')
              FROM users WHERE $WHERE LIMIT 1;")

if [[ -z "$USER_ROW" ]]; then
    echo "No user found for: $ARG" >&2
    exit 1
fi

IFS='|' read -r USER_ID USERNAME EMAIL ROLE PROTECTED ACTIVE STATUS <<< "$USER_ROW"

# ── Header ────────────────────────────────────────────────────────────────────

echo ""
hr
printf "  USER DATA REPORT\n"
hr
pad "ID:"        "$USER_ID"
pad "Username:"  "$USERNAME"
pad "Email:"     "$EMAIL"
pad "Role:"      "$ROLE"
pad "Protected:" "$([ "$PROTECTED" = t ] && echo yes || echo no)"
pad "Active:"    "$([ "$ACTIVE"    = t ] && echo yes || echo no)"
pad "Status:"    "$STATUS"

# ── Site memberships ──────────────────────────────────────────────────────────

echo ""
hr
printf "  SITE MEMBERSHIPS\n"
hr

SITE_ROWS=$(q "
SELECT s.id, s.hostname, su.role,
       (COALESCE(s.owner_user_id::text,'') = '$USER_ID') AS is_owner
FROM site_users su
JOIN sites s ON s.id = su.site_id
WHERE su.user_id = '$USER_ID'
ORDER BY s.hostname;")

if [[ -z "$SITE_ROWS" ]]; then
    echo "  (none)"
else
    printf "  %-38s %-12s %s\n" "SITE ID" "ROLE" "HOSTNAME"
    while IFS='|' read -r sid hostname role is_owner; do
        owner_flag=""; [[ "$is_owner" == "t" ]] && owner_flag=" [owner]"
        printf "  %-38s %-12s %s%s\n" "$sid" "$role" "$hostname" "$owner_flag"
    done <<< "$SITE_ROWS"
fi

# Collect site IDs as a quoted CSV for IN() clauses
SITE_ID_CSV=$(q "SELECT string_agg(quote_literal(site_id::text), ',')
                 FROM site_users WHERE user_id = '$USER_ID';")

# ── Content counts ────────────────────────────────────────────────────────────

echo ""
hr
printf "  CONTENT SUMMARY\n"
hr

POSTS_COUNT=$(q "SELECT COUNT(*) FROM posts WHERE author_id = '$USER_ID';" | tr -d '[:space:]')
MEDIA_COUNT=$(q "SELECT COUNT(*) FROM media WHERE uploaded_by = '$USER_ID';" | tr -d '[:space:]')
POSTS_COUNT=${POSTS_COUNT:-0}
MEDIA_COUNT=${MEDIA_COUNT:-0}

if [[ -n "$SITE_ID_CSV" ]]; then
    TAX_COUNT=$(q "SELECT COUNT(*) FROM taxonomies WHERE site_id IN ($SITE_ID_CSV);" | tr -d '[:space:]')
    FORM_SUB_COUNT=$(q "SELECT COUNT(*) FROM form_submissions WHERE site_id IN ($SITE_ID_CSV);" | tr -d '[:space:]')
    FORM_BLK_COUNT=$(q "SELECT COUNT(*) FROM form_blocks WHERE site_id IN ($SITE_ID_CSV);" | tr -d '[:space:]')
else
    TAX_COUNT=0; FORM_SUB_COUNT=0; FORM_BLK_COUNT=0
fi

TAX_COUNT=${TAX_COUNT:-0}
FORM_SUB_COUNT=${FORM_SUB_COUNT:-0}
FORM_BLK_COUNT=${FORM_BLK_COUNT:-0}

pad "Posts / Pages:"     "$POSTS_COUNT"
pad "Media uploads:"     "$MEDIA_COUNT"
pad "Taxonomies:"        "$TAX_COUNT"
pad "Form submissions:"  "$FORM_SUB_COUNT"
pad "Form blocks:"       "$FORM_BLK_COUNT"

# ── Posts breakdown ───────────────────────────────────────────────────────────

if [[ "$POSTS_COUNT" -gt 0 ]]; then
    echo ""
    hr
    printf "  POSTS / PAGES BREAKDOWN\n"
    hr
    printf "  %-8s %-12s %s\n" "TYPE" "STATUS" "COUNT"
    q "SELECT post_type, status, COUNT(*)
       FROM posts
       WHERE author_id = '$USER_ID'
       GROUP BY post_type, status
       ORDER BY post_type, status;" \
    | while IFS='|' read -r ptype status cnt; do
        printf "  %-8s %-12s %s\n" "$ptype" "$status" "$cnt"
    done
fi

# ── Media detail ──────────────────────────────────────────────────────────────

if [[ "$MEDIA_COUNT" -gt 0 ]]; then
    echo ""
    hr
    printf "  MEDIA\n"
    hr
    printf "  %-38s %-22s %s\n" "ID" "MIME TYPE" "FILENAME"
    q "SELECT id, mime_type, filename FROM media
       WHERE uploaded_by = '$USER_ID'
       ORDER BY created_at DESC;" \
    | while IFS='|' read -r mid mime fname; do
        printf "  %-38s %-22s %s\n" "$mid" "$mime" "$fname"
    done
fi

# ── Taxonomy breakdown ────────────────────────────────────────────────────────

if [[ "$TAX_COUNT" -gt 0 && -n "$SITE_ID_CSV" ]]; then
    echo ""
    hr
    printf "  TAXONOMIES BREAKDOWN\n"
    hr
    printf "  %-12s %s\n" "TYPE" "COUNT"
    q "SELECT taxonomy, COUNT(*)
       FROM taxonomies
       WHERE site_id IN ($SITE_ID_CSV)
       GROUP BY taxonomy
       ORDER BY taxonomy;" \
    | while IFS='|' read -r ttype cnt; do
        printf "  %-12s %s\n" "$ttype" "$cnt"
    done
fi

# ── Form submissions detail ───────────────────────────────────────────────────

if [[ "$FORM_SUB_COUNT" -gt 0 && -n "$SITE_ID_CSV" ]]; then
    echo ""
    hr
    printf "  FORM SUBMISSIONS (on user's sites)\n"
    hr
    printf "  %-38s %-26s %s\n" "ID" "SUBMITTED AT" "SITE"
    q "SELECT fs.id, fs.created_at, COALESCE(s.hostname,'unscoped')
       FROM form_submissions fs
       LEFT JOIN sites s ON s.id = fs.site_id
       WHERE fs.site_id IN ($SITE_ID_CSV)
       ORDER BY fs.created_at DESC
       LIMIT 20;" \
    | while IFS='|' read -r fsid fsat hostname; do
        printf "  %-38s %-26s %s\n" "$fsid" "$fsat" "$hostname"
    done
    [[ "$FORM_SUB_COUNT" -gt 20 ]] && echo "  ... and $((FORM_SUB_COUNT - 20)) more"
fi

# ── Footer ────────────────────────────────────────────────────────────────────

echo ""
hr
printf "  Report generated: %s\n" "$(date '+%Y-%m-%d %H:%M:%S')"
hr
echo ""
