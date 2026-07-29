-- Stores per-site chosen values for theme customizer layout options (the
-- `[customizer.options.*]` schema declared in a theme's theme.toml). Only
-- the site's override is stored here; if a row is absent, the schema's
-- own `default` applies. Theme templates never read this table directly —
-- core resolves schema + stored value into a plain `theme_options` context
-- variable before rendering.
CREATE TABLE theme_options (
    site_id     UUID NOT NULL,
    theme_name  TEXT NOT NULL,
    option_key  TEXT NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (site_id, theme_name, option_key)
);
