-- Builder projects: a named collection of pages and masters for a site.
-- One project per site can be active (the one the live site serves).
CREATE TABLE builder_projects (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name        VARCHAR(255) NOT NULL,
    description TEXT,
    is_active   BOOLEAN NOT NULL DEFAULT FALSE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX builder_projects_site_id_idx ON builder_projects(site_id);
-- Only one active project per site
CREATE UNIQUE INDEX builder_projects_active_idx
    ON builder_projects(site_id) WHERE is_active = TRUE;

-- Link existing (and future) page compositions to a project
ALTER TABLE page_compositions
    ADD COLUMN project_id UUID REFERENCES builder_projects(id) ON DELETE CASCADE;

CREATE INDEX page_compositions_project_id_idx ON page_compositions(project_id);
