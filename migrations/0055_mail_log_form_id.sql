-- Ties a mail_log row back to the form that triggered it (notify-admin or
-- confirm-submitter sends), so a form's analytics page can show its own
-- email history instead of every send for the whole site. NULL for sends
-- with no form context (e.g. password reset emails).
ALTER TABLE mail_log ADD COLUMN form_id UUID REFERENCES forms(id) ON DELETE SET NULL;

CREATE INDEX idx_mail_log_form_created ON mail_log (form_id, created_at DESC) WHERE form_id IS NOT NULL;
