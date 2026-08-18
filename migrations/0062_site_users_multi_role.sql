-- Migration 0062: Allow a user to hold multiple roles on the same site.
--
-- Previously PRIMARY KEY (site_id, user_id) forced exactly one role per
-- (site, user) pair — site_user::add() upserted over any existing role.
-- Real orgs need e.g. one person who is both 'editor' and 'author' on a
-- site with different capabilities depending which hat they're wearing for
-- a given task (they pick one at login via a role-selection page, then the
-- session behaves exactly as if they only held that one role). This
-- migration widens the uniqueness constraint to (site_id, user_id, role)
-- so a user can hold several rows, one per role, on the same site.
--
-- The CHECK constraint on `role` is intentionally left untouched: it must
-- keep allowing only 'admin' | 'editor' | 'author' | 'subscriber'.
-- 'super_admin' and 'site_admin' are users.role-only, global-scope values
-- and have never been (and must never become) valid site_users.role
-- values. This is enforced independently at the Rust type level too — the
-- site-scoped SiteRole enum has no variant for either global tier, so it
-- is a compile error, not just a runtime constraint violation, to attempt
-- storing or session-pinning one as a site role.

ALTER TABLE site_users DROP CONSTRAINT site_users_pkey;

-- Surrogate key: needed because invited_by-style attribution and future
-- per-row actions (e.g. revoking a single role) benefit from a stable
-- single-column id — a 3-column composite PK is awkward to reference.
ALTER TABLE site_users ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE site_users ADD PRIMARY KEY (id);

ALTER TABLE site_users
    ADD CONSTRAINT site_users_site_user_role_unique UNIQUE (site_id, user_id, role);

-- idx_site_users_user_id already exists and remains useful for
-- "all roles/sites for this user" lookups.
