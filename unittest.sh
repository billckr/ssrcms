#!/usr/bin/env bash
# unittest.sh — Run all unit tests and display a formatted summary table.
# Usage: ./unittest.sh
#
# Passing tests run immediately (no database required).
# Ignored tests require a live PostgreSQL instance — see ./app.sh test-all.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source Rust toolchain if needed
if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

# Load SQLX_OFFLINE from .env if present
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"
fi
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"

cd "$SCRIPT_DIR"

# ── Run tests, capture output ──────────────────────────────────────────────────
output=$(cargo test -p synaptic-core -p admin 2>&1)

# ── Parse each "test result:" line ────────────────────────────────────────────
# cargo prints lines like:
#   test result: ok. 3 passed; 0 failed; 0 ignored; ...
# We collect them in order.

mapfile -t result_lines < <(echo "$output" | grep "^test result:")

# Suite labels in the order cargo emits them
labels=(
    "admin unit tests"
    "synaptic-core unit tests"
    "synaptic binary (main.rs)"
    "model_crud integration"
    "routes integration"
    "theme_e2e integration"
    "Doc-tests admin"
    "Doc-tests synaptic_core"
)

notes=(
    "View icon + posts/pages UI"
    "Core models, filters, config, errors"
    "Entry point — no inline tests"
    "Need live PostgreSQL (--include-ignored)"
    "Need live PostgreSQL (--include-ignored)"
    "Need live PostgreSQL (--include-ignored)"
    "None written"
    "None written"
)

# ── Extract passed / ignored counts from a result line ────────────────────────
get_passed()  { echo "$1" | grep -oP '\d+(?= passed)'  || echo "0"; }
get_ignored() { echo "$1" | grep -oP '\d+(?= ignored)' || echo "0"; }
get_failed()  { echo "$1" | grep -oP '\d+(?= failed)'  || echo "0"; }

# ── Table dimensions ──────────────────────────────────────────────────────────
col1=26  # Suite
col2=7   # Passing
col3=7   # Failed
col4=7   # Ignored
col5=38  # Notes

pad() {
    local s="$1" w="$2"
    printf "%-${w}s" "$s"
}

hline() {
    local l="$1" m="$2" r="$3"
    printf "%s" "$l"
    printf '%0.s─' $(seq 1 $((col1+2)))
    printf "%s" "$m"
    printf '%0.s─' $(seq 1 $((col2+2)))
    printf "%s" "$m"
    printf '%0.s─' $(seq 1 $((col3+2)))
    printf "%s" "$m"
    printf '%0.s─' $(seq 1 $((col4+2)))
    printf "%s" "$m"
    printf '%0.s─' $(seq 1 $((col5+2)))
    printf "%s\n" "$r"
}

row() {
    printf "│ %s │ %s │ %s │ %s │ %s │\n" \
        "$(pad "$1" $col1)" \
        "$(pad "$2" $col2)" \
        "$(pad "$3" $col3)" \
        "$(pad "$4" $col4)" \
        "$(pad "$5" $col5)"
}

echo ""
hline "┌" "┬" "┐"
row "Suite" "Passing" "Failed" "Ignored" "Notes"
hline "├" "┼" "┤"

total_passed=0
total_failed=0
any_failure=false

for i in "${!labels[@]}"; do
    line="${result_lines[$i]:-}"
    passed=$(get_passed  "$line")
    failed=$(get_failed  "$line")
    ignored=$(get_ignored "$line")

    total_passed=$(( total_passed + passed ))
    total_failed=$(( total_failed + failed ))
    [[ "$failed" -gt 0 ]] && any_failure=true

    row "${labels[$i]}" "$passed" "$failed" "$ignored" "${notes[$i]}"

    # Separator between rows except after last
    if [[ $i -lt $(( ${#labels[@]} - 1 )) ]]; then
        hline "├" "┼" "┤"
    fi
done

hline "├" "┼" "┤"
row "TOTAL" "$total_passed" "$total_failed" "" ""
hline "└" "┴" "┘"
echo ""

# ── Exit code reflects test outcome ───────────────────────────────────────────
if $any_failure; then
    echo "  RESULT: FAILED — ${total_failed} test(s) failed."
    echo ""
    # Print the raw cargo output so failures are visible
    echo "$output"
    exit 1
else
    echo "  RESULT: ALL TESTS PASSED (${total_passed} passing)"
    echo ""
    exit 0
fi
