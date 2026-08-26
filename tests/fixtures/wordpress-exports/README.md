# WordPress import fixture

A pre-generated WXR export + media zip for manually exercising "Import from
WordPress" (Site Settings -> Import Content tab, `core/src/handlers/admin/wp_import.rs`)
at realistic scale, without needing a live WordPress instance.

- `site.wxr` -- 250 posts across every status (publish/draft/pending/future/
  private), 2 custom pages + WP's own default pages, 150 media attachments,
  20 categories (5 nested), 20 tags, Yoast/RankMath/ACF-style postmeta
  (including a real serialized array), shortcodes and an `<iframe>` left as
  literal text, threaded comments, plus one trashed post, one auto-draft,
  and one custom-post-type item -- all to exercise both what the importer
  handles and what it's meant to skip (see
  `docs/wordpress-migration-pain-points.md`).
- `uploads.zip` -- the matching `wp-content/uploads/` tree (750 files: 150
  originals x 5 WordPress-generated size variants each).

## Usage

Upload both files together via the **zip fallback path**: `site.wxr`'s
`<wp:attachment_url>` entries point at `http://localhost:8080/...`, which
only resolves while the source WordPress container is running. Since the
whole point of this fixture is not needing that container anymore, always
attach `uploads.zip` alongside `site.wxr` in the import form so the importer
resolves attachments from the zip instead of trying to fetch them over HTTP.

## Regenerating

Source generator lives outside this repo at `~/Projects/wp-fixture`
(Docker Compose: MariaDB + WordPress + WP-CLI, entirely on named volumes).
To produce a fresh export:

```
cd ~/Projects/wp-fixture
./regenerate.sh
```

This wipes and rebuilds the WP instance, regenerates all content, and drops
`site.wxr` + `uploads.zip` in `~/Downloads`. Copy them here to replace these
fixtures.
