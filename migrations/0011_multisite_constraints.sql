-- Finalise multi-site constraints.
-- Safe to run before `synaptic-cli site init` (no data changes required).

-- Composite slug uniqueness replaces global uniqueness on posts.
-- NOTE: NULLs are treated as distinct in PostgreSQL UNIQUE constraints,
-- so (NULL, slug) rows are considered unique from each other, which is
-- safe until `site init` backfills all site_id values.
ALTER TABLE posts DROP CONSTRAINT IF EXISTS posts_slug_key;
ALTER TABLE posts ADD CONSTRAINT posts_site_slug_unique UNIQUE (site_id, slug);

-- Composite slug uniqueness on taxonomies.
ALTER TABLE taxonomies DROP CONSTRAINT IF EXISTS taxonomies_slug_taxonomy_key;
ALTER TABLE taxonomies ADD CONSTRAINT taxonomies_site_slug_tax_unique UNIQUE (site_id, slug, taxonomy);

-- site_settings: add a partial unique index for non-NULL site_ids.
-- We cannot add a composite PRIMARY KEY here because existing rows may have
-- site_id IS NULL (populated by `synaptic-cli site init` after this migration).
-- The application upserts use ON CONFLICT with this partial index.
CREATE UNIQUE INDEX IF NOT EXISTS site_settings_site_key_idx
    ON site_settings (site_id, key)
    WHERE site_id IS NOT NULL;
