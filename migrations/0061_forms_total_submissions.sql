-- Lifetime submission counter per form, kept separate from the live
-- COUNT(*) shown on the Submissions tab (which reflects what's currently
-- stored and drives its pagination/export). This one only ever increments,
-- so deleting old responses doesn't erase how much interest a form
-- actually got — an analytics-purpose number, not a data-management one.
ALTER TABLE forms ADD COLUMN total_submissions BIGINT NOT NULL DEFAULT 0;

-- Backfill from submissions already linked via the form_id FK
-- (migration 0060).
UPDATE forms f
SET total_submissions = sub.cnt
FROM (
    SELECT form_id, COUNT(*) AS cnt
    FROM form_submissions
    WHERE form_id IS NOT NULL
    GROUP BY form_id
) sub
WHERE f.id = sub.form_id;
