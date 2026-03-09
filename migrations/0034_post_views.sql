-- Track unique post views per IP per day.
-- Using ip_hash (anonymized IP) + post_id + viewed_date as the unique key
-- ensures one view per visitor per post per day without storing raw IPs.
CREATE TABLE IF NOT EXISTS post_views (
    post_id     UUID    NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    ip_hash     TEXT    NOT NULL,
    viewed_date DATE    NOT NULL DEFAULT CURRENT_DATE,
    PRIMARY KEY (post_id, ip_hash, viewed_date)
);

CREATE INDEX IF NOT EXISTS idx_post_views_post_id ON post_views (post_id);
