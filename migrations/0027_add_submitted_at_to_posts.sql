-- Track when an author first submits a post for editor review.
-- Set automatically whenever a post transitions to 'pending' status.
ALTER TABLE posts ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ;
