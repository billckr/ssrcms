-- Marks users created via the Deploy Test Data seeding feature, so "Clear test
-- data" can optionally remove exactly the users it created, and never anyone else.
ALTER TABLE users
  ADD COLUMN is_seeded BOOLEAN NOT NULL DEFAULT FALSE;
