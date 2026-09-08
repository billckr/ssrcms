-- Verified self-service "join this site with my existing account" requests,
-- reached from /subscribe when the submitted email already belongs to an
-- existing subscriber account not yet a member of the current site. Same
-- hashed, single-use, 60-minute token posture as password_resets /
-- email_changes.
CREATE TABLE site_join_requests (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    site_id    UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX site_join_requests_token_hash_idx ON site_join_requests (token_hash);
CREATE INDEX site_join_requests_user_id_idx ON site_join_requests (user_id);
