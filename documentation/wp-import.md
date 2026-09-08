---
title: WordPress Import
group: feature
updated_by: claude
last_updated: 2026-08-22
---
# WordPress Import

> Last updated: 2026-08-22 | Updated by: claude

## Overview

SynapCMS can import content from a WordPress WXR export (`Tools → Export → All content` in WP, an XML file). The importer lives on the **Import Content** tab of a site's Settings page (`/admin/sites/{id}/settings?tab=import`), not the Media Library — it used to be a link inside the Media Library's picker toolbar, but that trapped the compact media-browser iframe on a full-chrome page when clicked from inside it, so it was moved out to its own settings tab. Implementation: `core/src/handlers/admin/wp_import.rs`.

A single upload runs media import and content import together, in one pass:

1. **Attachments** are imported into the Media Library first (so featured images and in-content `<img>`s can be rewritten to the new URLs before posts are created).
2. **Authors** are matched or created (see below).
3. **Posts and Pages** are imported, with categories/tags, featured images, custom fields, and parent/child page relationships.

Every run writes a summary flash message, and a more detailed trace (including exactly which items were skipped and why) to the server log — see `./app.sh logs`.

**Re-imports update in place, not duplicate (added 2026-08-22):** `wp_import_post_map` records which Synap post each WXR item (`<wp:post_id>`) became (parallel to `wp_import_media_map` for attachments). `import_post` checks this table first: if the item was imported before for this site, the existing post is updated instead of a new one being created. This is what makes it safe to re-upload the *same* export a second time — the main use case being adding a media zip you didn't have on the first pass, so images that failed to download over HTTP the first time get filled in. Specifics:
- The slug is never touched on update (it's a public URL, not something a re-import should shift).
- `featured_image_id` is only ever *filled in*, never cleared or replaced — if a prior run already resolved it, a run where it doesn't resolve (e.g. still no matching zip entry) leaves it alone rather than nulling it out.
- Title, content (re-rewritten against whatever media/URL map exists *this* run), excerpt, status, and published date are otherwise overwritten to match the export every time — so a manual edit made in the admin after import will be lost on a re-import. The Import Content tab's UI copy calls this out.
- Categories/tags (`attach_to_post`) and custom fields (`set_meta`) were already idempotent (`ON CONFLICT DO NOTHING` / upsert), so re-running was always safe for those.

**Search indexing (added 2026-08-22):** imported posts/pages are created directly via the DB (`create_post_unique_slug`), bypassing the normal admin post handlers' per-post `search::indexer::index_post` call on publish — so without this, imported content stayed unsearchable until the next restart or a manual reindex. `run_import` now calls `search::indexer::rebuild_index` once, at the end of Pass 3 (after parent_id linking, before the final progress write), the same full-index rebuild the admin UI's "Rebuild Search Index" button and `synap search reindex` trigger — one batch commit covering every published post across every site, not scoped to just the imported ones (see the **Search** doc's On-Demand Reindex section). A rebuild failure only logs a warning; it doesn't fail the import or change the flash message.

## What's Supported

- **Posts and Pages** — WP's two built-in post types. Title, content, excerpt, slug (de-duplicated with a `-2`, `-3`, ... suffix on collision), status, publish date, comments-open/closed.
- **Status mapping**: `publish` → Published, `draft` → Draft, `pending` → Pending, `future` → Scheduled, `private` → Draft (Synap has no private-visibility gate, so this is the safer default rather than publishing something meant to be private). `trash`, `auto-draft`, and `inherit` (attachments' own status) are skipped as not real content.
- **Media / attachments** — downloaded and added to the Media Library, organized into one `media_folder` per year/month (matching the upload date), same as WP's own `/wp-content/uploads/YYYY/MM/` layout. Re-running an import (e.g. a newer export from the same site, or the same export with a media zip added) reuses already-imported media instead of re-downloading it, tracked in the `wp_import_media_map` table, and updates the existing posts instead of duplicating them, tracked in `wp_import_post_map` (see below).
- **Content URL rewriting** — `<img src>`/`<a href>` references to the old site's media URLs inside imported post content are rewritten to the new Synap media URLs, including a fuzzy match that strips WP's auto-generated size suffixes (`-300x200`) when the exact resized file wasn't itself an attachment in the export.
- **Featured images** — resolved via the WP `_thumbnail_id` postmeta pointing at an imported attachment.
- **Categories and Tags** — WP's two built-in taxonomies (`category`, `post_tag`); matched or created by slug. Any other custom `<category domain="...">` a plugin registered is skipped.
- **Custom fields** — every other postmeta key/value pair is copied verbatim onto the post's custom fields, *except* WP's own internal housekeeping keys (`_edit_lock`, `_edit_last`, `_wp_old_slug`, `_wp_old_date`, `_wp_desired_post_slug`, `_thumbnail_id`), which are dropped since they're meaningless in Synap.
- **Page hierarchy** — WP `post_parent` is resolved to the new Synap page's `parent_id` in a second pass, once every item in the export has been imported (so parent/child order in the file doesn't matter).
- **Authors** — each WP author (`<wp:author>`) is matched to an existing Synap user by email. If no match exists, a new Synap account is created automatically: role `author`, granted `author` access on this site, `can_self_publish` off by default, with a randomly generated password. The generated username/password pairs are shown in the post-import flash message so the admin doing the import can hand them out — there's no self-service password recovery for staff accounts yet, so that message is currently the only place to get them (see the **Users & Roles** doc). Authors with no email on file in the export (some minimal WXR files omit it) can't be matched or created; their posts are assigned to whoever ran the import instead. If a matched author has no existing access to *this* site (e.g. they were matched by email but only ever had an account/role on a different Synap site — see "WP Multisite" below), they're granted `author` access here too, but only if they hold no role on this site yet, so re-running an import never resets an existing `can_self_publish` grant back to off.

## What's Not Supported

- **User passwords** — WXR never includes WP password hashes (a different hashing scheme anyway), so this can't be imported under any circumstances. New accounts get a random Synap password instead (see above).
- **Custom post types** — anything other than `post`/`page` (e.g. WooCommerce products, a plugin-registered CPT) is skipped and counted in the flash message as "skipped (unsupported type)".
- **WP-internal block/site-editor data** — `nav_menu_item`, `wp_navigation`, `wp_global_styles`, `wp_template`, `wp_template_part`, `wp_block`, `wp_font_family`, `wp_font_face`, `custom_css`, `customize_changeset`, `oembed_cache`, `user_request`. These are WP's own editor plumbing, never real content — they're silently skipped without cluttering the flash message (still visible in the server log if you want the detail).
- **Comments** — individual `<wp:comment>` entries (the actual comment text/authors on a post) are not parsed or imported at all right now, only the post-level open/closed toggle.
- **SEO plugin metadata** — Yoast/RankMath/etc. fields land as raw custom-field text (via the generic postmeta copy above) but aren't understood or surfaced by Synap's own SEO features.
- **Shortcodes and Gutenberg block comments** — left as literal text inside imported content; Synap has no shortcode runtime, so `[gallery ids="1,2,3"]` or `<!-- wp:paragraph -->` markers render as-is rather than being interpreted.
- **Widgets** — not imported; WP's sidebar/widget-area export isn't part of WXR's content items and has no Synap equivalent (theme layout, not content).
- **Forms** — plugin-based forms (Gravity Forms, CF7, etc.) aren't part of WXR content and aren't imported; use Synap's own Form Designer to rebuild them.
- **Private-post visibility** — WP's `private` status has no direct Synap equivalent (no password/visibility gate is set automatically); imported as Draft instead so nothing meant to be private goes live unreviewed.

See `docs/wordpress-migration-pain-points.md` in the repo for the fuller migration-planning writeup these gaps come from, including suggested working order for closing them.

## WP Multisite

WordPress multisite has no network-wide export — each subsite exports its own WXR file (`Tools -> Export` run from within that subsite). Migrating a multisite network means running this importer once per subsite, each into its corresponding Synap site; there's no bulk/network-level import.

If the same person authors on more than one subsite, they'll typically share one email address across those subsites' exports. The importer takes advantage of that: an author matched by email is reused as the same Synap user across every site you import into, rather than creating a duplicate account per site (see above).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /admin/sites/{id}/import-wp | `wp_import::import` | Upload a WXR file and run the import (Import Content tab) |

## Security Notes

- Requires site-manager permission on the target site (`require_site_manager` — the same gate as other site-settings actions).
- The uploaded file is parsed in-memory as XML (`quick_xml`); it is never rendered as a Tera template, so it can't reach the template-injection surface the plugin sandbox guards against.
- Attachment downloads are fetched server-side with a dedicated `SynapCMS-WP-Importer/1.0` user agent; a failed download just counts toward "media item(s) failed to import" rather than failing the whole run.




