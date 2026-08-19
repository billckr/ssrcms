# WordPress → SynapCMS Migration Pain Points

Working list of structural mismatches between WordPress and SynapCMS that will bite
agencies/freelancers migrating an existing WP site over. This is the source of truth
for planning the eventual **WP Import** feature — each item should get its own mapping
strategy (and ideally its own migration-script subtask) before import is built.

Status legend: `[ ]` not started · `[~]` in progress · `[x]` resolved

---

## [x] Permalinks

**WP:** `%postname%`, `%year%/%monthnum%/%postname%`, `/category/%postname%`, etc.,
configurable per-site under Settings → Permalinks.

**Synap:** Same preset list + custom structure now exists per-site
(`admin/src/pages/sites.rs`, `permalink_structure` column). Matching a site's old WP
structure means already-indexed/published links keep resolving after migration.

**Status:** Done (shipped last night, per-site presets + custom token structure).

---

## [~] Media library structure

**WP:**
- Files physically organized on disk as `/wp-content/uploads/YYYY/MM/original-name.jpg`
  — original filename preserved, human-readable path.
- On upload, WP auto-generates multiple resized variants (`thumbnail`, `medium`,
  `medium_large`, `large`, plus theme/plugin-registered custom sizes) and rewrites
  `<img srcset>` to reference them.
- Every attachment is its own row in `wp_posts` (`post_type = 'attachment'`) with
  `post_parent` pointing at whatever post/page it was uploaded through — media is
  "attached to" content, not just organized in folders.
- Inline `<img>` tags in `post_content` reference a *specific size variant's* filename
  (e.g. `photo-300x200.jpg`), not the original.

**Synap:** (`docs/media-library.md`)
- Files stored flat under `uploads/`, renamed to `<uuid>.<ext>` — original filename is
  kept only as metadata (`media.filename`), not part of the path.
- Single stored dimension per file; `width`/`height` columns exist but aren't even
  populated yet. No resized variants, no `srcset` generation.
- Folders (`media_folders` table, migration `0035`/`0036`) are a flat, single-level,
  site-scoped organizational label (`folder_id` on `media`) — not a WP-style date tree,
  and not nestable.
- No `post_parent`-equivalent link between a media row and the post it's embedded in.

**Why it's the biggest one:** a WP export's `post_content` is full of `<img>` tags
pointing at size-specific filenames in a `/YYYY/MM/` path. None of those paths or
filenames will exist post-import unless the importer rewrites every inline image URL
to the new `/uploads/{uuid}.ext` path *and* picks which original file to point at,
since Synap has no equivalent size variant to match WP's resized reference.

**Import plan (draft):**
1. Pull the original (largest) file for each WP attachment, ignore the resized
   variants — Synap doesn't need them.
2. Rewrite every `<img src>`/`srcset` occurrence found in `post_content` (regex on the
   WP upload path pattern) to the new Synap media URL.
3. Map WP's `/YYYY/MM/` structure to a single generated `media_folders` row per
   month (e.g. "2024-03") if we want to preserve *some* organizational signal, or skip
   folder migration entirely and leave everything flat — needs a decision.
4. Decide whether to keep original filenames in `media.filename` (yes — already
   supported) vs. discard.

**Status:** In progress. Shipped a first cut: **Import Content** tab on
Site Settings (`/admin/sites/{id}/settings?tab=import`). Upload a WXR export
(WordPress Tools → Export); it parses every `<item>` whose `<wp:post_type>`
is `attachment`, fetches each original file from its `<wp:attachment_url>`
(the old site must still be reachable), and imports it into that site's
media library — original filename preserved (decoded from the URL), alt text
pulled from `_wp_attachment_image_alt` postmeta, sorted into a
`media_folders` entry per upload year/month (`"unsorted"` if a post date is
missing). Each successful import is also recorded in `wp_import_media_map`
(`old_url` → `media_id`) for the future post-content importer to use when
rewriting `<img>` references. Implementation:
`core/src/handlers/admin/wp_import.rs` (WXR parsing + fetch loop, unit-tested),
`core/src/handlers/admin/media_store.rs` (storage logic shared with the
regular upload handler), the "Import Content" tab panel in
`admin/src/pages/sites.rs::render_settings`, migration
`0065_wp_import_media_map.sql`.

Originally this lived as its own page linked from the Media Library toolbar
(`/admin/media/import-wp`), but that toolbar is shared with the compact
media-browser iframe the sidebar's "Media" link and the featured-image
picker open as an overlay — navigating *inside* that iframe to a full-chrome
page left it stranded there (looked like "a second app window" on top of the
real one) until the whole outer page was reloaded. Moved to a Site Settings
tab instead, alongside Maintenance/Email Settings, since it's a whole-site
bulk operation, not a media-picker action.

**Update:** the post-content importer now exists too (see below) — media and
posts/pages import together from one WXR upload, and the `<img>`/`href`
rewriting this section originally called "not yet done" is implemented
(`rewrite_content_urls` in `wp_import.rs`, exact + size-suffix-stripped
fuzzy match).

