-- Form Designer: reusable form definitions, scoped per site.
-- Field/settings shape lives entirely in JSONB so field types can be added
-- later without a migration — mirrors the form_submissions.data approach.
CREATE TABLE forms (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id    UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    slug       TEXT NOT NULL,
    fields     JSONB NOT NULL DEFAULT '[]',
    settings   JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, slug)
);
