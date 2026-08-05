-- Password recovery tokens for the public /recover flow. Tokens are
-- single-use, short-lived, and stored hashed (never the raw token) — same
-- defensive posture as password_hash: a DB read alone can't be used to
-- reset an account.
CREATE TABLE password_resets (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX password_resets_token_hash_idx ON password_resets (token_hash);
CREATE INDEX password_resets_user_id_idx ON password_resets (user_id);
