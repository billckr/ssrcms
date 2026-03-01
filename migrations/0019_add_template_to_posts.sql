-- Migration: 0019_add_template_to_posts
-- Adds an optional template override column to posts.
-- When NULL the renderer falls back to the default (page.html for pages, single.html for posts).
-- Theme authors can reference any template file within their theme's templates/ directory,
-- e.g. "forms/contact" resolves to templates/forms/contact.html.

ALTER TABLE posts ADD COLUMN template TEXT;
