-- Tracks forms that have been administratively disabled for a site.
-- When a form_name is present in this table for a site, POST /form/{name}
-- will reject submissions silently (redirect to ?blocked=1).
CREATE TABLE form_blocks (
    site_id   UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    form_name TEXT NOT NULL,
    blocked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (site_id, form_name)
);
