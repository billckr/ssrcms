-- Documentation table for auto-generated feature docs (populated by the document-changes skill).
CREATE TABLE IF NOT EXISTS documentation (
    id           SERIAL PRIMARY KEY,
    slug         VARCHAR NOT NULL UNIQUE,
    title        VARCHAR NOT NULL,
    content      TEXT    NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by   VARCHAR
);
