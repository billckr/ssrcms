-- Explicit ownership chain:
--   sites.owner_user_id  — immutable "who created this site" (NULL = super_admin / CLI install)
--   site_users.invited_by — "which admin added this user to this site" (NULL = legacy / CLI seed)

ALTER TABLE sites
    ADD COLUMN owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Backfill: for each site that already has exactly one admin in site_users, set them as owner.
UPDATE sites s
SET owner_user_id = su.user_id
FROM site_users su
WHERE su.site_id = s.id
  AND su.role = 'admin'
  AND s.owner_user_id IS NULL;

ALTER TABLE site_users
    ADD COLUMN invited_by UUID REFERENCES users(id) ON DELETE SET NULL;

COMMENT ON COLUMN sites.owner_user_id IS
    'Immutable creator of this site. NULL = installed by CLI / super_admin. Never updated after insert.';

COMMENT ON COLUMN site_users.invited_by IS
    'User (admin or super_admin) who added this person to the site. NULL for legacy/CLI-seeded rows.';
