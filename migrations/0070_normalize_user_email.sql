-- Authentication treats email addresses case-insensitively. Normalize stored
-- values and enforce that invariant in PostgreSQL as well as application code.
-- If legacy data contains case-only duplicates, deployment intentionally stops
-- here so an operator can resolve the ambiguous identities safely.
UPDATE users SET email = lower(trim(email)) WHERE email <> lower(trim(email));

CREATE UNIQUE INDEX users_email_lower_unique ON users (lower(email));
