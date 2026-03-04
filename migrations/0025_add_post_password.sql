-- Migration: 0025_add_post_password
-- Adds optional password protection to posts and pages.
-- post_password stores an Argon2 hash. NULL = no protection.

ALTER TABLE posts ADD COLUMN IF NOT EXISTS post_password TEXT;
