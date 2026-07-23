-- Stores each user's dashboard widget column/order preference, e.g.
-- {"left": ["one"], "middle": ["two"], "right": ["three"]}. NULL means
-- the default layout.
ALTER TABLE users
  ADD COLUMN dashboard_widget_layout JSONB;
