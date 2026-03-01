-- Migration 0017: Introduce explicit 'site_admin' value for users.role
--
-- Previously, users who own/administer sites were stored with role='editor'
-- as a placeholder (since 'admin' is a site_users concept, not a users concept).
-- This made metrics and queries ambiguous — real editors and site owners looked
-- identical in the users table.
--
-- 'site_admin' now means: "this user owns at least one site; their per-site
-- permissions are governed by site_users.role". It is not a privilege escalation
-- — they still cannot access other sites without a site_users row.

-- 1. Expand the check constraint to allow the new value.
ALTER TABLE users DROP CONSTRAINT users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role = ANY (ARRAY[
        'super_admin'::text,
        'site_admin'::text,
        'editor'::text,
        'author'::text,
        'subscriber'::text
    ]));

-- 2. Backfill: promote any user with role='editor' who also has at least one
--    site_users row with role='admin'. Pure editors (no admin site_users row)
--    are left unchanged.
UPDATE users
SET role = 'site_admin'
WHERE role = 'editor'
  AND id IN (
      SELECT DISTINCT user_id
      FROM site_users
      WHERE role = 'admin'
  );
