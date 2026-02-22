-- Migration: 0003_create_posts
-- Posts and pages share this table, distinguished by post_type

CREATE TABLE posts (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title            TEXT NOT NULL,
    slug             TEXT NOT NULL UNIQUE,
    content          TEXT NOT NULL DEFAULT '',
    content_format   TEXT NOT NULL DEFAULT 'html'
                         CHECK (content_format IN ('html', 'markdown')),
    excerpt          TEXT,                    -- null = auto-generate from content
    status           TEXT NOT NULL DEFAULT 'draft'
                         CHECK (status IN ('draft', 'published', 'scheduled', 'trashed')),
    post_type        TEXT NOT NULL DEFAULT 'post'
                         CHECK (post_type IN ('post', 'page')),
    author_id        UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    featured_image_id UUID REFERENCES media(id) ON DELETE SET NULL,
    published_at     TIMESTAMPTZ,
    scheduled_at     TIMESTAMPTZ,             -- future publish time when status = 'scheduled'
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_posts_slug ON posts(slug);
CREATE INDEX idx_posts_status ON posts(status);
CREATE INDEX idx_posts_post_type ON posts(post_type);
CREATE INDEX idx_posts_author_id ON posts(author_id);
CREATE INDEX idx_posts_published_at ON posts(published_at DESC);
CREATE INDEX idx_posts_status_type_published ON posts(status, post_type, published_at DESC);
