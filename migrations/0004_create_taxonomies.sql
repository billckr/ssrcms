-- Migration: 0004_create_taxonomies
-- Categories and tags share this table, distinguished by taxonomy type

CREATE TABLE taxonomies (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL,
    taxonomy    TEXT NOT NULL DEFAULT 'category'
                    CHECK (taxonomy IN ('category', 'tag')),
    description TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (slug, taxonomy)
);

CREATE INDEX idx_taxonomies_slug ON taxonomies(slug);
CREATE INDEX idx_taxonomies_taxonomy ON taxonomies(taxonomy);

-- Join table: many-to-many posts <-> taxonomies
CREATE TABLE post_taxonomies (
    post_id     UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    taxonomy_id UUID NOT NULL REFERENCES taxonomies(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, taxonomy_id)
);

CREATE INDEX idx_post_taxonomies_taxonomy_id ON post_taxonomies(taxonomy_id);
