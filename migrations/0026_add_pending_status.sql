-- Add 'pending' (awaiting editor review) to the posts status constraint.
ALTER TABLE posts DROP CONSTRAINT IF EXISTS posts_status_check;
ALTER TABLE posts ADD CONSTRAINT posts_status_check
    CHECK (status IN ('draft', 'pending', 'published', 'scheduled', 'trashed'));
