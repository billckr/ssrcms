#!/usr/bin/env bash
# test_builder_race.sh
# Tests the builder publish/save race condition at the database level.
#
# Simulates two concurrent UPDATE statements hitting the same page row:
#   - "publish" writes empty content to both composition + draft_composition
#   - "save"    writes stale content to draft_composition only
#
# If they overlap, the live composition column must reflect the publish,
# not the stale save. This is the same race the client-side isSaving fix
# prevents from occurring in the browser.
#
# Usage: ./scripts/test_builder_race.sh

set -e

PAGE_ID="532fcd70-6327-4c25-87a9-b97c6a04847a"

EMPTY='{"root":{"props":{}},"zones":{},"content":[]}'
STALE='{"root":{"props":{}},"zones":{},"content":[{"type":"Hero","props":{"id":"Hero-race-test","heading":"RACE TEST — should not be on live page"}}]}'

echo "=== Builder Race Condition Test ==="
echo ""

# ── Snapshot originals ────────────────────────────────────────────────────────

ORIG_COMP_FILE=$(mktemp)
ORIG_DRAFT_FILE=$(mktemp)

psql -U postgres synaptic_signals -t -A -c \
  "SELECT composition FROM page_compositions WHERE id = '$PAGE_ID';" > "$ORIG_COMP_FILE"
psql -U postgres synaptic_signals -t -A -c \
  "SELECT draft_composition FROM page_compositions WHERE id = '$PAGE_ID';" > "$ORIG_DRAFT_FILE"

restore() {
  # Write a SQL file with dollar-quoted JSON literals to avoid shell escaping issues
  RESTORE_SQL=$(mktemp --suffix=.sql)
  printf "UPDATE page_compositions\nSET   composition       = \$comp\$%s\$comp\$,\n      draft_composition = \$draft\$%s\$draft\$,\n      updated_at        = NOW()\nWHERE id = '%s';\n" \
    "$(cat "$ORIG_COMP_FILE")" "$(cat "$ORIG_DRAFT_FILE")" "$PAGE_ID" > "$RESTORE_SQL"
  psql -U postgres synaptic_signals -q -f "$RESTORE_SQL" 2>/dev/null \
    && echo "  ✓ Restored original page data." \
    || echo "  ! Restore failed — page may need manual reset."
  rm -f "$ORIG_COMP_FILE" "$ORIG_DRAFT_FILE" "$RESTORE_SQL"
}
trap restore EXIT

# ── Round 1: publish wins (correct order) ─────────────────────────────────────

echo "→ Round 1: publish fires first, stale save fires after..."

psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   composition       = \$a\$${EMPTY}\$a\$,
         draft_composition = \$a\$${EMPTY}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';"

psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   draft_composition = \$a\$${STALE}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';"

COMP=$(psql -U postgres synaptic_signals -t -c \
  "SELECT jsonb_array_length(composition->'content') FROM page_compositions WHERE id = '$PAGE_ID';" | xargs)
DRAFT=$(psql -U postgres synaptic_signals -t -c \
  "SELECT jsonb_array_length(draft_composition->'content') FROM page_compositions WHERE id = '$PAGE_ID';" | xargs)

echo "  Live composition blocks:  $COMP  (expected: 0)"
echo "  Draft composition blocks: $DRAFT (expected: 1)"
[ "$COMP" = "0" ] && echo "  ✓ PASS: Live page unaffected by stale save." || { echo "  ✗ FAIL"; FAILED=1; }
echo ""

# ── Round 2: stale save fires first, publish fires after ─────────────────────

echo "→ Round 2: stale save fires first, publish fires after..."

psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   draft_composition = \$a\$${STALE}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';"

psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   composition       = \$a\$${EMPTY}\$a\$,
         draft_composition = \$a\$${EMPTY}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';"

COMP=$(psql -U postgres synaptic_signals -t -c \
  "SELECT jsonb_array_length(composition->'content') FROM page_compositions WHERE id = '$PAGE_ID';" | xargs)
DRAFT=$(psql -U postgres synaptic_signals -t -c \
  "SELECT jsonb_array_length(draft_composition->'content') FROM page_compositions WHERE id = '$PAGE_ID';" | xargs)

echo "  Live composition blocks:  $COMP  (expected: 0)"
echo "  Draft composition blocks: $DRAFT (expected: 0)"
[ "$COMP" = "0" ] && echo "  ✓ PASS: Publish correctly overwrote stale save." || { echo "  ✗ FAIL"; FAILED=1; }
echo ""

# ── Round 3: simulate true concurrency via parallel psql processes ────────────

echo "→ Round 3: publish and save fire truly concurrently (parallel processes)..."

# Reset to a known state with stale content
psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   composition       = \$a\$${STALE}\$a\$,
         draft_composition = \$a\$${STALE}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';"

# Fire both simultaneously
psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   composition       = \$a\$${EMPTY}\$a\$,
         draft_composition = \$a\$${EMPTY}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';" &
PID_PUB=$!

psql -U postgres synaptic_signals -q -c \
  "UPDATE page_compositions
   SET   draft_composition = \$a\$${STALE}\$a\$,
         updated_at        = NOW()
   WHERE id = '$PAGE_ID';" &
PID_SAVE=$!

wait $PID_PUB
wait $PID_SAVE

COMP=$(psql -U postgres synaptic_signals -t -c \
  "SELECT jsonb_array_length(composition->'content') FROM page_compositions WHERE id = '$PAGE_ID';" | xargs)

echo "  Live composition blocks:  $COMP  (expected: 0)"
if [ "$COMP" = "0" ]; then
  echo "  ✓ PASS: Publish won the race — live page is clean."
else
  echo "  ✗ FAIL: Live composition has $COMP block(s)."
  echo "    NOTE: This indicates a server-level race the client-side fix cannot prevent"
  echo "    (e.g. two browser tabs). Consider adding a DB-level check."
  FAILED=1
fi
echo ""

# ── Summary ───────────────────────────────────────────────────────────────────

if [ -z "$FAILED" ]; then
  echo "=== RESULT: ALL PASS ==="
else
  echo "=== RESULT: FAILURES DETECTED ==="
  exit 1
fi
