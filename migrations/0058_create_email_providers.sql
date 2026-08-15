-- A site can configure multiple named third-party email accounts (Mailgun,
-- SMTP, SendGrid, Postmark) instead of only the single install-wide Mailgun
-- account. Each form then independently picks which one to send through
-- (forms.email_provider_id, added in 0059) — there's no single "default"
-- per site, since different forms on the same site may want different
-- accounts (e.g. sales vs support).
--
-- Credentials vary by provider_type, so rather than a wide sparse column
-- set they're stored as one opaque JSON blob, encrypted at rest the same
-- way the old per-site Mailgun key was (crypto::encrypt/decrypt, keyed by
-- SECRET_KEY) — see core/src/mail.rs.
CREATE TABLE email_providers (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id          UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    provider_type    TEXT NOT NULL,
    label            TEXT NOT NULL,
    config_encrypted TEXT NOT NULL,
    verified         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX email_providers_site_id_idx ON email_providers (site_id);
