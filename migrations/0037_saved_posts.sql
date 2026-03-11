-- Saved posts: allows subscribers to bookmark posts for later reading.
CREATE TABLE saved_posts (
    user_id  UUID NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    post_id  UUID NOT NULL REFERENCES posts(id)  ON DELETE CASCADE,
    site_id  UUID          REFERENCES sites(id)  ON DELETE CASCADE,
    saved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, post_id)
);

CREATE INDEX saved_posts_user_site_idx ON saved_posts (user_id, site_id);
