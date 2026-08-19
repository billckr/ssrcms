-- Records the old WordPress attachment URL each imported media row came from,
-- so a future post-content importer can rewrite <img> references found in
-- imported post/page bodies without re-parsing the WXR export.
CREATE TABLE wp_import_media_map (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    old_url     TEXT NOT NULL,
    media_id    UUID NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, old_url)
);

CREATE INDEX idx_wp_import_media_map_site_id ON wp_import_media_map(site_id);
