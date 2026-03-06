-- Enable or disable comments per post/page. Default off.
ALTER TABLE posts ADD COLUMN IF NOT EXISTS comments_enabled BOOLEAN NOT NULL DEFAULT FALSE;

-- Flat comment storage with optional parent for one level of threading.
CREATE TABLE IF NOT EXISTS comments (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id     UUID        NOT NULL REFERENCES posts(id)    ON DELETE CASCADE,
    site_id     UUID                 REFERENCES sites(id)    ON DELETE CASCADE,
    author_id   UUID        NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
    parent_id   UUID                 REFERENCES comments(id) ON DELETE CASCADE,
    body        TEXT        NOT NULL CHECK (char_length(body) BETWEEN 1 AND 2000),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_comments_post_id   ON comments(post_id);
CREATE INDEX IF NOT EXISTS idx_comments_author_id  ON comments(author_id);
CREATE INDEX IF NOT EXISTS idx_comments_parent_id  ON comments(parent_id);
