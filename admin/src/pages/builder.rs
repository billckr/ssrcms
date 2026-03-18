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
    pub slug: Option<String>,
    pub page_type: String,
    pub is_homepage: bool,
    pub updated_at: String,
}

// ── Project list ──────────────────────────────────────────────────────────────

pub fn render_project_list(projects: &[ProjectRow], flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let rows = if projects.is_empty() {
        r#"<tr><td colspan="5" style="text-align:center;color:var(--muted)">
            No projects yet. Create one below to get started.
        </td></tr>"#.to_string()
    } else {
        projects.iter().map(|p| {
            let active_badge = if p.is_active {
                r#" <span class="badge" style="font-size:.7rem;background:#16a34a;color:#fff">Live</span>"#
            } else { "" };
            let activate_btn = if p.is_active {
                format!(
                    r#"<form method="POST" action="/admin/builder/deactivate" style="display:inline">
                        <button class="btn btn-sm" type="submit" style="background:#e2e8f0;border-color:#cbd5e1;color:#475569">Deactivate</button>
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
            let id_esc    = crate::html_escape(&p.id);
            let name_esc  = crate::html_escape(&p.name);
            let is_active = if p.is_active { "true" } else { "false" };
            format!(
                r#"<tr>
  <td><a href="/admin/builder/{id}">{name}</a>{active_badge}</td>
  <td>{desc}</td>
  <td>{pages}</td>
  <td>{updated}</td>
  <td class="actions" style="white-space:nowrap">
    <a href="/admin/builder/{id}/pages/new" class="icon-btn" title="New Page ">
      <img src="/admin/static/icons/code.svg" alt="New Page">
    </a>
    <button class="icon-btn" type="button" title="Rename"
            onclick="openRenameDialog('{id}', '{name_js}', '{desc_js}')">
      <img src="/admin/static/icons/edit.svg" alt="Rename">
    </button>
    <form method="POST" action="/admin/builder/{id}/delete" style="display:inline"
          onsubmit="return confirmDelete(this, {is_active})">
      <button class="icon-btn icon-danger" type="submit" title="Delete">
        <img src="/admin/static/icons/trash-2.svg" alt="Delete">
      </button>
    </form>
    {activate_btn}
  </td>
</tr>"#,
                id           = id_esc,
                name         = name_esc,
                name_js      = crate::html_escape(&p.name.replace('\'', "\\'")),
                desc_js      = crate::html_escape(&p.description.as_deref().unwrap_or("").replace('\'', "\\'")),
                active_badge = active_badge,
                desc         = p.description.as_deref().map(crate::html_escape).unwrap_or_default(),
                pages        = p.page_count,
                updated      = crate::html_escape(&p.updated_at),
                activate_btn = activate_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let flash_html = match flash {
        Some(msg) => format!(
            r#"<div style="background:#fef2f2;border:1px solid #fca5a5;color:#b91c1c;padding:.75rem 1rem;border-radius:6px;margin-bottom:1rem">{}</div>"#,
            crate::html_escape(msg),
        ),
        None => String::new(),
    };

    let content = format!(
        r#"{flash_html}<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem">
  <p style="margin:0;color:var(--muted)">Organise your site pages into projects. One project can be live at a time.</p>
  <button type="button" class="btn btn-primary" onclick="document.getElementById('new-project-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">+ Project</button>
</div>
<table class="data-table" style="margin-bottom:2rem">
  <thead>
    <tr><th>Project</th><th>Description</th><th>Pages</th><th>Updated</th><th>Actions</th></tr>
  </thead>
  <tbody>{rows}</tbody>
</table>

<!-- New project dialog -->
<dialog id="new-project-dialog" style="border:1px solid #e2e8f0;border-radius:8px;padding:1.5rem;min-width:400px;box-shadow:0 4px 24px rgba(0,0,0,.12);position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);margin:0">
  <form method="POST" action="/admin/builder/create">
    <h3 style="margin:0 0 1rem">New Project</h3>
    <div class="form-group">
      <label for="proj-name">Project Name</label>
      <input id="proj-name" type="text" name="name" required maxlength="35"
             placeholder="e.g. Main Site" style="width:100%">
    </div>
    <div class="form-group">
      <label for="proj-desc">Description <span style="color:var(--muted)">(optional)</span></label>
      <input id="proj-desc" type="text" name="description" maxlength="100"
             placeholder="e.g. Full site redesign 2026" style="width:100%">
    </div>
    <div style="display:flex;gap:.5rem;justify-content:flex-end;margin-top:1rem">
      <button type="button" class="btn" onclick="this.closest('form').reset();closeNewProjectDialog()">Cancel</button>
      <button type="submit" class="btn btn-primary">Save</button>
    </div>
  </form>
</dialog>

<!-- Rename project dialog -->
<dialog id="rename-dialog" style="border:1px solid #e2e8f0;border-radius:8px;padding:1.5rem;min-width:400px;box-shadow:0 4px 24px rgba(0,0,0,.12);position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);margin:0">
  <form method="POST" id="rename-form">
    <h3 style="margin:0 0 1rem">Edit Project</h3>
    <div class="form-group">
      <label for="rename-input">Project Name</label>
      <input id="rename-input" type="text" name="project_name" required maxlength="35"
             style="width:100%" autocomplete="off">
    </div>
    <div class="form-group">
      <label for="rename-desc-input">Description <span style="color:var(--muted)">(optional)</span></label>
      <input id="rename-desc-input" type="text" name="description" maxlength="100"
             style="width:100%" autocomplete="off">
    </div>
    <div style="display:flex;gap:.5rem;justify-content:flex-end;margin-top:1rem">
      <button type="button" class="btn" onclick="document.getElementById('rename-dialog').close();document.querySelector('.admin-content').style.filter=''">Cancel</button>
      <button type="submit" class="btn btn-primary">Save</button>
    </div>
  </form>
</dialog>
<script>
function closeNewProjectDialog() {{
  document.getElementById('new-project-dialog').close();
  document.querySelector('.admin-content').style.filter = '';
}}
document.getElementById('new-project-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
function confirmDelete(form, isActive) {{
  if (isActive) {{
    alert('This project is currently live. Deactivate it before deleting.');
    return false;
  }}
  return confirm('Delete this project and all its pages? This cannot be undone.');
}}
function openRenameDialog(id, currentName, currentDesc) {{
  var dlg = document.getElementById('rename-dialog');
  var form = document.getElementById('rename-form');
  document.getElementById('rename-input').value = currentName;
  document.getElementById('rename-desc-input').value = currentDesc || '';
  form.action = '/admin/builder/' + id + '/rename';
  dlg.showModal();
  document.getElementById('rename-input').select();
  document.querySelector('.admin-content').style.filter = 'blur(1.5px)';
}}
document.getElementById('rename-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
</script>"#,
        flash_html = flash_html,
        rows = rows,
    );

    crate::admin_page("Page Builder", "/admin/builder", None, &content, ctx)
}

// ── Page list within a project ────────────────────────────────────────────────

pub fn render_page_list(project: &ProjectRow, pages: &[PageRow], ctx: &crate::PageContext) -> String {
    let active_badge = if project.is_active {
        r#" <span class="badge" style="font-size:.7rem;background:#16a34a;color:#fff">Live</span>"#
    } else { "" };

    let rows = if pages.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">
            No pages yet. Click <strong>+ New Page</strong> to add one.
        </td></tr>"#.to_string()
    } else {
        pages.iter().map(|p| {
            let homepage_badge = if p.is_homepage {
                r#" <img src="/admin/static/icons/home.svg" title="Homepage" style="width:18px;height:18px;vertical-align:middle;margin-left:4px;filter:invert(35%) sepia(1) saturate(5) hue-rotate(200deg) brightness(0.9)">"#
            } else { "" };
            let post_template_badge = if p.page_type == "post_template" {
                r#" <img src="/admin/static/icons/layout.svg" title="Post Template" style="width:18px;height:18px;vertical-align:middle;margin-left:4px;filter:invert(45%) sepia(1) saturate(4) hue-rotate(10deg) brightness(0.95)">"#
            } else { "" };
            let archive_template_badge = if p.page_type == "archive_template" {
                r#" <img src="/admin/static/icons/archive.svg" title="Archive Template" style="width:18px;height:18px;vertical-align:middle;margin-left:4px;filter:invert(45%) sepia(1) saturate(4) hue-rotate(10deg) brightness(0.95)">"#
            } else { "" };
            let url_display = match p.page_type.as_str() {
                "homepage"         => "/".to_string(),
                "post_template"    => "(all posts)".to_string(),
                "archive_template" => "(categories &amp; tags)".to_string(),
                _                  => format!("/{}", p.slug.as_deref().unwrap_or("")),
            };
            format!(
                r#"<tr>
  <td><a href="/admin/builder/{proj}/pages/{page}">{name}</a>{homepage_badge}{post_template_badge}{archive_template_badge}</td>
  <td style="color:var(--muted);font-size:.875rem;font-family:monospace">{url}</td>
  <td>{updated}</td>
  <td class="actions">
    <a href="/admin/builder/{proj}/pages/{page}" class="icon-btn" title="Edit">
      <img src="/admin/static/icons/edit.svg" alt="Edit">
    </a>
    <button class="icon-btn" type="button" title="Duplicate page"
            onclick="openDuplicateDialog('{proj}', '{page}', '{name_js}')">
      <img src="/admin/static/icons/copy.svg" alt="Duplicate">
    </button>
    <form method="POST" action="/admin/builder/{proj}/pages/{page}/delete" style="display:inline"
          onsubmit="return confirm('Delete this page?')">
      <button class="icon-btn icon-danger" type="submit" title="Delete">
        <img src="/admin/static/icons/trash-2.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                proj                    = crate::html_escape(&project.id),
                page                    = crate::html_escape(&p.id),
                name                    = crate::html_escape(&p.name),
                name_js                 = crate::html_escape(&p.name.replace('\'', "\\'")),
                homepage_badge          = homepage_badge,
                post_template_badge     = post_template_badge,
                archive_template_badge  = archive_template_badge,
                url                     = url_display,
                updated                 = crate::html_escape(&p.updated_at),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:flex-end;align-items:center;gap:.75rem;margin-bottom:1.5rem">
  <a href="/admin/builder" class="btn">← Projects</a>
  {active_badge}
  <span style="flex:1"></span>
  <a href="/admin/builder/{proj_id}/pages/new" class="btn btn-primary">+ New Page</a>
</div>
<table class="data-table">
  <thead>
    <tr><th>Page</th><th>URL</th><th>Updated</th><th>Actions</th></tr>
  </thead>
  <tbody>{rows}</tbody>
</table>

<!-- Duplicate page dialog -->
<dialog id="duplicate-dialog" style="border:1px solid #e2e8f0;border-radius:8px;padding:1.5rem;min-width:380px;box-shadow:0 4px 24px rgba(0,0,0,.12);position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);margin:0">
  <form method="POST" id="duplicate-form">
    <input type="hidden" name="name" id="duplicate-name-input-hidden">
    <h3 style="margin:0 0 1rem">Duplicate Page</h3>
    <div class="form-group">
      <label for="duplicate-name-input">New Page Name</label>
      <input id="duplicate-name-input" type="text" required maxlength="100"
             style="width:100%" autofocus>
    </div>
    <div style="display:flex;gap:.5rem;justify-content:flex-end;margin-top:1rem">
      <button type="button" class="btn" onclick="document.getElementById('duplicate-dialog').close();document.querySelector('.admin-content').style.filter=''">Cancel</button>
      <button type="submit" class="btn btn-primary">Duplicate &amp; Open Editor</button>
    </div>
  </form>
</dialog>
<script>
function openDuplicateDialog(projId, pageId, currentName) {{
  var dlg  = document.getElementById('duplicate-dialog');
  var form = document.getElementById('duplicate-form');
  var inp  = document.getElementById('duplicate-name-input');
  inp.value = currentName + ' (copy)';
  form.action = '/admin/builder/' + projId + '/pages/' + pageId + '/duplicate';
  dlg.showModal();
  inp.select();
  document.querySelector('.admin-content').style.filter = 'blur(1.5px)';
}}
document.getElementById('duplicate-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
document.getElementById('duplicate-form').addEventListener('submit', function() {{
  document.getElementById('duplicate-name-input-hidden').value =
    document.getElementById('duplicate-name-input').value;
}});
</script>"#,
        active_badge = active_badge,
        proj_id      = crate::html_escape(&project.id),
        rows         = rows,
    );

    crate::admin_page(
        &format!("Page Builder — {}", project.name),
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
    project_name: &str,
    site_label: &str,
    pure_mode: bool,
    menus_json: &str,
    _ctx: &crate::PageContext,
) -> String {
    let page_id_js = match page_id {
        Some(id) => format!(r#""{}""#, id),
        None => "null".to_string(),
    };
    let name_escaped    = crate::html_escape(page_name);
    let project_escaped = crate::html_escape(project_name);
    let site_escaped    = crate::html_escape(site_label);
    let pure_mode_js    = if pure_mode { "true" } else { "false" };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Page Builder — Synaptic Signals</title>
  <link rel="stylesheet" href="/admin/static/builder/builder.css">
  <style>
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{ font: 14px/1.5 system-ui, sans-serif; }}
    html, body {{ height: 100%; }}
    #root {{ height: 100%; }}
  </style>
</head>
<body>
  <div id="root"></div>
  <script>
    window.__builderInit = {{
      pageId:      {page_id_js},
      pageName:    "{name_escaped}",
      projectId:   "{project_id}",
      siteId:      "{site_id}",
      projectName: "{project_escaped}",
      siteLabel:   "{site_escaped}",
      pureMode:    {pure_mode_js},
      menus:       {menus_json},
    }};
  </script>
  <script type="module" src="/admin/static/builder/builder.js"></script>
</body>
</html>"#,
        page_id_js      = page_id_js,
        name_escaped    = name_escaped,
        project_escaped = project_escaped,
        site_escaped    = site_escaped,
        project_id      = project_id,
        site_id         = site_id,
        pure_mode_js    = pure_mode_js,
        menus_json      = menus_json,
    )
}

// ── New page form ─────────────────────────────────────────────────────────────

pub fn render_new_page_form(
    project: &ProjectRow,
    has_homepage: bool,
    has_post_template: bool,
    has_archive_template: bool,
    existing_pages: &[PageRow],
    ctx: &crate::PageContext,
) -> String {
    let homepage_option = if has_homepage {
        r#"<option value="homepage" disabled>Homepage (already exists)</option>"#
    } else {
        r#"<option value="homepage">Homepage — serves at /</option>"#
    };
    let post_template_option = if has_post_template {
        r#"<option value="post_template" disabled>Post Template (already exists)</option>"#
    } else {
        r#"<option value="post_template">Post Template — wraps all post URLs</option>"#
    };
    let archive_template_option = if has_archive_template {
        r#"<option value="archive_template" disabled>Archive Template (already exists)</option>"#
    } else {
        r#"<option value="archive_template">Archive Template — wraps category &amp; tag pages</option>"#
    };

    let copy_from_field = if existing_pages.is_empty() {
        String::new()
    } else {
        let options = existing_pages.iter().map(|p| {
            format!(
                r#"<option value="{id}">{name}</option>"#,
                id   = crate::html_escape(&p.id),
                name = crate::html_escape(&p.name),
            )
        }).collect::<Vec<_>>().join("\n");
        format!(
            r#"<div class="form-group">
      <label for="copy-from">Copy layout from <span style="color:var(--muted)">(optional)</span></label>
      <select id="copy-from" name="copy_from" style="width:100%">
        <option value="">— Start blank —</option>
        {options}
      </select>
      <p style="margin:.35rem 0 0;font-size:.8rem;color:var(--muted)">
        Opens the editor pre-filled with that page's current draft layout.
      </p>
    </div>"#,
            options = options,
        )
    };

    let content = format!(
        r#"<a href="/admin/builder/{proj_id}" style="color:var(--muted);font-size:.875rem;display:inline-block;margin-bottom:1rem">
  ← Back to {proj_name}
</a>
<div style="max-width:520px">
  <form method="POST" action="/admin/builder/{proj_id}/pages/new">
    <div class="form-group">
      <label for="page-name">Page Name</label>
      <input id="page-name" type="text" name="name" required
             placeholder="e.g. About Us" maxlength="100" autofocus
             style="width:100%">
    </div>
    <div class="form-group">
      <label for="page-type">Page Type</label>
      <select id="page-type" name="page_type" onchange="toggleSlug(this.value)" style="width:100%">
        {homepage_option}
        <option value="page" selected>Regular page</option>
        {post_template_option}
        {archive_template_option}
      </select>
    </div>
    <div class="form-group" id="slug-group">
      <label for="page-slug">URL Slug</label>
      <div style="display:flex;align-items:center;gap:.5rem">
        <span style="color:var(--muted)">/</span>
        <input id="page-slug" type="text" name="slug"
               placeholder="about-us" maxlength="100"
               style="flex:1"
               pattern="[a-zA-Z0-9][a-zA-Z0-9\-_]*"
               title="Letters, numbers, hyphens and underscores only">
      </div>
      <p style="margin:.35rem 0 0;font-size:.8rem;color:var(--muted)">
        Letters, numbers, hyphens and underscores only. No spaces.
      </p>
    </div>
    {copy_from_field}
    <button type="submit" class="btn btn-primary">Create Page &amp; Open Editor</button>
  </form>
</div>
<script>
  function toggleSlug(type) {{
    var noSlug = type === 'homepage' || type === 'post_template';
    document.getElementById('slug-group').style.display = noSlug ? 'none' : '';
    document.getElementById('page-slug').required = !noSlug;
  }}
  toggleSlug(document.getElementById('page-type').value);

  var slugEdited = false;
  document.getElementById('page-slug').addEventListener('input', function() {{
    slugEdited = true;
  }});
  document.getElementById('page-name').addEventListener('input', function() {{
    if (slugEdited) return;
    var slug = this.value
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s-]/g, '')
      .replace(/\s+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');
    document.getElementById('page-slug').value = slug;
  }});
</script>"#,
        proj_id                  = crate::html_escape(&project.id),
        proj_name                = crate::html_escape(&project.name),
        homepage_option          = homepage_option,
        post_template_option     = post_template_option,
        archive_template_option  = archive_template_option,
        copy_from_field          = copy_from_field,
    );

    crate::admin_page(
        &format!("New Page — {}", project.name),
        "/admin/builder",
        None,
        &content,
        ctx,
    )
}
