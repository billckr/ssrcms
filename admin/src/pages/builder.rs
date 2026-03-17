//! Visual page builder admin pages.

use uuid::Uuid;

pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub page_count: i64,
    pub updated_at: String,
}

pub struct PageRow {
    pub id: String,
    pub name: String,
    pub is_homepage: bool,
    pub updated_at: String,
}

// ── Project list ──────────────────────────────────────────────────────────────

pub fn render_project_list(projects: &[ProjectRow], ctx: &crate::PageContext) -> String {
    let rows = if projects.is_empty() {
        r#"<tr><td colspan="5" style="text-align:center;color:var(--muted)">
            No projects yet. Create one below to get started.
        </td></tr>"#.to_string()
    } else {
        projects.iter().map(|p| {
            let active_badge = if p.is_active {
                r#" <span class="badge badge-success" style="font-size:.7rem">Live</span>"#
            } else { "" };
            let activate_btn = if p.is_active {
                format!(
                    r#"<form method="POST" action="/admin/builder/deactivate" style="display:inline">
                        <button class="btn btn-sm" type="submit">Deactivate</button>
                    </form>"#
                )
            } else {
                format!(
                    r#"<form method="POST" action="/admin/builder/{id}/activate" style="display:inline">
                        <button class="btn btn-sm btn-primary" type="submit">Set Live</button>
                    </form>"#,
                    id = crate::html_escape(&p.id),
                )
            };
            format!(
                r#"<tr>
  <td><a href="/admin/builder/{id}">{name}</a>{active_badge}</td>
  <td>{desc}</td>
  <td>{pages}</td>
  <td>{updated}</td>
  <td class="actions">
    <a href="/admin/builder/{id}" class="icon-btn" title="Open">
      <img src="/admin/static/icons/edit.svg" alt="Open">
    </a>
    {activate_btn}
    <form method="POST" action="/admin/builder/{id}/delete" style="display:inline"
          onsubmit="return confirm('Delete this project and all its pages? This cannot be undone.')">
      <button class="icon-btn icon-danger" type="submit" title="Delete">
        <img src="/admin/static/icons/delete.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                id           = crate::html_escape(&p.id),
                name         = crate::html_escape(&p.name),
                active_badge = active_badge,
                desc         = p.description.as_deref().map(crate::html_escape).unwrap_or_default(),
                pages        = p.page_count,
                updated      = crate::html_escape(&p.updated_at),
                activate_btn = activate_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem">
  <p style="margin:0;color:var(--muted)">Organise your site pages into projects. One project can be live at a time.</p>
</div>
<table class="data-table" style="margin-bottom:2rem">
  <thead>
    <tr><th>Project</th><th>Description</th><th>Pages</th><th>Updated</th><th>Actions</th></tr>
  </thead>
  <tbody>{rows}</tbody>
</table>
<h3 style="margin-bottom:1rem">Create New Project</h3>
<form method="POST" action="/admin/builder/create" style="display:flex;gap:.75rem;align-items:flex-end;flex-wrap:wrap">
  <div class="form-group" style="margin:0">
    <label for="proj-name">Project Name</label>
    <input id="proj-name" type="text" name="name" required placeholder="e.g. Main Site" maxlength="255" style="width:220px">
  </div>
  <div class="form-group" style="margin:0">
    <label for="proj-desc">Description <span style="color:var(--muted)">(optional)</span></label>
    <input id="proj-desc" type="text" name="description" placeholder="e.g. Full site redesign 2026" maxlength="500" style="width:280px">
  </div>
  <button type="submit" class="btn btn-primary">Create Project</button>
</form>"#,
        rows = rows,
    );

    crate::admin_page("Page Builder", "/admin/builder", None, &content, ctx)
}

// ── Page list within a project ────────────────────────────────────────────────

pub fn render_page_list(project: &ProjectRow, pages: &[PageRow], ctx: &crate::PageContext) -> String {
    let active_badge = if project.is_active {
        r#" <span class="badge badge-success" style="font-size:.7rem">Live</span>"#
    } else { "" };

    let rows = if pages.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">
            No pages yet. Click <strong>+ New Page</strong> to add one.
        </td></tr>"#.to_string()
    } else {
        pages.iter().map(|p| {
            let homepage_badge = if p.is_homepage {
                r#" <span class="badge badge-success" style="font-size:.7rem">Homepage</span>"#
            } else { "" };
            let set_hp_btn = if !p.is_homepage {
                format!(
                    r#"<form method="POST" action="/admin/builder/{proj}/pages/{page}/set-homepage" style="display:inline">
                        <button class="btn btn-sm" type="submit" title="Set as homepage">Set Homepage</button>
                    </form>"#,
                    proj = crate::html_escape(&project.id),
                    page = crate::html_escape(&p.id),
                )
            } else { String::new() };
            format!(
                r#"<tr>
  <td><a href="/admin/builder/{proj}/pages/{page}">{name}</a>{homepage_badge}</td>
  <td>{updated}</td>
  <td class="actions">
    <a href="/admin/builder/{proj}/pages/{page}" class="icon-btn" title="Edit">
      <img src="/admin/static/icons/edit.svg" alt="Edit">
    </a>
    {set_hp_btn}
    <form method="POST" action="/admin/builder/{proj}/pages/{page}/delete" style="display:inline"
          onsubmit="return confirm('Delete this page?')">
      <button class="icon-btn icon-danger" type="submit" title="Delete">
        <img src="/admin/static/icons/delete.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                proj         = crate::html_escape(&project.id),
                page         = crate::html_escape(&p.id),
                name         = crate::html_escape(&p.name),
                homepage_badge = homepage_badge,
                updated      = crate::html_escape(&p.updated_at),
                set_hp_btn   = set_hp_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem">
  <div>
    <a href="/admin/builder" style="color:var(--muted);font-size:.875rem">← All Projects</a>
    <h2 style="margin:.25rem 0 0">{name}{active_badge}</h2>
    {desc}
  </div>
  <a href="/admin/builder/{proj_id}/pages/new" class="btn btn-primary">+ New Page</a>
</div>
<table class="data-table">
  <thead>
    <tr><th>Page</th><th>Updated</th><th>Actions</th></tr>
  </thead>
  <tbody>{rows}</tbody>
</table>"#,
        name         = crate::html_escape(&project.name),
        active_badge = active_badge,
        desc         = project.description.as_deref()
            .map(|d| format!(r#"<p style="margin:.25rem 0 0;color:var(--muted);font-size:.875rem">{}</p>"#, crate::html_escape(d)))
            .unwrap_or_default(),
        proj_id      = crate::html_escape(&project.id),
        rows         = rows,
    );

    crate::admin_page(
        &format!("{} — Page Builder", project.name),
        "/admin/builder",
        None,
        &content,
        ctx,
    )
}

// ── Editor shell ──────────────────────────────────────────────────────────────

pub fn render_editor(
    page_id: Option<Uuid>,
    page_name: &str,
    project_id: Uuid,
    site_id: Uuid,
    _ctx: &crate::PageContext,
) -> String {
    let page_id_js = match page_id {
        Some(id) => format!(r#""{}""#, id),
        None => "null".to_string(),
    };
    let name_escaped = crate::html_escape(page_name);

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
      pageId:     {page_id_js},
      pageName:   "{name_escaped}",
      projectId:  "{project_id}",
      siteId:     "{site_id}",
    }};
  </script>
  <script type="module" src="/admin/static/builder/builder.js"></script>
</body>
</html>"#,
        page_id_js   = page_id_js,
        name_escaped = name_escaped,
        project_id   = project_id,
        site_id      = site_id,
    )
}
