-- Migration 0014: Fix site_settings primary key to support per-site rows.
--
-- Problem: migration 0006 created `key TEXT PRIMARY KEY`.  After migration
-- 0010 added the nullable site_id column, the PK still covers only `key`,
-- so inserting a per-site row (site_id = <uuid>, key = 'active_theme') fails
-- with a PK violation because the global seed row already owns that key value.
--
-- Fix: drop the single-column PK and replace it with two partial unique
-- indexes — one for global rows (site_id IS NULL) and one for per-site rows
-- (site_id IS NOT NULL, already created in migration 0011).

ALTER TABLE site_settings DROP CONSTRAINT IF EXISTS site_settings_pkey;

-- Unique constraint for global/legacy rows (site_id IS NULL).
CREATE UNIQUE INDEX IF NOT EXISTS site_settings_global_key_idx
    ON site_settings (key)
    WHERE site_id IS NULL;
