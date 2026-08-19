-- Marks a subscriber account as GDPR-erased. The users row itself is kept
-- (not deleted) and anonymized in place instead — see
-- core/src/models/user.rs::erase_personal_data for why (posts.author_id
-- and media.uploaded_by are ON DELETE RESTRICT; comments.author_id is ON
-- DELETE CASCADE, so a hard delete would either fail or silently wipe
-- their comment history off other people's posts).
ALTER TABLE users ADD COLUMN personal_data_erased_at TIMESTAMPTZ;
