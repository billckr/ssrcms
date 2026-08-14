-- Usernames no longer need to be globally unique across the whole install.
-- This app is multi-tenant: independent site owners each manage their own
-- users, and it shouldn't matter to one that another site somewhere else
-- already has a "bill". Uniqueness is now enforced at the application layer,
-- scoped to users who actually share a site (see user::username_available
-- and user::get_by_username_in_site in core/src/models/user.rs) — email
-- remains the real login identity and stays globally unique.
ALTER TABLE users DROP CONSTRAINT users_username_key;
