-- Which configured email_providers row (if any) this form's notify/confirm
-- emails should send through. NULL keeps today's behavior: fall back to
-- the install-wide Mailgun account. ON DELETE SET NULL so removing a
-- provider a form was using just reverts that form to the fallback instead
-- of erroring.
ALTER TABLE forms ADD COLUMN email_provider_id UUID REFERENCES email_providers(id) ON DELETE SET NULL;
