-- Soft-delete support for comments.
-- deleted_at is set when a subscriber self-deletes within the 15-minute window.
-- The row is never removed; admins can still see and hard-delete it.
ALTER TABLE comments ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
