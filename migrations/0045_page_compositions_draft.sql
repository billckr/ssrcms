-- Add a separate draft column so in-progress edits don't overwrite live content.
-- composition      = live (what visitors see, only updated on Publish)
-- draft_composition = work in progress (what the editor reads/writes)

ALTER TABLE page_compositions
  ADD COLUMN draft_composition JSONB NOT NULL DEFAULT '{}';

-- Seed draft from existing live content so no work is lost.
UPDATE page_compositions SET draft_composition = composition;
