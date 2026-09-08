---
title: Tags
group: feature
updated_by: claude
last_updated: 2026-07-22
---
# Tags

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Tags are a taxonomy type used to label posts with fine-grained keywords. They are stored in the `taxonomies` table with `taxonomy = 'tag'`. Tags are site-scoped and otherwise behave identically to categories in storage and code paths.

## How It Works

Tags share all model functions in `core/src/models/taxonomy.rs` with categories — the `TaxonomyType::Tag` enum variant routes queries to filter `taxonomy = 'tag'`. Template context is built via `TermContext::from_taxonomy`, which produces a `url` of the form `{base_url}/tag/{slug}`.

### Admin Handler (`core/src/handlers/admin/taxonomy.rs`)

- `tags` — lists all tags for the site with published post counts. Requires `can_manage_taxonomies`.
- `create` — the same handler is used for both categories and tags. The `taxonomy` form field (`"category"` or `"tag"`) determines which type is created.

The "Add Tag" form (`admin/src/pages/taxonomy.rs::render`, updated 2026-07-22) shares its template with categories: same `.profile-container` card styling as `/admin/users/new`, submit disabled until Name is non-empty, and a live client-side slug preview as you type Name (stops auto-syncing once Slug is edited directly). Heading/button read "Add Tag" (previously "Add New Tags").
- `delete_tag` — enforces site ownership for non-global-admins before deletion.

### Post Editor Integration

When creating or editing a post, `fetch_term_options` queries both `TaxonomyType::Category` and `TaxonomyType::Tag` for the current site and passes them to the `PostEdit` view as selectable options. On save, `save_post_terms` detaches all existing taxonomy associations and reattaches the submitted set.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/tags | `taxonomy::tags` | List tags |
| POST | /admin/tags/new | `taxonomy::create` | Create tag |
| POST | /admin/tags/{id}/delete | `taxonomy::delete_tag` | Delete tag |
| GET | /tag/{slug} | `archive::tag_archive` | Public tag archive |

## Database Schema

Tags use the same `taxonomies` table as categories, with `taxonomy = 'tag'`. Post-tag associations are in `post_taxonomies`.

## Security Notes

Identical to categories: editor or above required, site isolation enforced on delete.
