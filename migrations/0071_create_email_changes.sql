-- Pending self-service email-change requests for the /account email-change
-- flow. Tokens are single-use, short-lived, and stored hashed (never the raw
-- token) — same defensive posture as password_resets. The candidate address
-- is stored in plaintext (it's not yet the user's identity email and carries
-- no more sensitivity than the form that submitted it) and is only ever
-- committed to users.email inside the same transaction that consumes the
-- token.
CREATE TABLE email_changes (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    new_email  TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX email_changes_token_hash_idx ON email_changes (token_hash);
CREATE INDEX email_changes_user_id_idx ON email_changes (user_id);
