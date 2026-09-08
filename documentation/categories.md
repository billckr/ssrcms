---
title: Categories
group: feature
updated_by: claude
last_updated: 2026-07-22
---
# Categories

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Categories are a taxonomy type used to classify posts. They are stored in the `taxonomies` table with `taxonomy = 'category'`. Each category has a name, a URL-safe slug, and an optional description. Categories are site-scoped and can be assigned to multiple posts; posts can have multiple categories.

## How It Works

### Model (`core/src/models/taxonomy.rs`)

Key structs:
- `Taxonomy` — DB row: `id`, `site_id`, `name`, `slug`, `taxonomy`, `description`, `created_at`
- `TermContext` — template-safe view with `id`, `name`, `slug`, `taxonomy`, `url` (e.g. `{base_url}/category/{slug}`), `post_count`
- `CreateTaxonomy` — input struct

Key functions: `create`, `get_by_id`, `get_by_slug`, `list` (filtered by taxonomy type and site), `for_post`, `attach_to_post`, `detach_from_post`, `post_count`, `delete`.

### Admin Handler (`core/src/handlers/admin/taxonomy.rs`)

- `categories` — lists all categories for the site with their published post counts. Requires `can_manage_taxonomies`.
- `create` — accepts `TermForm` with `name`, optional `slug`, and `taxonomy`. Validates slug via `is_valid_slug`. Handles duplicate-key errors with a user-friendly message.

The "Add Category" form (`admin/src/pages/taxonomy.rs::render`, updated 2026-07-22) is wrapped in the same `.profile-container` card used on `/admin/users/new`, the submit button stays disabled until Name has non-whitespace text, and the Slug field is live-filled from Name as you type (client-side `toSlug()`, mirroring the server's `crate::utils::slugify::slugify`) rather than only being generated server-side on submit — editing Slug directly stops the auto-sync. The heading/button read "Add Category" (previously "Add New Categories").
- `delete_category` — enforces site ownership for non-global-admins before deletion.

Slug validation is performed by `crate::utils::slugify::is_valid_slug` — slugs must be lowercase letters, numbers, and hyphens only.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/categories | `taxonomy::categories` | List categories |
| POST | /admin/categories/new | `taxonomy::create` | Create category |
| POST | /admin/categories/{id}/delete | `taxonomy::delete_category` | Delete category |
| GET | /category/{slug} | `archive::category_archive` | Public category archive |

## Database Schema

`taxonomies` table: `id UUID PK`, `site_id UUID`, `name TEXT`, `slug TEXT`, `taxonomy TEXT` (category/tag), `description TEXT`, `created_at TIMESTAMPTZ`.

`post_taxonomies` join table: `post_id UUID`, `taxonomy_id UUID` — composite PK. Insert is idempotent via `ON CONFLICT DO NOTHING`.

## Security Notes

Only editors and above (`can_manage_taxonomies`) can manage categories. Site isolation: non-global-admins can only delete categories that belong to their current site.
