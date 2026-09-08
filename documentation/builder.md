---
title: Visual Page Builder
group: feature
updated_by: claude
last_updated: 2026-07-23
---
# Visual Page Builder

> Last updated: 2026-07-22 | Updated by: claude

## Overview

The Visual Page Builder is a drag-and-drop page composer for site admins/theme managers. It pairs a React admin UI built on **Puck** (`@puckeditor/core`) with a Rust backend that stores page layouts as JSON and renders them to public HTML via Tera block templates. It lets a site owner assemble pages (homepage, regular pages, and special "post template" / "archive template" wrapper pages) from a fixed palette of blocks (Hero, Header, Footer, Columns, Posts, Categories, Tags, Search, Form, etc.) without writing any Tera or HTML themselves.

## How It Works

### Project / Page / Draft-vs-Live model

- A **`builder_project`** (`core/src/models/builder_project.rs`, table `builder_projects`) is a named collection of pages for a site. Exactly one project per site can be `is_active` at a time (enforced by a partial unique index) — that is the project the live site actually serves.
- A **`page_composition`** (`core/src/models/page_composition.rs`, table `page_compositions`) belongs to a project and has a `page_type`: `"homepage"`, `"page"` (regular, with a unique `slug` per project), `"post_template"` (wraps every post/page URL), or `"archive_template"` (wraps category/tag archive URLs). Only one of each special type is allowed per project (enforced in the create-page handler, not the DB).
- Each composition has **two JSON columns**: `composition` (live — what visitors see) and `draft_composition` (work in progress — what the Puck editor reads and writes). `save_composition` only updates the draft; `publish_composition` copies the draft into both columns atomically, promoting it to live. This lets admins edit freely without affecting the public site until they explicitly click Publish.
- A project can only be activated (`activate_project` handler) if `count_published` finds at least one page whose live `composition->'content'` array is non-empty — you can't go live with zero published pages.
- Pages can be duplicated (`page_composition::duplicate`) — always as a new regular `"page"` type, copying the source's `draft_composition` and deriving a new slug (`{slug}-copy` or a slugified name).

### Admin UI (`admin/src/pages/builder.rs`, `core/src/handlers/admin/builder.rs`)

