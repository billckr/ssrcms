-- Persistent record of every outbound Mailgun send attempt, success or
-- failure. Distinct from the tracing log file (which rotates/isn't
-- queryable) — this is what powers an admin "Email Log" screen and gives a
-- durable answer to "did we even try to send that?" months later.
CREATE TABLE mail_log (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id            UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    to_email           TEXT NOT NULL,
    subject            TEXT NOT NULL,
    success            BOOLEAN NOT NULL,
    mailgun_message_id TEXT,
    error              TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mail_log_site_created ON mail_log (site_id, created_at DESC);
