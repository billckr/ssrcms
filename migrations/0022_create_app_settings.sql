-- App-wide key-value settings table.
-- Holds installation-level configuration that applies across all sites.
-- Only super_admin can edit these, via /admin/settings.
-- Not to be confused with site_settings (per-site) or AppConfig (.env / synaptic.toml).

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL DEFAULT ''
);

-- Seed sensible defaults. ON CONFLICT DO NOTHING so re-running the migration
-- on an existing installation does not overwrite values the agency has changed.
INSERT INTO app_settings (key, value) VALUES
    ('app_name',      'Synaptic'),
    ('timezone',      'UTC'),
    ('max_upload_mb', '25')
ON CONFLICT (key) DO NOTHING;
