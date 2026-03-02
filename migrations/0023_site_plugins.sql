-- Per-site plugin activation table.
-- Records which plugins have been installed and activated for each site.
-- Plugin names match the `plugin.toml` [plugin] name field.

CREATE TABLE site_plugins (
    site_id      UUID        NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    plugin_name  TEXT        NOT NULL,
    active       BOOLEAN     NOT NULL DEFAULT false,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (site_id, plugin_name)
);
