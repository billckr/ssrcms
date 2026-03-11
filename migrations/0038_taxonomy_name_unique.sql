-- Migration 0038: Enforce unique taxonomy names per site per type.
--
-- The existing constraint (site_id, slug, taxonomy) prevents duplicate slugs
-- but allows two categories/tags with the same display name but different slugs
-- (e.g. "Technology" slug=technology and "Technology" slug=technology-2).
-- This adds a matching constraint on name so neither name nor slug can be
-- reused within the same site and taxonomy type.

ALTER TABLE taxonomies
    ADD CONSTRAINT taxonomies_site_name_tax_unique
    UNIQUE (site_id, name, taxonomy);
