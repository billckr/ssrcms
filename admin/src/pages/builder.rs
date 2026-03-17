//! Visual page builder admin pages.
//!
//! `render_list` — lists all compositions for the current site.
//! `render_editor` — serves the HTML shell that loads the Puck React bundle.

use uuid::Uuid;

pub struct CompositionRow {
    pub id: String,
    pub name: String,
    pub is_homepage: bool,
    pub updated_at: String,
}

pub fn render_list(rows: &[CompositionRow], ctx: &crate::PageContext) -> String {
    let table = if rows.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">
            No pages yet. Click <strong>New Page</strong> to get started.
        </td></tr>"#.to_string()
    } else {
        rows.iter().map(|r| {
            let homepage_badge = if r.is_homepage {
                r#" <span class="badge badge-success" style="font-size:.7rem">Homepage</span>"#
            } else {
                ""
            };
            let activate_btn = if r.is_homepage {
                format!(
                    r#"<form method="POST" action="/admin/builder/deactivate" style="display:inline">
                        <button class="btn btn-sm" type="submit" title="Deactivate as homepage">Deactivate</button>
                    </form>"#
                )
            } else {
                format!(
                    r#"<form method="POST" action="/admin/builder/activate/{id}" style="display:inline">
                        <button class="btn btn-sm btn-primary" type="submit" title="Set as homepage">Set Homepage</button>
                    </form>"#,
                    id = crate::html_escape(&r.id),
                )
            };
            format!(
                r#"<tr>
  <td>
    <a href="/admin/builder/edit/{id}">{name}</a>{homepage_badge}
  </td>
  <td>{updated}</td>
  <td class="actions">
    <a href="/admin/builder/edit/{id}" class="icon-btn" title="Edit">
      <img src="/admin/static/icons/edit.svg" alt="Edit">
    </a>
    {activate_btn}
    <form method="POST" action="/admin/builder/delete/{id}" style="display:inline"
          onsubmit="return confirm('Delete this page? This cannot be undone.')">
      <button class="icon-btn icon-danger" type="submit" title="Delete">
        <img src="/admin/static/icons/delete.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                id           = crate::html_escape(&r.id),
                name         = crate::html_escape(&r.name),
                homepage_badge = homepage_badge,
                updated      = crate::html_escape(&r.updated_at),
                activate_btn = activate_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem">
  <p style="margin:0;color:var(--muted)">Build pages visually — no code required.</p>
  <a href="/admin/builder/new" class="btn btn-primary">+ New Page</a>
</div>
<table class="data-table">
  <thead>
    <tr>
      <th>Page Name</th>
      <th>Last Updated</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>{table}</tbody>
</table>"#,
        table = table,
    );

    crate::admin_page("Page Builder", "/admin/builder", None, &content, ctx)
}

/// Renders the full-page Puck editor shell.
/// The shell is a minimal HTML page that loads the compiled React bundle from
/// `/admin/static/builder/` and injects `window.__builderInit` so the JS knows
/// which composition to load and which site it belongs to.
pub fn render_editor(
    composition_id: Option<Uuid>,
    composition_name: &str,
    site_id: Uuid,
    _ctx: &crate::PageContext,
) -> String {
    let comp_id_js = match composition_id {
        Some(id) => format!(r#""{}""#, id),
        None => "null".to_string(),
    };
    let name_escaped = crate::html_escape(composition_name);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Page Builder — Synaptic Signals</title>
  <link rel="stylesheet" href="/admin/static/builder/builder.css">
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    html, body, #root {{ height: 100%; width: 100%; overflow: hidden; }}
  </style>
</head>
<body>
  <div id="root"></div>
  <script>
    window.__builderInit = {{
      compositionId:   {comp_id_js},
      compositionName: "{name_escaped}",
      siteId:          "{site_id}",
    }};
  </script>
  <script type="module" src="/admin/static/builder/builder.js"></script>
</body>
</html>"#,
        comp_id_js   = comp_id_js,
        name_escaped = name_escaped,
        site_id      = site_id,
    )
}
