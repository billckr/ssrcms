//! Admin navigation menus list and editor pages.

use uuid::Uuid;

/// A menu row for the list view.
pub struct MenuRow {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub item_count: i64,
}

/// A menu item row for the edit view.
pub struct MenuItemRow {
    pub id: String,
    pub menu_id: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
    pub label: String,
    pub url: Option<String>,
    pub page_id: Option<String>,
    pub page_title: Option<String>,   // resolved title for display
    pub target: String,
}

pub struct MenuEdit {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

const LOCATION_OPTIONS: &[(&str, &str)] = &[
    ("", "Name only (custom get_menu)"),
    ("primary", "Primary Navigation"),
    ("footer", "Footer Links"),
];

fn location_label(location: Option<&str>) -> &'static str {
    match location {
        Some("primary") => "Primary Navigation",
        Some("footer")  => "Footer Links",
        _               => "Name only (custom get_menu)",
    }
}

pub fn render_list(menus: &[MenuRow], ctx: &crate::PageContext) -> String {
    let location_opts = LOCATION_OPTIONS.iter().map(|(val, label)| {
        format!(
            r#"<option value="{val}">{label}</option>"#,
            val = crate::html_escape(val),
            label = label,
        )
    }).collect::<Vec<_>>().join("");

    let rows = if menus.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">No menus yet. Create one below.</td></tr>"#.to_string()
    } else {
        menus.iter().map(|m| {
            format!(
                r#"<tr>
  <td><a href="/admin/menus/{id}">{name}</a></td>
  <td>{location}</td>
  <td>{items}</td>
  <td class="actions">
    <a href="/admin/menus/{id}" class="icon-btn" title="Edit">
      <img src="/admin/static/icons/edit.svg" alt="Edit">
    </a>
    <form method="POST" action="/admin/menus/{id}/delete" style="display:inline"
          onsubmit="return confirm('Delete this menu?')">
      <button class="icon-btn icon-danger" title="Delete" type="submit">
        <img src="/admin/static/icons/delete.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                id       = crate::html_escape(&m.id),
                name     = crate::html_escape(&m.name),
                location = location_label(m.location.as_deref()),
                items    = m.item_count,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<form method="POST" action="/admin/menus" style="display:flex;gap:.75rem;align-items:flex-end;margin-bottom:1.5rem;flex-wrap:wrap">
  <div class="form-group" style="margin:0;flex:1;min-width:160px">
    <label for="new-menu-name">Menu Name</label>
    <input id="new-menu-name" type="text" name="name" required placeholder="e.g. Main Menu" maxlength="25" style="width:200px">
  </div>
  <div class="form-group" style="margin:0">
    <label for="new-menu-location">Location</label>
    <select id="new-menu-location" name="location">{location_opts}</select>
  </div>
  <button type="submit" class="btn btn-primary" style="align-self:flex-end">Create Menu</button>
</form>
<table class="data-table">
  <thead><tr><th>Name</th><th>Location</th><th>Items</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        location_opts = location_opts,
        rows          = rows,
    );

    crate::admin_page("Menus", "/admin/menus", None, &content, ctx)
}

pub fn render_edit(
    menu: &MenuEdit,
    items: &[MenuItemRow],
    pages: &[(Uuid, String)],
    ctx: &crate::PageContext,
    flash: Option<&str>,
) -> String {
    let location_opts = LOCATION_OPTIONS.iter().map(|(val, label)| {
        let selected = if menu.location.as_deref().unwrap_or("") == *val { " selected" } else { "" };
        format!(
            r#"<option value="{val}"{selected}>{label}</option>"#,
            val      = crate::html_escape(val),
            label    = label,
            selected = selected,
        )
    }).collect::<Vec<_>>().join("");

    // Build item cards (recursive, depth-indented)
    fn render_items(
        items: &[MenuItemRow],
        pages: &[(Uuid, String)],
        parent_id: Option<&str>,
        menu_id: &str,
        depth: usize,
    ) -> String {
        items.iter()
            .filter(|i| i.parent_id.as_deref() == parent_id)
            .map(|i| {
                let dest = if let Some(ref pt) = i.page_title {
                    format!("Page: {}", crate::html_escape(pt))
                } else if let Some(ref url) = i.url {
                    crate::html_escape(url)
                } else {
                    "—".to_string()
                };
                let target_badge = if i.target == "_blank" {
                    r#"<span class="badge" style="margin-left:.4rem">new tab</span>"#
                } else { "" };
                let depth_indicator = "— ".repeat(depth);
                let children = render_items(items, pages, Some(&i.id), menu_id, depth + 1);

                let page_opts: String = std::iter::once(("".to_string(), "— Custom URL —".to_string()))
                    .chain(pages.iter().map(|(id, title)| (id.to_string(), title.clone())))
                    .map(|(pid, ptitle)| {
                        let sel = if i.page_id.as_deref() == Some(&pid) { " selected" } else { "" };
                        format!(r#"<option value="{pid}"{sel}>{ptitle}</option>"#,
                            pid    = crate::html_escape(&pid),
                            ptitle = crate::html_escape(&ptitle),
                            sel    = sel,
                        )
                    }).collect();

                let parent_opts: String = std::iter::once(("".to_string(), "— No parent —".to_string()))
                    .chain(items.iter().filter(|p| p.id != i.id).map(|p| (p.id.clone(), p.label.clone())))
                    .map(|(pid, plabel)| {
                        let sel = if i.parent_id.as_deref() == Some(&pid) { " selected" } else { "" };
                        format!(r#"<option value="{pid}"{sel}>{plabel}</option>"#,
                            pid    = crate::html_escape(&pid),
                            plabel = crate::html_escape(&plabel),
                            sel    = sel,
                        )
                    }).collect();

                let target_opts: String = [("_self", "Same tab"), ("_blank", "New tab")]
                    .iter()
                    .map(|(val, label)| {
                        let sel = if i.target == *val { " selected" } else { "" };
                        format!(r#"<option value="{val}"{sel}>{label}</option>"#, val=val, label=label, sel=sel)
                    }).collect();

                format!(
                    r#"<div class="menu-item-card">
  <div class="menu-item-card__row">
    <div class="menu-item-card__info">
      <span class="menu-item-card__label">{depth_indicator}{label}</span>{target_badge}
      <span class="menu-item-card__dest">{dest}</span>
    </div>
    <div class="menu-item-card__actions">
      <label class="btn btn-primary btn-sm" for="edit-toggle-{item_id}" style="cursor:pointer;padding:.25rem .6rem;font-size:.8rem">Edit</label>
      <form method="POST" action="/admin/menus/{menu_id}/items/{item_id}/delete"
            onsubmit="return confirm('Delete this item?')" style="display:inline">
        <button class="icon-btn icon-danger" title="Delete" type="submit">
          <img src="/admin/static/icons/delete.svg" alt="Delete">
        </button>
      </form>
    </div>
  </div>
  <input type="checkbox" id="edit-toggle-{item_id}" class="menu-item-toggle" style="display:none">
  <div class="menu-item-card__form">
    <form method="POST" action="/admin/menus/{menu_id}/items/{item_id}/edit">
      <div class="form-row">
        <div class="form-group">
          <label>Label</label>
          <input type="text" name="label" value="{label_val}" required>
        </div>
        <div class="form-group">
          <label>Target</label>
          <select name="target">{target_opts}</select>
        </div>
      </div>
      <div class="form-row">
        <div class="form-group">
          <label>Page <span class="form-hint">overrides URL when selected</span></label>
          <select name="page_id">{page_opts}</select>
        </div>
        <div class="form-group">
          <label>Custom URL <span class="form-hint">used when no page selected</span></label>
          <input type="text" name="url" value="{url_val}" placeholder="/about or https://…">
        </div>
      </div>
      <div class="form-row">
        <div class="form-group">
          <label>Parent item</label>
          <select name="parent_id">{parent_opts}</select>
        </div>
        <div class="form-group">
          <label>Sort order</label>
          <input type="number" name="sort_order" value="{sort_order}" style="width:100px">
        </div>
      </div>
      <div class="form-actions">
        <button type="submit" class="btn btn-primary">Save Changes</button>
        <label for="edit-toggle-{item_id}" class="btn" style="cursor:pointer">Cancel</label>
      </div>
    </form>
  </div>
</div>
{children}"#,
                    depth_indicator = depth_indicator,
                    label           = crate::html_escape(&i.label),
                    target_badge    = target_badge,
                    dest            = dest,
                    menu_id         = crate::html_escape(menu_id),
                    item_id         = crate::html_escape(&i.id),
                    label_val       = crate::html_escape(&i.label),
                    url_val         = crate::html_escape(i.url.as_deref().unwrap_or("")),
                    sort_order      = i.sort_order,
                    page_opts       = page_opts,
                    parent_opts     = parent_opts,
                    target_opts     = target_opts,
                    children        = children,
                )
            })
            .collect::<Vec<_>>().join("\n")
    }

    let items_html = render_items(items, pages, None, &menu.id, 0);
    let items_section = if items.is_empty() {
        r#"<p style="color:var(--muted);padding:.75rem 0">No items yet. Add one below.</p>"#.to_string()
    } else {
        format!(r#"<div class="menu-item-list">{items_html}</div>"#, items_html = items_html)
    };

    // Add item form
    let page_opts_add: String = std::iter::once(("".to_string(), "— Custom URL —".to_string()))
        .chain(pages.iter().map(|(id, title)| (id.to_string(), title.clone())))
        .map(|(pid, ptitle)| {
            format!(r#"<option value="{pid}">{ptitle}</option>"#,
                pid    = crate::html_escape(&pid),
                ptitle = crate::html_escape(&ptitle),
            )
        }).collect();

    let parent_opts_add: String = std::iter::once(("".to_string(), "— No parent (top level) —".to_string()))
        .chain(items.iter().map(|i| (i.id.clone(), i.label.clone())))
        .map(|(pid, plabel)| {
            format!(r#"<option value="{pid}">{plabel}</option>"#,
                pid    = crate::html_escape(&pid),
                plabel = crate::html_escape(&plabel),
            )
        }).collect();

    let content = format!(
        r#"<style>
.menu-settings-card {{
  background: var(--card-bg, #fff);
  border: 1px solid var(--border);
  border-radius: var(--radius, 6px);
  padding: 1.25rem 1.5rem;
  margin-bottom: 1.75rem;
}}
.form-row {{
  display: grid;
  grid-template-columns: repeat(2, minmax(120px, 260px));
  gap: .75rem;
  margin-bottom: .75rem;
}}
.form-row .form-group {{ margin: 0; }}
.form-hint {{
  font-size: 11px;
  color: var(--muted);
  font-weight: 400;
  margin-left: .25rem;
}}
.form-actions {{
  display: flex;
  gap: .5rem;
  margin-top: .75rem;
  padding-top: .75rem;
  border-top: 1px solid var(--border);
}}
.btn-sm {{ font-size: 12px; padding: .2rem .6rem; }}
.menu-item-list {{
  border: 1px solid var(--border);
  border-radius: var(--radius, 6px);
  overflow: hidden;
  margin-bottom: 1.5rem;
}}
.menu-item-card {{
  border-bottom: 1px solid var(--border);
}}
.menu-item-card:last-child {{ border-bottom: none; }}
.menu-item-card__row {{
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: .65rem 1rem;
  background: var(--card-bg, #fff);
}}
.menu-item-card__info {{
  display: flex;
  flex-direction: column;
  gap: .15rem;
}}
.menu-item-card__label {{
  font-weight: 600;
  font-size: .9rem;
}}
.menu-item-card__dest {{
  font-size: .78rem;
  color: var(--muted);
}}
.menu-item-card__actions {{
  display: flex;
  align-items: center;
  gap: .5rem;
  flex-shrink: 0;
}}
.menu-item-toggle:checked ~ .menu-item-card__form {{
  display: block;
}}
.menu-item-card__form {{
  display: none;
  padding: 1rem 1.25rem 1.25rem;
  background: var(--sidebar-bg, #f8f8f8);
  border-top: 1px solid var(--border);
}}
.add-item-section {{
  background: var(--card-bg, #fff);
  border: 1px solid var(--border);
  border-radius: var(--radius, 6px);
  padding: 1.25rem 1.5rem;
  margin-bottom: 1.5rem;
}}
.add-item-section h4 {{
  margin: 0 0 1rem;
  font-size: .95rem;
}}
@media (max-width: 600px) {{
  .form-row {{ grid-template-columns: 1fr; }}
}}
</style>

<div class="menu-settings-card">
  <h3 style="margin:0 0 1rem;font-size:1rem">Menu Settings</h3>
  <form method="POST" action="/admin/menus/{menu_id}">
    <div class="form-row">
      <div class="form-group" style="margin:0">
        <label for="menu-name">Menu Name</label>
        <input id="menu-name" type="text" name="name" value="{menu_name}" required maxlength="25" style="width:200px">
      </div>
      <div class="form-group" style="margin:0">
        <label for="menu-location">Assign to Location</label>
        <select id="menu-location" name="location">{location_opts}</select>
      </div>
    </div>
    <div class="form-actions">
      <button type="submit" class="btn btn-primary">Save Settings</button>
    </div>
  </form>
</div>

<h3 style="margin-bottom:.75rem">Menu Items</h3>
{items_section}

<div class="add-item-section">
  <h4>+ Add Item</h4>
  <form method="POST" action="/admin/menus/{menu_id}/items/new">
    <div class="form-row">
      <div class="form-group" style="margin:0">
        <label>Label</label>
        <input type="text" name="label" required placeholder="e.g. Home">
      </div>
      <div class="form-group" style="margin:0">
        <label>Target</label>
        <select name="target">
          <option value="_self">Same tab</option>
          <option value="_blank">New tab</option>
        </select>
      </div>
    </div>
    <div class="form-row">
      <div class="form-group" style="margin:0">
        <label>Page <span class="form-hint">overrides URL when selected</span></label>
        <select name="page_id">{page_opts_add}</select>
      </div>
      <div class="form-group" style="margin:0">
        <label>Custom URL <span class="form-hint">used when no page selected</span></label>
        <input type="text" name="url" placeholder="/about or https://…">
      </div>
    </div>
    <div class="form-row">
      <div class="form-group" style="margin:0">
        <label>Parent item</label>
        <select name="parent_id">{parent_opts_add}</select>
      </div>
      <div class="form-group" style="margin:0">
        <label>Sort order</label>
        <input type="number" name="sort_order" value="0" style="width:100px">
      </div>
    </div>
    <div class="form-actions">
      <button type="submit" class="btn btn-primary">Add Item</button>
    </div>
  </form>
</div>

<div style="display:flex;gap:.75rem;align-items:center;padding-top:.5rem">
  <a href="/admin/menus" class="btn">← Back to Menus</a>
  <form method="POST" action="/admin/menus/{menu_id}/delete"
        onsubmit="return confirm('Delete this menu and all its items?')" style="margin:0">
    <button type="submit" class="btn btn-danger">Delete Menu</button>
  </form>
</div>"#,
        menu_id         = crate::html_escape(&menu.id),
        menu_name       = crate::html_escape(&menu.name),
        location_opts   = location_opts,
        items_section   = items_section,
        page_opts_add   = page_opts_add,
        parent_opts_add = parent_opts_add,
    );

    crate::admin_page("Edit Menu", "/admin/menus", flash, &content, ctx)
}
