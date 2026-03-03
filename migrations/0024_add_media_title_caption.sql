-- Migration: 0024_add_media_title_caption
-- Add title and caption fields to media for improved image SEO

ALTER TABLE media
    ADD COLUMN title   TEXT NOT NULL DEFAULT '',
    ADD COLUMN caption TEXT NOT NULL DEFAULT '';
