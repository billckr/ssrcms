-- Create sites table for multi-site support.
-- Each site is identified by its hostname (Host header).
CREATE TABLE sites (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hostname     TEXT NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sites_hostname ON sites(hostname);
