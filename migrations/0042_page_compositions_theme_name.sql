-- Replace is_homepage flag with theme_name.
-- Compositions are now linked to the theme they belong to; activation
-- happens through the existing theme system, not a separate flag.

ALTER TABLE page_compositions DROP COLUMN IF EXISTS is_homepage;
ALTER TABLE page_compositions ADD COLUMN theme_name VARCHAR(255);

DROP INDEX IF EXISTS page_compositions_homepage_idx;

-- One composition per (site, theme) pair.
CREATE UNIQUE INDEX page_compositions_site_theme_idx
    ON page_compositions(site_id, theme_name)
    WHERE theme_name IS NOT NULL;
