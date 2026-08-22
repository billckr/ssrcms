-- Records which Synap post a WordPress WXR item (by its wp:post_id) became,
-- so re-uploading the same export (e.g. this time with a media zip attached,
-- to backfill images that failed to download on the first pass) updates the
-- existing post instead of creating a duplicate.
CREATE TABLE wp_import_post_map (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    wp_post_id  TEXT NOT NULL,
    post_id     UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, wp_post_id)
);

CREATE INDEX idx_wp_import_post_map_site_id ON wp_import_post_map(site_id);
