-- Tighten the comment body limit from 2000 to 400 characters.
-- Truncate any existing comments that exceed the new limit before
-- adding the constraint so the migration doesn't fail on existing data.
UPDATE comments SET body = left(body, 400) WHERE char_length(body) > 400;

ALTER TABLE comments DROP CONSTRAINT IF EXISTS comments_body_check;
ALTER TABLE comments ADD CONSTRAINT comments_body_check
    CHECK (char_length(body) BETWEEN 1 AND 400);
