-- Migration: 0005_create_post_meta
-- Key-value store for custom fields registered by plugins

CREATE TABLE post_meta (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id     UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    meta_key    TEXT NOT NULL,
    meta_value  TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (post_id, meta_key)
);

CREATE INDEX idx_post_meta_post_id ON post_meta(post_id);
CREATE INDEX idx_post_meta_key ON post_meta(meta_key);
