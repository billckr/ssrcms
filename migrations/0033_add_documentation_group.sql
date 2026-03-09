-- Add group column to documentation table for categorizing docs in the admin viewer.
-- Values: 'system' (Admin Panel, Routing, Database) or 'feature' (Posts, Forms, etc.)
ALTER TABLE documentation ADD COLUMN IF NOT EXISTS grp VARCHAR NOT NULL DEFAULT 'feature';
