-- Soft delete for users.
-- Instead of hard DELETE (which cascades and destroys posts/pages),
-- set deleted_at. The application filters these out of all lists and
-- login checks, but content is preserved and re-assignable.

ALTER TABLE users
    ADD COLUMN deleted_at TIMESTAMPTZ NULL DEFAULT NULL;

-- Index so live-user queries stay fast (partial index on NULL deleted_at).
CREATE INDEX idx_users_active ON users(id) WHERE deleted_at IS NULL;

COMMENT ON COLUMN users.deleted_at IS
    'Non-NULL = soft-deleted. User cannot log in; their content is preserved.';