**Update:** zip-upload fallback for when the old WP site isn't reachable is
now built too. The Import Content form takes an optional second file — a zip
of `wp-content/uploads/` at any nesting depth. `ZipMediaIndex` (`wp_import.rs`)
indexes every non-directory entry by its lowercased path components and
matches each attachment against it by `{year}/{month}/{filename}` suffix
first, falling back to filename alone only when that's unambiguous; anything
not found in the zip (or no zip uploaded at all) still falls back to the
existing HTTP fetch, so partial zips degrade gracefully instead of failing
the whole import. MIME type for zip-sourced files is guessed from the file
extension (`guess_mime_from_extension`) since there's no `Content-Type`
header to read, unlike the HTTP path.

---

## [x] Posts and pages (with categories, tags, featured images, custom fields)

**Status:** Shipped alongside the media importer, same "Import from
WordPress" form and WXR file. `core/src/handlers/admin/wp_import.rs::import`
now does both in one pass: attachments first (building an old-URL → new
media-URL map), then posts/pages, then a third pass to patch `parent_id`
now that every imported item has a real UUID (WXR item order doesn't
guarantee parents appear before children).

**What's included:** title, content (with inline `<img>`/`<a href>` URLs
rewritten to the imported media), excerpt, slug (deduped with a `-2`/`-3`
suffix on collision), status (`publish`/`draft`/`pending`/`future` map
directly; `private` maps to Draft — no real "private" visibility gate
exists to set automatically), published date (prefers `<wp:post_date_gmt>`),
parent page, comment-open/closed, categories and tags (get-or-created by
slug), featured image (via `_thumbnail_id` postmeta → the matching
attachment's URL → its imported media row), author (matched to an
*existing* Synap user by email, falling back to the importing admin), and
every other postmeta key copied verbatim into `post_meta` (so Yoast/RankMath
keys, ACF field values, etc. all survive the import even though nothing
reads them as anything but a flat string yet — see the two sections below).

**What's explicitly out of scope, per item, per the caveats already on this
list:**
- Custom post types are skipped and counted, not imported as anything.
- Trash / auto-draft / inherit-status items are skipped as not real content.
- Shortcodes and Gutenberg block comments in content are left as literal
  text — no shortcode runtime exists to execute them.
- `<iframe>`/`<script>` tags (WP embeds) get stripped by the existing
  `sanitize_content` HTML sanitizer, same as they would for any other post.
- WP user passwords are never touched — author matching only links to an
  *already-existing* Synap account by email; no new accounts are created and
  no password is imported or set.
- Re-running the importer with overlapping content creates duplicate posts
  (the slug-dedupe just makes each one a valid separate row, e.g.
  `my-post` and `my-post-2`) — there's no "skip if already imported"
  idempotency for posts the way there now is for media (which does dedupe
  re-imports via `wp_import_media_map`).
- `<wp:post_date>` is treated as UTC when `<wp:post_date_gmt>` is missing or
  zeroed — exact only if the source WP site's timezone was UTC.

---

## [ ] Custom fields / post meta

**WP:** `wp_postmeta` allows arbitrary serialized PHP values per key, and the *same*
key can repeat multiple times on one post (how ACF repeater fields, gallery ID lists,
and multi-value taxonomies-as-meta are stored). Plugins routinely store nested
arrays/objects here.

**Synap:** `post_meta` (`core/src/models/post.rs::get_meta`/`set_meta`) is a flat
`meta_key → meta_value` string map, one value per key, upserted via
`ON CONFLICT (post_id, meta_key) DO UPDATE`.

**Why it's a pain point:** simple ACF text/number/select fields map 1:1. Anything
that's a WP serialized array (repeaters, relationship fields, flexible content, ACF
Gallery, Yoast's internal arrays) does **not** map cleanly — needs either flattening
into multiple synthetic keys (`field_0_name`, `field_1_name`, ...) or dropping.

**Status:** Not started — but the post/page importer above now copies every
WP postmeta key verbatim into `post_meta` as a flat string during import, so
simple single-value fields are already sitting there for whatever reads
`post_meta` next. Serialized-array values (repeaters, ACF Gallery, etc.)
land as their raw PHP-serialized string (e.g. `a:1:{s:5:"width";i:1200;}`)
— readable by nothing until this item is actually built.

---

## [ ] SEO plugin metadata (Yoast / RankMath / All in One SEO)

**WP:** SEO title, meta description, canonical URL, OG image, focus keyword, and
redirects are stored as postmeta under plugin-specific keys
(`_yoast_wpseo_metadesc`, `_yoast_wpseo_title`, `rank_math_description`, etc.).

**Synap:** Has its own SEO & Structured Data plugin (the Phase 2 MVP plugin per
`CLAUDE.md`) with its own meta key naming.

**Why it's a pain point:** losing SEO metadata on migration is one of the most
visible regressions a client will notice (search snippets changing, lost redirects).
Needs an explicit per-plugin key-mapping table (Yoast keys → Synap SEO plugin keys,
RankMath keys → same) so at minimum title/description/canonical/OG image survive.

