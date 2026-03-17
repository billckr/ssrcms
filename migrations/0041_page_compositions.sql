CREATE TABLE page_compositions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name        VARCHAR(255) NOT NULL,
    slug        VARCHAR(255) NOT NULL,
    layout      VARCHAR(100) NOT NULL,
    composition JSONB NOT NULL DEFAULT '{}',
    is_homepage BOOLEAN NOT NULL DEFAULT FALSE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX page_compositions_site_id_idx ON page_compositions(site_id);
-- Only one active homepage per site
CREATE UNIQUE INDEX page_compositions_homepage_idx
    ON page_compositions(site_id) WHERE is_homepage = TRUE;
