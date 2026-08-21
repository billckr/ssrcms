#!/usr/bin/env bash
# populate-wp-test-data.sh — fill the local WordPress dev install with a
# large batch of test content, for exercising SynapCMS's WP importer
# (core/src/handlers/admin/wp_import.rs) end-to-end via a real WXR export.
#
# Usage: ./scripts/populate-wp-test-data.sh [total_posts_and_pages] [new_media_count]
#   (defaults: 200 posts/pages, 15 new media items)
#
# What it creates (via scripts/wp-test-data-populate.php): a handful of
# author/editor/contributor users, categories, tags, new media items
# (some referenced inline in post content, for URL-rewrite testing), and
# posts/pages split across the requested total — with a status mix
# (publish/draft/pending/future/private), nested pages, featured images,
# and custom fields. Safe to re-run: users/terms are reused if they already
# exist, posts/pages are simply added again, and media is additive — each
# run downloads new_media_count fresh images on top of whatever already
# exists (so the media manager has something new to show too).
#
# Requires: wp-cli (https://wp-cli.org) on PATH — install once with:
#   curl -sO https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar
#   chmod +x wp-cli.phar && sudo mv wp-cli.phar /usr/local/bin/wp

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WP_PATH="${WP_PATH:-/var/www/html/wordpress}"
TOTAL="${1:-200}"
NEW_MEDIA="${2:-15}"

if ! command -v wp >/dev/null 2>&1; then
    echo "wp-cli not found on PATH. Install it first:" >&2
    echo "  curl -sO https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar" >&2
    echo "  chmod +x wp-cli.phar && sudo mv wp-cli.phar /usr/local/bin/wp" >&2
    exit 1
fi

if [ ! -f "$WP_PATH/wp-load.php" ]; then
    echo "No WordPress install found at $WP_PATH (override with WP_PATH=...)." >&2
    exit 1
fi

echo "Populating $WP_PATH with $TOTAL posts/pages worth of test content ($NEW_MEDIA new media items)..."
wp eval-file "$SCRIPT_DIR/wp-test-data-populate.php" "$TOTAL" "$NEW_MEDIA" --allow-root --path="$WP_PATH"

echo
echo "Done. Export from WP Admin (Tools -> Export -> All Content), or run:"
echo "  wp export --dir=/tmp --allow-root --path=$WP_PATH"
