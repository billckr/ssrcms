-- Per-user override for the Author role: when true, this specific author can
-- publish their own posts directly instead of only submitting draft/pending
-- for an Editor to review and publish. Default false keeps the existing
-- editor-author collaboration workflow as the default for everyone; only
-- meaningful when site_users.role = 'author'.
ALTER TABLE site_users ADD COLUMN can_self_publish BOOLEAN NOT NULL DEFAULT false;
