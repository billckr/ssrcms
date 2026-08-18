-- Poll Designer: single-question vote polls, sibling feature to the Form
-- Designer (forms/form_submissions). options/settings live in JSONB for the
-- same reason forms.fields/.settings do — no migration needed when a poll's
-- option list changes. poll_votes stays a narrow, fixed-shape table (not
-- JSONB) since it needs a real unique index for vote-dedupe and its shape
-- never varies.
CREATE TABLE polls (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id      UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    slug         TEXT NOT NULL,
    question     TEXT NOT NULL,
    options      JSONB NOT NULL DEFAULT '[]',
    settings     JSONB NOT NULL DEFAULT '{}',
    total_votes  BIGINT NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, slug)
);

CREATE TABLE poll_votes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id     UUID NOT NULL REFERENCES polls(id) ON DELETE CASCADE,
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    option_key  TEXT NOT NULL,
    voter_token TEXT NOT NULL,
    ip_address  TEXT,
    voted_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Enforces vote-dedupe at the DB level, not just the cookie check — even a
-- forged/replayed request can't insert two rows for the same browser token.
CREATE UNIQUE INDEX idx_poll_votes_poll_voter ON poll_votes (poll_id, voter_token);
CREATE INDEX idx_poll_votes_poll_option ON poll_votes (poll_id, option_key);
CREATE INDEX idx_poll_votes_poll_ip ON poll_votes (poll_id, ip_address) WHERE ip_address IS NOT NULL;