Routes are grouped: `/admin/builder` (project list), `/admin/builder/{project_id}` (page list), `/admin/builder/{project_id}/pages/new` (new page form, with an optional "Copy layout from" existing page), and `/admin/builder/{project_id}/pages/{page_id}` (the editor). All admin routes require `admin.caps.can_manage_themes`. The editor page (`render_editor`) is a thin HTML shell that mounts the React app at `#root`, passing initial state through `window.__builderInit` (page id/name, project id, site id, project/site labels, and the site's nav menus as JSON).

**Removed (2026-07-22): `/admin/builder2/{project_id}/pages/{page_id}` (`edit_page2`).** This near-duplicate "pure mode" route (hid the editor chrome via a `pureMode` flag) had no working entry point anywhere in the app — nothing linked to it — and the team doesn't recall what it was originally meant for. Deleted along with the now-dead `pure_mode` parameter on `render_editor` (always `false` now).

The editor talks to a small JSON API:
- `GET /admin/builder/load/{id}` — returns the page's `draft_composition`.
- `POST /admin/builder/save` — saves `data` (the Puck JSON) into the draft column via `save_composition`.
- `POST /admin/builder/publish` — promotes `data` to live via `publish_composition`; the frontend refuses to call this if `data.content` is empty.

### React editor (`admin/builder-ui/src/App.jsx`)

Wraps `@puckeditor/core`'s `<Puck>` component, configured with a `components` map from block name → React component (imported from `admin/builder-ui/src/blocks/`). On load it fetches the draft via `GET /admin/builder/load/:id`. Changes trigger `isDirty`/status-text state; an auto-save timer (`AUTO_SAVE_MS = 30_000`) calls `doSave` 30s after the last change, and a `beforeunload` handler warns on navigating away with unsaved changes. `handlePublish` posts to `/admin/builder/publish`. Header actions include a manual "Save Draft" button, a status indicator, and a link back to the site (skipped entirely in `pureMode`).

### Block architecture (React + Tera pairs)

Each block is a pair of files: a React component in `admin/builder-ui/src/blocks/` (defines the Puck field schema and the WYSIWYG render) and a matching Tera template in `themes/builder/blocks/` (renders the same block for real visitors). Current blocks: ArchivePosts, Button, Card/Cards, Categories, Columns, Div, Footer, Form, Header, Hero, Menu, Paragraph, PostContent, PostNavigation, Posts, Search, Tags, Text. `Sidebar.jsx` also exists in the blocks directory but is a planned block that was never finished — not imported/registered in `App.jsx`'s `config.components`, no matching Tera template. There's a lot more Puck builder work planned generally; this is one item on that list, not a bug, and isn't scheduled soon. Note also the naming mismatch: the Puck component key is `Cards` (mapped to `CardBlock` from `Card.jsx`) and its Tera template is `Cards.html`, while the source file itself is `Card.jsx`.

Per **CLAUDE.md**'s shared-layout convention, every top-level (section) block imports `PADDING_OPTIONS` and `MAX_WIDTH_OPTIONS` from `admin/builder-ui/src/blocks/ColorField.jsx` rather than defining local copies, so that setting e.g. "Standard (1200px)" produces the same max-width across different block types and content edges align down a page. `ColorField.jsx` also exports the `ColorField` component itself (a hex color swatch + `react-colorful` picker) used by blocks needing color pickers. Blocks dropped *inside* a zone (e.g. Text nested in Columns) don't need padding/max-width fields — they inherit layout from their container.

### Public rendering (`core/src/templates/composer.rs`, `core/src/templates/loader.rs`)

`composer::render_composition(composition_json, templates, site_ctx)` deserializes the saved Puck JSON into `PuckData { content: Vec<PuckBlock>, zones: HashMap<String, Vec<PuckBlock>> }` (`zones` are keyed `"{block_id}:{zone_name}"` for blocks like Columns that accept nested blocks via Puck's DropZone). For each top-level block it calls `render_block`, which:
1. Recursively pre-renders any of the block's zones into a `zone_html` map (stripping the `"{block_id}:"` prefix from zone keys).
2. Clones the site context and inserts `block_config` (the block's `props`), `block_id`, and `zone_html`.
3. Renders `{block_type}.html` via `templates.render_builder_block(...)`.

If a block's template fails to render, the error is logged and an HTML comment placeholder is emitted instead of failing the whole page. Some blocks (currently `Hero`, `Posts`) contribute extra responsive CSS injected once per block type into a `<style>` tag in the page `<head>` (via `block_css`). An empty composition (`content` array empty) renders a minimal blank HTML shell.

`TemplateEngine::render_builder_block` (`core/src/templates/loader.rs`) lazily loads every `.html` file in `themes/builder/blocks/` into a dedicated `"__builder__"` Tera instance (cached under that key, separate from the per-site/per-theme instances used for normal theme rendering), keyed by filename so template names match `{block_type}.html` exactly.

### Where composed pages get served

- **Homepage**: `core/src/handlers/home.rs` — if `page_composition::get_homepage(site_id)` finds an active project's homepage composition, it renders it via `composer::render_composition` instead of the theme's `index.html`.
- **Post/page single view**: `core/src/handlers/post.rs` — if a `post_template` composition exists for the site's active project, it wraps individual post/page URLs via the composer instead of `single.html`/`page.html`.
- **Category/tag archives**: `core/src/handlers/archive.rs` — if an `archive_template` composition exists, it wraps archive URLs via the composer instead of `archive.html`.

In all three cases, `enrich_builder_context` (`core/src/handlers/home.rs`) is called first to populate the Tera context with live DB data the blocks need: `builder_posts` (recent `PostContext` list), `builder_categories`/`builder_tags` (`TermContext` lists with post counts), and `builder_menus` (all nav menus for the site, for the Menu block to pick from).

## Database Schema

- **`builder_projects`** (migrations 0042, 0043): `id`, `site_id` (FK → sites, cascade delete), `name` (VARCHAR 35), `description` (VARCHAR 100, nullable), `is_active` (bool, partial-unique per site), `created_by` (FK → users, SET NULL), `created_at`/`updated_at`.
- **`page_compositions`** (migrations 0041, 0042, 0044, 0045, 0046→0047): `id`, `site_id` (FK, cascade), `project_id` (FK → builder_projects, cascade, added in 0042), `name`, `slug` (VARCHAR 100, nullable, unique per project when set, added in 0044), `page_type` (VARCHAR 20, default `'page'`, added in 0044 — replaces an earlier `is_homepage`-only model), `composition` (JSONB, live), `draft_composition` (JSONB, added in 0045 — seeded from `composition` on migration, so no in-progress work was lost), `is_homepage` (bool, partial-unique per site in 0041), `created_by`, `created_at`/`updated_at`. Migration 0046 added an `is_post_template` boolean flag which migration 0047 immediately dropped again in favor of the generalized `page_type` column.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/builder | `admin_builder::list` | Project list for the site |
| POST | /admin/builder/create | `admin_builder::create_project` | Create a project |
| POST | /admin/builder/deactivate | `admin_builder::deactivate_project` | Deactivate the site's live project |
| POST | /admin/builder/save | `admin_builder::save` | JSON API: save draft composition |
| POST | /admin/builder/publish | `admin_builder::publish` | JSON API: promote draft to live |
| GET | /admin/builder/load/{id} | `admin_builder::load` | JSON API: load draft composition |
| GET | /admin/builder/{project_id} | `admin_builder::project_pages` | Page list within a project |
| POST | /admin/builder/{project_id}/rename | `admin_builder::rename_project` | Rename/redescribe project |
| POST | /admin/builder/{project_id}/activate | `admin_builder::activate_project` | Set project live (requires ≥1 published page) |
| POST | /admin/builder/{project_id}/delete | `admin_builder::delete_project` | Delete project (blocked while active) |
| GET/POST | /admin/builder/{project_id}/pages/new | `admin_builder::new_page_form` / `create_page` | New page form / create |
| GET | /admin/builder/{project_id}/pages/{page_id} | `admin_builder::edit_page` | Editor (normal chrome) |
| POST | /admin/builder/{project_id}/pages/{page_id}/set-homepage | `admin_builder::set_homepage` | Mark page as project homepage |
| POST | /admin/builder/{project_id}/pages/{page_id}/duplicate | `admin_builder::duplicate_page` | Duplicate page |
| POST | /admin/builder/{project_id}/pages/{page_id}/delete | `admin_builder::delete_page` | Delete page |

## Security Notes

- Every admin builder route requires `AdminUser` plus `admin.caps.can_manage_themes`; the JSON save/load/publish endpoints return `403 Forbidden` (not a redirect) on failure since they're called via `fetch()`.
- Project/page ownership is re-verified against `site_id` on every mutating call (`builder_project::get_by_id(db, id, site_id)`), preventing cross-site access even with a guessed UUID.
- A project can't be deleted while `is_active` (must deactivate first), and can't be activated with zero published pages — both enforced server-side, not just in the UI.
- Composition JSON from the editor is stored as JSONB and re-rendered through Tera templates as `block_config` context variables (not as template source strings), consistent with the project's structural-sandbox rule against template injection.

## Known Limitations / TODOs

- `Sidebar.jsx` is present in the block source tree but unregistered in `App.jsx` and has no Tera counterpart — a planned block not yet built. Part of a larger backlog of planned Puck builder work, not scheduled soon.
- Special page types (`homepage`, `post_template`, `archive_template`) are capped at one-per-project by application logic in the handlers, not by a DB constraint.