**Status:** Not started as a real feature — but same as above, the raw
Yoast/RankMath postmeta keys (`_yoast_wpseo_metadesc`, `_yoast_wpseo_title`,
`rank_math_description`, ...) are already copied verbatim into `post_meta`
by the post importer. The key-mapping table into Synap's own SEO plugin
fields is still the open work.

---

## [ ] Shortcodes and widgets (no runtime equivalent)

**WP:** Classic-editor content is full of shortcodes emitted by plugins —
`[gallery ids="1,2,3"]`, `[contact-form-7 id="12"]`, `[wpforms id="4"]`,
`[embed]`, WooCommerce shortcodes, etc. Sidebars/footers are built from widget areas
(Recent Posts, Tag Cloud, Custom HTML, Calendar, third-party widgets) rather than
page content.

**Synap:** Plugin system is Tera-template-based but currently **paused indefinitely**
(no plugin admin routes enabled) — there is no shortcode-execution runtime at all.
Layout composition instead happens through the Puck-based visual Page Builder
(section/block model), which has no widget-area concept.

**Why it's a pain point:** any imported `post_content` containing a shortcode will
render as literal bracket text on the front end — silently broken, not an error.
Widget-driven sidebars/footers have nothing to import into structurally; that content
would need to be manually rebuilt as Page Builder blocks.

**Import plan (draft):** strip or flag known shortcode patterns during import (at
minimum surface a "N shortcodes found, review manually" warning per post) rather than
attempting automatic conversion. True automatic conversion (e.g. `[gallery]` →
a Puck gallery block) is a stretch goal, not a v1 requirement.

**Status:** Not started.

---

## [ ] Forms (Gravity Forms / WPForms / Contact Form 7)

**WP:** Form structure + submission history live in plugin-specific tables with
plugin-specific field-type schemas.

**Synap:** Has its own Form Designer (Designer hub, "New Form"). Field types and
storage schema don't match any WP forms plugin.

**Why it's a pain point:** even mapping the form *shell* (fields, labels, required
flags) is plugin-specific work per popular plugin, and submission history almost
certainly cannot be carried over at all — different data model, no shared ID space.

**Status:** Not started. Lower priority than content/media — most clients can
recreate a contact form manually in an afternoon.

---

## [ ] User accounts & passwords

**WP:** Password hashes stored as phpass (or, in modern WP 6.8+, bcrypt via
`password_hash()`). Roles are WP's built-in five (subscriber/contributor/author/
editor/administrator) plus anything plugins register.

**Synap:** Has its own multi-role-per-site system (`SiteRole` enum, `site_users`
join table).

**Why it's a pain point:** WP password hashes cannot be reused as-is unless Synap's
auth layer specifically supports verifying (and upgrading) phpass/WP-bcrypt hashes —
otherwise every migrated user needs a forced password reset on first login. Role
mapping from WP's 5 roles to Synap's `SiteRole` set should be close to 1:1 but needs
an explicit table, especially for any custom roles a WP plugin (e.g. WooCommerce's
"customer"/"shop_manager") registered.

**Status:** Not started.

---

## [ ] Custom post types

**WP:** Plugins/themes commonly register additional post types (Portfolio,
Testimonials, Products, Events) with their own admin list screens and taxonomies.

**Synap:** `post.post_type` is a free-form string column, but only `post` and `page`
are actually wired up through the admin UI, routing, and templates today.

**Why it's a pain point:** a WP export containing custom post types has no home in
Synap without deciding, per post type, whether to (a) import as regular Posts with a
tag/category marking the original type, (b) drop entirely, or (c) build real custom
post type support first (bigger scope, not just an import-mapping problem).

**Status:** Not started. Needs a scoping decision before import work starts — this
one might block on a bigger feature rather than just being an import-script detail.

---

## [ ] Comments

**WP:** Threaded comments with moderation status, Akismet-style spam filtering
plugins, and `wp_commentmeta` for plugin extensions (e.g. reCAPTCHA verification
data).

**Synap:** Has its own `comment.rs` model (threaded, with pagination). No
`comment_meta`-equivalent, and no spam-filtering integration yet.

**Why it's a pain point:** lower severity than the above — structurally close enough
that a straight import (author, content, timestamp, parent, approval status) should
work for the common case. Spam-flagged comments and Akismet-specific data would just
be dropped, which is an acceptable loss.

**Status:** Not started. Lowest priority on this list.

---

## Suggested working order

1. ~~**Media library structure**~~ — done.
2. ~~**Posts and pages**~~ — done, shipped together with media (one importer,
   one WXR upload).
3. **SEO metadata mapping** — high visibility to clients, moderate effort (mostly a
   key-mapping table) — raw values are already sitting in `post_meta`.
4. **Custom fields / post meta as a real feature** — reading the raw values already
   imported into something the admin UI/templates can actually use.
5. **Shortcodes/widgets** — can ship as "detect and flag" before attempting real
   conversion.
6. **User accounts & passwords** — needed for any real client cutover, not just
   content migration.
7. **Custom post types** — needs a scoping decision first; may not be v1.
8. **Forms** — lower priority, most clients rebuild manually.
9. **Comments** — lowest priority, mostly a straightforward import.
