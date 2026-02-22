-- Migration: 0002_create_media
-- Media library: tracks all uploaded files

CREATE TABLE media (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename      TEXT NOT NULL,
    mime_type     TEXT NOT NULL,
    path          TEXT NOT NULL,           -- relative path under uploads/
    alt_text      TEXT NOT NULL DEFAULT '',
    width         INTEGER,                  -- null for non-image files
    height        INTEGER,                  -- null for non-image files
    file_size     BIGINT NOT NULL DEFAULT 0,
    uploaded_by   UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_media_uploaded_by ON media(uploaded_by);
CREATE INDEX idx_media_mime_type ON media(mime_type);

-- Add the FK from users.avatar_media_id now that media exists
ALTER TABLE users ADD CONSTRAINT fk_users_avatar_media
    FOREIGN KEY (avatar_media_id) REFERENCES media(id) ON DELETE SET NULL;
