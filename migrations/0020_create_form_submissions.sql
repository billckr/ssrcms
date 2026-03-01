-- Migration: 0020_create_form_submissions
-- Stores public form submissions scoped per site.
-- Field data is stored as JSONB — no schema changes needed when forms change.

CREATE TABLE form_submissions (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id      UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    form_name    TEXT NOT NULL,
    data         JSONB NOT NULL DEFAULT '{}',
    ip_address   TEXT,
    read_at      TIMESTAMPTZ,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_form_submissions_site_form ON form_submissions(site_id, form_name, submitted_at DESC);
