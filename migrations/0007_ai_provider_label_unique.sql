-- Two near-simultaneous "Add Provider" submissions (e.g. a slow provider
-- verification call making a double-click look like nothing happened)
-- could both pass the application-level `label_exists_for_site` check
-- before either INSERT committed, creating duplicate rows for the same
-- site+label. De-duplicate any such rows first (keep the oldest — the one
-- most likely to have actually finished verifying) before enforcing this
-- at the database level, where a race can no longer slip through.
DELETE FROM ai_providers a
USING ai_providers b
WHERE a.site_id = b.site_id
  AND a.label = b.label
  AND a.created_at > b.created_at;

ALTER TABLE ai_providers
    ADD CONSTRAINT ai_providers_site_id_label_unique UNIQUE (site_id, label);
