-- Migration: 0012_add_is_protected_to_users
-- Adds is_protected flag to prevent deletion of the install-time admin account.

ALTER TABLE users ADD COLUMN is_protected BOOLEAN NOT NULL DEFAULT FALSE;
