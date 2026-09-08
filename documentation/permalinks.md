---
title: Permalinks
group: feature
updated_by: claude
last_updated: 2026-08-19
---

# Permalinks

> Last updated: 2026-08-18 | Updated by: claude

## Overview

Per-site, WordPress-style configurable URL structure for **posts**. Lets a
site migrating off WordPress keep its old permalink structure (e.g.
`/%year%/%monthnum%/%postname%/`) so already-published and already-indexed
URLs keep resolving after the switch — the whole point of the feature.
**Pages are unaffected** — they always use their existing flat or
hierarchical slug path, regardless of this setting, matching WordPress's own
behavior (Permalinks only ever governs post URLs there too).

## How It Works

### Configuration

Each site has a `permalink_structure` setting (default `/%postname%`,
identical to this app's original bare-slug behavior — no site's URLs change
until an admin opts in). Editable from **Site Settings → General → Permalinks**
(`admin/src/pages/sites.rs`), which offers WordPress's familiar presets
(Post name, Month and name, Day and name, Category, Custom Structure) that
populate a single underlying text field, plus a live JS preview.

Supported tokens: `%postname%` (required, must be the final token — see
below), `%year%`, `%monthnum%`, `%day%`, `%hour%`, `%minute%`, `%second%`,
`%post_id%`, `%category%` (falls back to `uncategorized` if the post has no
category — WordPress's own fallback).

Saving validates that the structure ends with `%postname%` or `%postname%/`
(`core/src/handlers/admin/sites.rs::save_site_config`) — rejected otherwise,
since request resolution depends on the postname always being the final
path segment.

### URL generation

`core/src/models/post::build_permalink(structure, post, category_slug)` does
the token substitution, using `published_at` (falling back to `created_at`
for an unpublished post, e.g. a preview link) for date tokens. This is the
single choke point `PostContext::build()` calls for non-page posts, so every
public-facing URL — templates, RSS feeds, archive listings, `sitemap.xml`,
and the theme API's `url_for()`/`posts()` Tera functions — picks up the
site's configured structure automatically. The admin posts list and post
editor's "View" links (`core/src/handlers/admin/posts.rs`) were updated to
use the same function, so what an editor sees in `/admin/posts` matches what
a visitor actually gets.

### Request resolution (decorative segments)

`core/src/handlers/page.rs::single_page` is the fallback for any URL Axum
can't otherwise match. When a multi-segment path doesn't resolve as a page
hierarchy, `try_post_permalink()` retries it by its **last path segment**
as a post slug — deliberately without validating the earlier (date/category)
segments against the site's configured structure. This is safe because
slugs are already unique per site (`posts_site_slug_unique`), so there's
nothing to disambiguate.

- If the request matches the post's current canonical URL exactly, it
  renders directly (full parity with `/{slug}` — password gate, unique-view
  tracking, comments — via the shared `post::render_single_post_response`).
- If it doesn't match (a stale date, a renamed category, or the structure
  changed since the link was published), it issues a **301 redirect** to
  the true canonical URL — self-correcting instead of serving the same post
  at infinitely many path variations, which is bad for SEO.
- The original bare `/{slug}` route (`post_handler::single_post`) keeps
  working **unconditionally**, regardless of the configured structure. This
  is deliberate: an already-published/indexed link can never break just
  because a site later switches to a date- or category-prefixed structure.

### Known collision (edge case)

Fixed routes (`/category/{slug}`, `/tag/{slug}`, `/author/{username}`) are
registered ahead of the fallback. If a site uses a `%category%`-prefixed
structure and a real category happens to be named `category`, `tag`, or
`author`, a post permalink under that category would be shadowed by the
matching archive route instead of reaching `try_post_permalink`. Not solved
today — a real edge case, not a design flaw in the common case.

## Database Schema

No new table — stored as a row in the existing `site_settings` key/value
table: `(site_id, key = 'permalink_structure', value = '<structure string>')`.
Loaded into `SiteSettings.permalink_structure`
(`core/src/app_state.rs`), same caching/reload pattern as every other
per-site setting.

## Security Notes

The 301 redirect target is always built server-side from the resolved
post's own real data (`build_permalink`) — never echoes back attacker-
controlled path segments, so there's no open-redirect surface here.

## Known Limitations / TODOs

- Secondary URL builders that predate this feature — breadcrumbs, the
  saved-posts account page, comment-submit and post-unlock redirects —
  still build flat `/{slug}` URLs. They still resolve correctly (via the
  bare-slug fallback), just don't display in the site's configured
  structure. Not yet updated for full display consistency.
- No admin UI or DB flag distinguishes "this site's permalinks were changed
  recently" — no bulk redirect map or rewrite history; each old URL is
  resolved individually, on request, via the last-segment lookup above.

