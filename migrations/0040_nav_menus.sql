CREATE TABLE nav_menus (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    location TEXT,  -- 'primary', 'footer', NULL = unassigned
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (site_id, name)
    -- location uniqueness enforced in app (NULL allowed multiple times in SQL UNIQUE)
);

CREATE TABLE nav_menu_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    menu_id UUID NOT NULL REFERENCES nav_menus(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES nav_menu_items(id) ON DELETE CASCADE,
    sort_order INT NOT NULL DEFAULT 0,
    label TEXT NOT NULL,
    url TEXT,                  -- custom URL; ignored when page_id is set
    page_id UUID REFERENCES posts(id) ON DELETE SET NULL,
    target TEXT NOT NULL DEFAULT '_self',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX nav_menu_items_menu_id_idx ON nav_menu_items(menu_id);
CREATE INDEX nav_menu_items_parent_id_idx ON nav_menu_items(parent_id);
