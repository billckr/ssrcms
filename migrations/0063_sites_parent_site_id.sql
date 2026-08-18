-- Tracks site hierarchy: which site was a site_admin logged into when they
-- created this one. NULL means top-level — either the system default site,
-- or any site created while a super_admin was logged in (super_admin acts
-- on behalf of the agency, not on behalf of another client site).
--
-- Used to restrict two things to top-level sites only:
--   - System Settings (branding) capability (AdminCaps::can_manage_site_settings)
--   - which site's own branding a site falls back to when it has none of its
--     own (a child site inherits its immediate parent's brand only, never the
--     agency-wide global default) — see page_ctx() in handlers/admin/mod.rs.

ALTER TABLE sites
    ADD COLUMN parent_site_id UUID REFERENCES sites(id) ON DELETE SET NULL;

COMMENT ON COLUMN sites.parent_site_id IS
    'Site the creator was logged into when this site was created. NULL for top-level sites (created by super_admin, or the system default site). Non-NULL sites cannot manage their own branding and inherit it from this parent instead.';
