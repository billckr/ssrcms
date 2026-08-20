-- Marks that a user has dismissed the dashboard's Welcome panel. NULL = not
-- dismissed yet, shown on their next dashboard load. Timestamp (not a bare
-- bool) so a future re-introduction (e.g. after a major version bump wants
-- to show it again) can compare against a cutoff instead of needing a
-- second column — see personal_data_erased_at for the same pattern.
ALTER TABLE users ADD COLUMN welcome_panel_dismissed_at TIMESTAMPTZ;
