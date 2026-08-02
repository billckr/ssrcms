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

pub fn render_list(menus: &[MenuRow], ctx: &crate::PageContext, flash: Option<&str>) -> String {
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
        r#"<div class="two-col">
  <div>
    <table class="data-table">
      <thead><tr><th>Name</th><th>Location</th><th>Items</th><th>Actions</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>
  </div>
  <div>
    <div class="card-boxed">
      <h2 class="card-boxed-header">Add Menu</h2>
      <div class="card-boxed-body">
        <form method="POST" action="/admin/menus">
          <div class="form-group">
            <label for="new-menu-name">Menu Name</label>
            <input id="new-menu-name" type="text" name="name" required placeholder="e.g. Main Menu" maxlength="25">
            <span class="form-hint char-count" id="new-menu-name-count">0/25</span>
          </div>
          <div class="form-group">
            <label for="new-menu-location">Location</label>
            <select id="new-menu-location" name="location">{location_opts}</select>
          </div>
          <button type="submit" class="btn btn-primary" id="create-menu-submit" disabled>Create Menu</button>
        </form>
      </div>
    </div>
  </div>
</div>
<script>
(function() {{
  var nameInput = document.getElementById('new-menu-name');
  var submitBtn = document.getElementById('create-menu-submit');
  var count = document.getElementById('new-menu-name-count');
  function update() {{
    submitBtn.disabled = nameInput.value.trim().length === 0;
    if (count) count.textContent = nameInput.value.length + '/25';
  }}
  nameInput.addEventListener('input', update);
  update();
}})();
</script>"#,
        location_opts = location_opts,
        rows          = rows,
    );

    crate::admin_page("Menus", "/admin/menus", flash, &content, ctx)
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

    // Build item cards (recursive; nested in .menu-item-children containers so
    // drag-and-drop reordering can be scoped to one sibling group at a time).
    fn render_items(
        items: &[MenuItemRow],
        pages: &[(Uuid, String)],
        parent_id: Option<&str>,
        menu_id: &str,
    ) -> String {
        items.iter()
            .filter(|i| i.parent_id.as_deref() == parent_id)
            .map(|i| {
                let has_children = items.iter().any(|c| c.parent_id.as_deref() == Some(i.id.as_str()));
                let dest = if let Some(ref pt) = i.page_title {
                    format!("Page: {}", crate::html_escape(pt))
                } else if let Some(ref url) = i.url {
                    crate::html_escape(url)
                } else if has_children {
                    "Dropdown parent (no link)".to_string()
                } else {
                    "No link".to_string()
                };
                let target_badge = if i.target == "_blank" {
                    r#"<span class="badge" style="margin-left:.4rem">new tab</span>"#
                } else { "" };
                let children = render_items(items, pages, Some(&i.id), menu_id);
                let children_section = if children.is_empty() {
                    String::new()
                } else {
                    format!(
                        r#"<div class="menu-item-children" data-parent-id="{item_id}">{children}</div>"#,
                        item_id  = crate::html_escape(&i.id),
                        children = children,
                    )
                };

                let page_opts: String = std::iter::once(("".to_string(), "Select Page".to_string()))
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

                let parent_attr = i.parent_id.as_deref().unwrap_or("");

                format!(
                    r#"<div class="menu-item-group" data-item-id="{item_id}" data-parent-id="{parent_attr}">
  <div class="menu-item-card">
  <div class="menu-item-card__row">
    <span class="drag-handle" title="Drag to reorder" draggable="true">
      <img src="/admin/static/icons/move.svg" alt="">
    </span>
    <div class="menu-item-card__info">
      <span class="menu-item-card__label">{label}</span>{target_badge}
      <span class="menu-item-card__dest">{dest}</span>
    </div>
    <div class="menu-item-card__actions">
      <label class="icon-btn" for="edit-toggle-{item_id}" title="Edit" style="cursor:pointer">
        <img src="/admin/static/icons/edit.svg" alt="Edit">
      </label>
      <form method="POST" action="/admin/menus/{menu_id}/items/{item_id}/delete"
            onsubmit="return confirm('Delete the following menu item?\n\n{label_val}')" style="display:inline">
        <button class="icon-btn icon-danger" type="submit" title="Delete">
          <img src="/admin/static/icons/delete.svg" alt="Delete">
        </button>
      </form>
    </div>
  </div>
  <input type="checkbox" id="edit-toggle-{item_id}" class="menu-item-toggle" style="display:none">
  <div class="menu-item-card__form">
    <form method="POST" action="/admin/menus/{menu_id}/items/{item_id}/edit" class="js-menu-item-form">
      <div class="form-row">
        <div class="form-group">
          <label>Label</label>
          <input type="text" name="label" value="{label_val}" required maxlength="100">
          <span class="form-hint char-count item-label-count">{label_len}/100</span>
        </div>
        <div class="form-group">
          <label>Target</label>
          <select name="target">{target_opts}</select>
        </div>
      </div>
      <div class="form-stack">
        <div class="form-group" style="margin:0">
          <label>Page</label>
          <select name="page_id">{page_opts}</select>
        </div>
        <div class="form-group" style="margin:0">
          <label>Custom URL</label>
          <span class="form-hint form-hint-block">optional, for label-only items leave blank</span>
          <input type="text" name="url" value="{url_val}" placeholder="/about or https://…" maxlength="500">
          <span class="field-error field-error-url">Enter a path starting with / or a full http(s):// URL</span>
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
{children_section}
</div>"#,
                    label            = crate::html_escape(&i.label),
                    target_badge     = target_badge,
                    dest             = dest,
                    menu_id          = crate::html_escape(menu_id),
                    item_id          = crate::html_escape(&i.id),
                    parent_attr      = crate::html_escape(parent_attr),
                    label_val        = crate::html_escape(&i.label),
                    label_len        = i.label.chars().count(),
                    url_val          = crate::html_escape(i.url.as_deref().unwrap_or("")),
                    sort_order       = i.sort_order,
                    page_opts        = page_opts,
                    parent_opts      = parent_opts,
                    target_opts      = target_opts,
                    children_section = children_section,
                )
            })
            .collect::<Vec<_>>().join("\n")
    }

    let items_html = render_items(items, pages, None, &menu.id);
    let items_section = if items.is_empty() {
        r#"<p style="color:var(--muted);font-size:.875rem;margin:.25rem 0 1rem">No items yet.</p>"#.to_string()
    } else {
        format!(
            r#"<div class="menu-item-list" data-parent-id="" data-reorder-url="/admin/menus/{menu_id}/items/reorder">{items_html}</div>"#,
            menu_id   = crate::html_escape(&menu.id),
            items_html = items_html,
        )
    };

    // Add item form
    let page_opts_add: String = std::iter::once(("".to_string(), "Select Page".to_string()))
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
.form-row {{
  display: grid;
  grid-template-columns: repeat(2, minmax(120px, 260px));
  gap: .75rem;
  margin-bottom: .75rem;
}}
.form-row .form-group {{ margin: 0; }}
.form-stack {{
  display: flex;
  flex-direction: column;
  gap: .75rem;
  margin-bottom: .75rem;
  max-width: 260px;
}}
.form-hint-block {{
  display: block;
  margin: .1rem 0 .35rem;
}}
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
}}
.btn-sm {{ font-size: 12px; padding: .2rem .6rem; }}
.field-error {{
  display: none;
  color: var(--danger, #c0392b);
  font-size: 11px;
  margin-top: .25rem;
}}
.field-invalid {{ border-color: var(--danger, #c0392b) !important; }}
.menu-item-list {{
  border: 1px solid var(--border);
  border-radius: var(--radius, 6px);
  overflow: hidden;
  margin-bottom: 1.5rem;
}}
.menu-item-card {{
  border-bottom: 1px solid var(--border);
}}
.menu-item-group:last-child > .menu-item-card {{ border-bottom: none; }}
.menu-item-children {{
  padding-left: 1.75rem;
  border-top: 1px solid var(--border);
  background: var(--sidebar-bg, #f8f8f8);
}}
.menu-item-children .menu-item-group:last-child > .menu-item-card {{ border-bottom: none; }}
.menu-item-card__row {{
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: .65rem 1rem;
  background: var(--card-bg, #fff);
  gap: .65rem;
}}
.drag-handle {{
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  cursor: grab;
  opacity: .5;
}}
.drag-handle:hover {{ opacity: .9; }}
.drag-handle img {{ width: 16px; height: 16px; }}
.menu-item-group.dragging > .menu-item-card {{ opacity: .4; }}
.menu-item-card__info {{
  display: flex;
  flex-direction: column;
  gap: .15rem;
  flex: 1;
  min-width: 0;
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
@media (max-width: 600px) {{
  .form-row {{ grid-template-columns: 1fr; }}
}}
</style>

<div class="card-boxed">
  <h2 class="card-boxed-header">Menu Settings</h2>
  <div class="card-boxed-body">
    <div class="form-row">
      <div class="form-group" style="margin:0">
        <label for="menu-name">Menu Name</label>
        <input id="menu-name" type="text" name="name" value="{menu_name}" required maxlength="25" style="width:200px" form="menu-settings-form">
        <span class="form-hint char-count" id="menu-name-count">0/25</span>
      </div>
      <div class="form-group" style="margin:0">
        <label for="menu-location">Assign to Location</label>
        <select id="menu-location" name="location" form="menu-settings-form">{location_opts}</select>
      </div>
    </div>
    <div class="form-actions">
      <form id="menu-settings-form" method="POST" action="/admin/menus/{menu_id}" style="margin:0;display:inline">
        <button type="submit" class="btn btn-primary" id="menu-settings-save" disabled>Save</button>
      </form>
      <form method="POST" action="/admin/menus/{menu_id}/delete"
            onsubmit="return confirm('Delete this menu and all its items?')" style="margin:0;display:inline">
        <button type="submit" class="btn btn-danger">Delete</button>
      </form>
    </div>
  </div>
</div>
<script>
(function() {{
  var nameInput = document.getElementById('menu-name');
  var locationSelect = document.getElementById('menu-location');
  var saveBtn = document.getElementById('menu-settings-save');
  var count = document.getElementById('menu-name-count');
  var initialName = nameInput.value;
  var initialLocation = locationSelect.value;
  function update() {{
    saveBtn.disabled = nameInput.value === initialName && locationSelect.value === initialLocation;
    if (count) count.textContent = nameInput.value.length + '/25';
  }}
  nameInput.addEventListener('input', update);
  locationSelect.addEventListener('change', update);
  update();
}})();
</script>

<div class="card-boxed">
  <h2 class="card-boxed-header">Menu Items</h2>
  <div class="card-boxed-body">
    {items_section}
  </div>
</div>

<div class="card-boxed">
  <h2 class="card-boxed-header">Add Item</h2>
  <div class="card-boxed-body">
    <p class="form-note">To create a dropdown parent (a menu heading that reveals sub-items), give it a Label and leave both Page and Custom URL blank — then add its sub-items with this one set as their Parent item.</p>
    <form method="POST" action="/admin/menus/{menu_id}/items/new" class="js-menu-item-form">
      <div class="form-row">
        <div class="form-group" style="margin:0">
          <label>Label</label>
          <input type="text" name="label" required placeholder="e.g. Home" maxlength="100">
          <span class="form-hint char-count item-label-count">0/100</span>
        </div>
        <div class="form-group" style="margin:0">
          <label>Target</label>
          <select name="target">
            <option value="_self">Same tab</option>
            <option value="_blank">New tab</option>
          </select>
        </div>
      </div>
      <div class="form-stack">
        <div class="form-group" style="margin:0">
          <label>Page</label>
          <select name="page_id">{page_opts_add}</select>
        </div>
        <div class="form-group" style="margin:0">
          <label>Custom URL</label>
          <span class="form-hint form-hint-block">optional, for label-only items leave blank</span>
          <input type="text" name="url" placeholder="/about or https://…" maxlength="500">
          <span class="field-error field-error-url">Enter a path starting with / or a full http(s):// URL</span>
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
        <button type="submit" class="btn btn-primary">Save Item</button>
      </div>
    </form>
  </div>
</div>
<script>
(function() {{
  function isValidUrl(v) {{
    if (!v) return true;
    return /^(\/|https?:\/\/)\S*$/.test(v);
  }}
  function bind(form) {{
    var label = form.querySelector('input[name="label"]');
    var url = form.querySelector('input[name="url"]');
    var submitBtn = form.querySelector('button[type="submit"]');
    var urlError = form.querySelector('.field-error-url');
    var labelCount = form.querySelector('.item-label-count');
    function update() {{
      var labelValid = label.value.trim().length > 0;
      var urlValid = isValidUrl(url.value.trim());
      if (urlError) urlError.style.display = urlValid ? 'none' : 'block';
      if (labelCount) labelCount.textContent = label.value.length + '/100';
      url.classList.toggle('field-invalid', !urlValid);
      submitBtn.disabled = !(labelValid && urlValid);
    }}
    label.addEventListener('input', update);
    url.addEventListener('input', update);
    update();
  }}
  document.querySelectorAll('.js-menu-item-form').forEach(bind);
}})();
</script>
<script>
(function() {{
  function directGroups(container) {{
    return Array.prototype.filter.call(container.children, function(el) {{
      return el.classList.contains('menu-item-group');
    }});
  }}

  function reorderUrlFor(container) {{
    var withUrl = container.closest('[data-reorder-url]');
    return withUrl ? withUrl.dataset.reorderUrl : null;
  }}

  function saveOrder(container) {{
    var url = reorderUrlFor(container);
    if (!url) return;
    var ids = directGroups(container).map(function(el) {{ return el.dataset.itemId; }});
    fetch(url, {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ order: ids }})
    }}).catch(function() {{}});
  }}

  function setupReorder(container) {{
    var dragEl = null;

    directGroups(container).forEach(function(group) {{
      var handle = group.querySelector(':scope > .menu-item-card .drag-handle');
      if (!handle) return;
      handle.addEventListener('dragstart', function(e) {{
        dragEl = group;
        group.classList.add('dragging');
        e.dataTransfer.effectAllowed = 'move';
        try {{ e.dataTransfer.setData('text/plain', group.dataset.itemId || ''); }} catch (err) {{}}
      }});
      handle.addEventListener('dragend', function() {{
        if (dragEl) dragEl.classList.remove('dragging');
        dragEl = null;
        saveOrder(container);
      }});
    }});

    container.addEventListener('dragover', function(e) {{
      if (!dragEl) return;
      e.preventDefault();
      var target = e.target.closest('.menu-item-group');
      if (!target || target === dragEl || target.parentElement !== container) return;
      var rect = target.getBoundingClientRect();
      var after = (e.clientY - rect.top) / rect.height > 0.5;
      container.insertBefore(dragEl, after ? target.nextSibling : target);
    }});
  }}

  document.querySelectorAll('.menu-item-list, .menu-item-children').forEach(setupReorder);
}})();
</script>"#,

        menu_id         = crate::html_escape(&menu.id),
        menu_name       = crate::html_escape(&menu.name),
        location_opts   = location_opts,
        items_section   = items_section,
        page_opts_add   = page_opts_add,
        parent_opts_add = parent_opts_add,
    );

    crate::admin_page("Edit Menu", "/admin/menus", flash, &content, ctx)
}
