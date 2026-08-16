-- Ties a form_submissions row back to the form definition that collected it,
-- alongside the existing form_name (slug) linkage which stays authoritative
-- for lookups (slug is immutable after creation and is what post/page
-- embeds reference, so nothing else changes). NULL means either an
-- orphaned submission (the definition was later deleted) or a pre-existing
-- row whose slug didn't match any current form at backfill time.
ALTER TABLE form_submissions ADD COLUMN form_id UUID REFERENCES forms(id) ON DELETE SET NULL;

-- Backfill existing rows where the slug still matches a live form.
UPDATE form_submissions fs
SET form_id = f.id
FROM forms f
WHERE fs.site_id = f.site_id AND fs.form_name = f.slug;

CREATE INDEX idx_form_submissions_form_id ON form_submissions (form_id) WHERE form_id IS NOT NULL;
