#!/usr/bin/env bash
# Import the `documentation` table's data from the dump produced by
# export-docs-table.sh (scripts/documentation_data.sql by default).
#
# Run this after migrations have created the (empty) `documentation` table
# on a new install/environment. Truncates the table first so the import is
# safe to re-run, then resets the id sequence so future inserts don't
# collide with the imported rows.
#
# Usage:
#   DATABASE_URL=postgres://user:pass@host:port/db ./scripts/import-docs-table.sh [input-file]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
IN_FILE="${1:-$SCRIPT_DIR/documentation_data.sql}"

if [[ -z "${DATABASE_URL:-}" && -f "$REPO_ROOT/.env" ]]; then
    DATABASE_URL=$(grep -E '^DATABASE_URL=' "$REPO_ROOT/.env" | cut -d= -f2- | tr -d '[:space:]')
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set and could not be read from .env" >&2
    exit 1
fi

if [[ ! -f "$IN_FILE" ]]; then
    echo "ERROR: dump file not found: $IN_FILE" >&2
    exit 1
fi

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<SQL
TRUNCATE TABLE public.documentation;
\i $IN_FILE
SET search_path = public;
SELECT setval('public.documentation_id_seq', COALESCE((SELECT MAX(id) FROM public.documentation), 1));
SQL

echo "Imported documentation table data from $IN_FILE"
