-- Persistent, queryable record of account/site lifecycle events — who did
-- what, to what, and when. Distinct from the tracing log file (rotates in
-- prod via journald, grows unbounded in dev, and isn't queryable from
-- inside the app) — same rationale as mail_log (0054) applied to admin
-- actions instead of outbound email.
--
-- actor_email/actor_role/target_label are denormalized snapshots, not joins,
-- because the whole point is surviving the actor or target being deleted
-- later. For the same reason, target_id and site_id are plain UUIDs with no
-- FK constraint: a "site deleted" event is recorded *after* the site row is
-- gone, so a FK on site_id would reject the very row it's meant to record.
-- actor_user_id does get a FK, since the actor still exists at write time —
-- ON DELETE SET NULL so deleting that user later doesn't erase their history.
CREATE TABLE audit_log (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    actor_email   TEXT NOT NULL,
    actor_role    TEXT NOT NULL,
    action        TEXT NOT NULL,
    target_type   TEXT NOT NULL,
    target_id     UUID,
    target_label  TEXT NOT NULL,
    site_id       UUID,
    details       JSONB,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_log_site_created ON audit_log (site_id, created_at DESC);
CREATE INDEX idx_audit_log_created ON audit_log (created_at DESC);
CREATE INDEX idx_audit_log_actor ON audit_log (actor_user_id);
