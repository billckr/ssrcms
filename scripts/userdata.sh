#!/usr/bin/env bash
# userdata.sh — Query all content associated with a Synaptic Signals user.
# Usage:
#   ./userdata.sh beth@beth.com
#   ./userdata.sh fec4c81b-8475-4195-8814-7030f6cf95bb

DB_URL="${DATABASE_URL:-postgres://synaptic:password@localhost:5432/synaptic_signals}"

ARG="${1:-}"
if [[ -z "$ARG" ]]; then
    echo "Usage: $0 <email|user-id>" >&2
    exit 1
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

q() {
    psql "$DB_URL" --no-align -t -q -c "$1" 2>/dev/null
}

pad() { printf "  %-22s %s\n" "$1" "$2"; }
hr()  { printf '  %s\n' "$(printf '─%.0s' {1..56})"; }

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

# ── Posts detail ──────────────────────────────────────────────────────────────

if [[ "$POSTS_COUNT" -gt 0 ]]; then
    echo ""
    hr
    printf "  POSTS / PAGES\n"
    hr
    printf "  %-8s %-10s %-32s %s\n" "TYPE" "STATUS" "TITLE" "SITE"
    q "SELECT p.post_type, p.status, LEFT(p.title,32), COALESCE(s.hostname,'unscoped')
       FROM posts p
       LEFT JOIN sites s ON s.id = p.site_id
       WHERE p.author_id = '$USER_ID'
       ORDER BY p.post_type, p.created_at DESC;" \
    | while IFS='|' read -r ptype status title hostname; do
        printf "  %-8s %-10s %-32s %s\n" "$ptype" "$status" "$title" "$hostname"
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

# ── Taxonomy detail ───────────────────────────────────────────────────────────

if [[ "$TAX_COUNT" -gt 0 && -n "$SITE_ID_CSV" ]]; then
    echo ""
    hr
    printf "  TAXONOMIES (tags / categories on user's sites)\n"
    hr
    printf "  %-10s %-24s %s\n" "TYPE" "NAME" "SITE"
    q "SELECT t.taxonomy, t.name, COALESCE(s.hostname,'unscoped')
       FROM taxonomies t
       LEFT JOIN sites s ON s.id = t.site_id
       WHERE t.site_id IN ($SITE_ID_CSV)
       ORDER BY t.taxonomy, t.name;" \
    | while IFS='|' read -r ttype tname hostname; do
        printf "  %-10s %-24s %s\n" "$ttype" "$tname" "$hostname"
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
