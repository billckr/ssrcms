-- Migration: 0006_create_site_settings
-- Global site configuration as a key-value store

CREATE TABLE site_settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default settings
INSERT INTO site_settings (key, value, description) VALUES
    ('site_name',        'Synaptic Signals',          'Site display name'),
    ('site_description', 'Fast by default, secure by design', 'Site tagline'),
    ('site_url',         'http://localhost:3000',      'Canonical base URL'),
    ('site_language',    'en-US',                      'BCP-47 language code'),
    ('active_theme',     'default',                    'Active theme directory name'),
    ('posts_per_page',   '10',                         'Number of posts per archive page'),
    ('date_format',      '%B %-d, %Y',                 'Default date display format'),
    ('admin_email',      '',                           'Administrator email address');
