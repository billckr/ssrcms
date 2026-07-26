-- Migration: 0050_add_post_sources
-- Adds an optional list of source URLs to posts/pages, with a toggle
-- controlling whether the list is shown on the live page or admin-only.

ALTER TABLE posts ADD COLUMN IF NOT EXISTS sources JSONB NOT NULL DEFAULT '[]';
ALTER TABLE posts ADD COLUMN IF NOT EXISTS sources_public BOOLEAN NOT NULL DEFAULT FALSE;
