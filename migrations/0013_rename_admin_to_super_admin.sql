-- Migration: 0013_rename_admin_to_super_admin
-- Renames the top-level `admin` role to `super_admin` to unambiguously separate
-- the agency super-admin tier from site-scoped roles stored in site_users.
-- Also retroactively marks all existing admin accounts as is_protected = TRUE.

-- 1. Mark all existing admin accounts as protected before the rename.
UPDATE users SET is_protected = TRUE WHERE role = 'admin';

-- 2. Drop the old CHECK constraint (created in migration 0001).
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;

-- 3. Rename existing role values.
UPDATE users SET role = 'super_admin' WHERE role = 'admin';

-- 4. Add the new CHECK constraint — 'admin' removed, 'super_admin' added.
--    Note: site_users.role CHECK remains unchanged ('admin' is still valid there).
ALTER TABLE users
  ADD CONSTRAINT users_role_check
  CHECK (role IN ('super_admin', 'editor', 'author', 'subscriber'));
