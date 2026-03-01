-- Add default_site_id to users so each user has an explicit "home" site.
-- ON DELETE SET NULL means deleting a site auto-clears the pointer — no orphans.

ALTER TABLE users
    ADD COLUMN default_site_id UUID REFERENCES sites(id) ON DELETE SET NULL;

-- Backfill: for each user, set their default_site_id to the earliest site
-- they own (owner_user_id). Covers existing super_admin and site_admin users.
UPDATE users u
SET default_site_id = (
    SELECT s.id
    FROM sites s
    WHERE s.owner_user_id = u.id
    ORDER BY s.created_at ASC
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM sites s WHERE s.owner_user_id = u.id
);
