-- Add site_id to all content tables (nullable initially for safe backfill).
-- Run `synaptic-cli site init --hostname <domain>` after this migration to
-- create the first site row and backfill existing content.

ALTER TABLE posts      ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE CASCADE;
ALTER TABLE taxonomies ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE CASCADE;
ALTER TABLE media      ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE CASCADE;

-- Add site_id to site_settings (nullable initially for safe backfill).
ALTER TABLE site_settings ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE CASCADE;
