-- Per-site user roles.
-- Global users (users table) get roles on each site independently.
CREATE TABLE site_users (
    site_id    UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       TEXT NOT NULL DEFAULT 'subscriber'
                   CHECK (role IN ('admin', 'editor', 'author', 'subscriber')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (site_id, user_id)
);

CREATE INDEX idx_site_users_user_id ON site_users(user_id);
