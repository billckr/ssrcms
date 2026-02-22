-- Migration: 0007_create_sessions
-- Server-side session store used by tower-sessions-sqlx-store

CREATE TABLE tower_sessions (
    id          TEXT PRIMARY KEY,
    data        BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_sessions_expiry ON tower_sessions(expiry_date);
