#!/usr/bin/env bash
# Export the `documentation` table's data as portable INSERT statements.
#
# The documentation table (migrations 0032/0033) is populated by the
# document-changes skill over time and is never seeded by migrations, so a
# fresh checkout/install has the empty table but none of the rows. This
# script dumps just the row data so it can be committed to the repo and
# imported into another environment with import-docs-table.sh.
#
# Usage:
#   DATABASE_URL=postgres://user:pass@host:port/db ./scripts/export-docs-table.sh [output-file]
#
# Defaults to reading DATABASE_URL from .env in the repo root if not set in
# the environment, and writes to scripts/documentation_data.sql by default.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_FILE="${1:-$SCRIPT_DIR/documentation_data.sql}"

if [[ -z "${DATABASE_URL:-}" && -f "$REPO_ROOT/.env" ]]; then
    DATABASE_URL=$(grep -E '^DATABASE_URL=' "$REPO_ROOT/.env" | cut -d= -f2- | tr -d '[:space:]')
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set and could not be read from .env" >&2
    exit 1
fi

pg_dump "$DATABASE_URL" \
    --table=documentation \
    --data-only \
    --column-inserts \
    --no-owner \
    -f "$OUT_FILE"

echo "Wrote documentation table data to $OUT_FILE"
