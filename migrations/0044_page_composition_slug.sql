ALTER TABLE page_compositions
    ADD COLUMN slug      VARCHAR(100),
    ADD COLUMN page_type VARCHAR(20) NOT NULL DEFAULT 'page';

-- homepage slug is always '/', regular pages have unique slugs per project
CREATE UNIQUE INDEX page_compositions_slug_project_idx
    ON page_compositions(project_id, slug)
    WHERE slug IS NOT NULL;
