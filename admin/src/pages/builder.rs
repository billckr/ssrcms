//! Visual page builder admin UI — server-rendered HTML with SortableJS + Quill via CDN.

use crate::admin_page;

// ── Data types ───────────────────────────────────────────────────────────────

pub struct CompositionRow {
    pub id: String,
    pub name: String,
    pub layout: String,
    /// Folder name of the theme this composition belongs to.
    pub theme_name: String,
    /// Whether this composition's theme is the currently active theme for the site.
    pub is_active: bool,
    pub updated_at: String,
}

pub struct BuilderEditorData {
    pub id: String,
    pub name: String,
    pub layout: String,
    /// Folder name of the theme this composition belongs to.
    pub theme_name: String,
    /// Whether this composition's theme is the currently active theme.
    pub is_active: bool,
    /// JSON string of the full composition (passed to JS as window.__builderInit).
    pub composition_json: String,
}

// ── Render: "name your theme" form ──────────────────────────────────────────

pub fn render_new_theme_form(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let flash_html = match flash {
        Some(msg) => format!(r#"<div class="flash error">{}</div>"#, html_esc(msg)),
        None => String::new(),
    };
    let content = format!(
        r#"{flash_html}
<div class="form-section" style="max-width:480px">
  <h2 style="margin-top:0">New Visual Composition</h2>
  <p style="color:#666;margin-bottom:1.5rem">Choose a name for this theme. A new theme will be created in your site's theme library, ready to build and activate.</p>
  <form method="POST" action="/admin/appearance/builder/new">
    <div class="form-group">
      <label for="theme-name">Theme name</label>
      <input type="text" id="theme-name" name="name" required placeholder="e.g. My Homepage" maxlength="64" autofocus>
    </div>
    <div style="display:flex;gap:.75rem;margin-top:1.5rem">
      <button type="submit" class="btn btn-primary">Create &amp; Open Builder</button>
      <a href="/admin/appearance/builder" class="btn">Cancel</a>
    </div>
  </form>
</div>"#,
        flash_html = flash_html,
    );
    admin_page("New Composition", "/admin/appearance", None, &content, ctx)
}

// ── Render: composition list ─────────────────────────────────────────────────

pub fn render_list(
    compositions: &[CompositionRow],
    flash: Option<&str>,
    ctx: &crate::PageContext,
) -> String {
    let rows: String = if compositions.is_empty() {
        r#"<tr><td colspan="4" style="padding:1.5rem;text-align:center;color:#888">No compositions yet. <a href="/admin/appearance/builder/new">Create one</a>.</td></tr>"#.to_string()
    } else {
        compositions.iter().map(|c| {
            let active_badge = if c.is_active {
                r#" <span style="font-size:.7rem;padding:.1rem .4rem;background:#d1fae5;color:#065f46;border-radius:3px;font-weight:600">Active</span>"#
            } else {
                ""
            };
            format!(
                "<tr>\
                   <td><a href=\"/admin/appearance/builder/{id}/edit\">{name}</a>{badge}</td>\
                   <td><code style=\"font-size:.8rem\">{theme}</code></td>\
                   <td>{layout}</td>\
                   <td>{updated}</td>\
                   <td class=\"row-actions\">\
                     <a href=\"/admin/appearance/builder/{id}/edit\" class=\"btn btn-sm\">Edit</a>\
                     <a href=\"/admin/appearance\" class=\"btn btn-sm\">Appearance</a>\
                     <form method=\"POST\" action=\"/admin/appearance/builder/{id}/delete\" style=\"display:inline\" onsubmit=\"return confirm('Delete this composition and its theme folder from disk? This cannot be undone.')\">\
                       <button type=\"submit\" class=\"btn btn-sm btn-danger\">Delete</button>\
                     </form>\
                   </td>\
                 </tr>",
                id = html_esc(&c.id),
                name = html_esc(&c.name),
                badge = active_badge,
                theme = html_esc(&c.theme_name),
                layout = html_esc(&c.layout),
                updated = html_esc(&c.updated_at),
            )
        }).collect()
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem">
  <h1 style="margin:0">Visual Builder</h1>
  <a href="/admin/appearance/builder/new" class="btn btn-primary">+ New Composition</a>
</div>
<p style="color:#666;margin-bottom:1rem">Each composition is a named theme in your site library. Activate it from <a href="/admin/appearance">Appearance</a> to make it your homepage.</p>
<table class="data-table">
  <thead><tr><th>Name</th><th>Theme folder</th><th>Layout</th><th>Updated</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows,
    );

    admin_page("Visual Builder", "/admin/appearance", flash, &content, ctx)
}

// ── Render: builder editor ────────────────────────────────────────────────────

pub fn render_editor(data: &BuilderEditorData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let save_url = format!("/admin/appearance/builder/{}/save", html_esc(&data.id));
    let preview_url = format!(
        "/admin/appearance/builder/preview?theme_name={}",
        html_esc(&data.theme_name)
    );

    let layout_options: String = [
        ("single-column", "Single Column"),
        ("left-sidebar", "Left Sidebar"),
        ("right-sidebar", "Right Sidebar"),
    ]
    .iter()
    .map(|(val, label)| {
        let sel = if *val == data.layout { " selected" } else { "" };
        format!(r#"<option value="{val}"{sel}>{label}</option>"#, val = val, label = label, sel = sel)
    })
    .collect();

    let theme_note = if data.is_active {
        format!(
            r#"<span style="background:#d1fae5;color:#065f46;padding:.3rem .7rem;border-radius:4px;font-size:.82rem;font-weight:600">Active theme</span>"#
        )
    } else {
        format!(
            r#"<a href="/admin/appearance" class="btn" style="font-size:.82rem">Activate in Appearance →</a>"#
        )
    };

    let init_script = format!(
        r#"<script>window.__builderInit = {{ saveUrl: "{save_url}", previewUrl: "{preview_url}", compId: "{comp_id}", layout: "{layout}", themeName: "{theme_name}", composition: {comp_json} }};</script>"#,
        save_url = save_url,
        preview_url = preview_url,
        comp_id = html_esc(&data.id),
        layout = html_esc(&data.layout),
        theme_name = html_esc(&data.theme_name),
        comp_json = data.composition_json,
    );

    let content = format!(
        r#"<!-- CDN deps -->
<script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.2/Sortable.min.js"></script>
<link  href="https://cdn.jsdelivr.net/npm/quill@2.0.3/dist/quill.snow.css" rel="stylesheet">
<script src="https://cdn.jsdelivr.net/npm/quill@2.0.3/dist/quill.js"></script>
{init_script}
<style>
.builder-shell {{ display:flex; flex-direction:column; gap:0; }}
.builder-topbar {{ display:flex; align-items:center; gap:.6rem; padding:.75rem 1rem; background:var(--sidebar-bg,#1e2330); border-radius:6px 6px 0 0; flex-wrap:wrap; }}
.builder-topbar input[type=text] {{ flex:1; min-width:160px; background:rgba(255,255,255,.08); border:1px solid rgba(255,255,255,.15); color:#fff; padding:.4rem .7rem; border-radius:4px; font-size:.9rem; }}
.builder-topbar select {{ background:rgba(255,255,255,.08); border:1px solid rgba(255,255,255,.15); color:#fff; padding:.4rem .7rem; border-radius:4px; font-size:.9rem; }}
.builder-topbar option {{ background:#1e2330; }}
.builder-workspace {{ display:grid; grid-template-columns:180px 1fr 280px; min-height:520px; border:1px solid var(--border,#e5e7eb); border-top:none; border-radius:0 0 6px 6px; }}
.builder-palette {{ background:#f8f9fa; border-right:1px solid var(--border,#e5e7eb); padding:.75rem .5rem; overflow-y:auto; }}
.builder-palette h3 {{ font-size:.7rem; text-transform:uppercase; letter-spacing:.06em; color:#888; margin:0 0 .5rem .25rem; }}
.palette-block {{ display:flex; align-items:center; gap:.4rem; padding:.4rem .5rem; margin-bottom:.3rem; background:#fff; border:1px solid #e0e0e0; border-radius:4px; cursor:grab; font-size:.82rem; user-select:none; }}
.palette-block:hover {{ background:#eef2ff; border-color:#a5b4fc; }}
.builder-canvas {{ padding:.75rem; overflow-y:auto; background:#fff; }}
.canvas-zone {{ margin-bottom:1rem; }}
.canvas-zone-label {{ font-size:.7rem; font-weight:600; text-transform:uppercase; letter-spacing:.06em; color:#888; margin-bottom:.35rem; display:flex; align-items:center; }}
.canvas-zone-drop {{ min-height:52px; border:2px dashed #ddd; border-radius:6px; padding:.35rem; transition:border-color .15s; }}
.canvas-zone-drop.drag-over {{ border-color:#6366f1; background:#eef2ff; }}
.canvas-block-chip {{ display:flex; align-items:center; justify-content:space-between; padding:.35rem .6rem; margin-bottom:.3rem; background:#f1f5f9; border:1px solid #e2e8f0; border-radius:4px; cursor:grab; font-size:.82rem; user-select:none; }}
.canvas-block-chip.selected {{ border-color:#6366f1; background:#eef2ff; }}
.canvas-block-chip:hover {{ background:#e2e8f0; }}
.chip-label {{ flex:1; }}
.chip-btn {{ background:none; border:none; cursor:pointer; padding:.1rem .25rem; color:#888; font-size:.8rem; border-radius:3px; }}
.chip-btn:hover {{ color:#333; background:rgba(0,0,0,.07); }}
.builder-config {{ border-left:1px solid var(--border,#e5e7eb); overflow:hidden; }}
.config-pane {{ padding:.75rem; overflow-y:auto; height:100%; }}
.config-pane h3 {{ font-size:.85rem; font-weight:600; margin:0 0 .75rem; padding-bottom:.5rem; border-bottom:1px solid #eee; }}
.config-field {{ margin-bottom:.75rem; }}
.config-field label {{ display:block; font-size:.78rem; font-weight:500; margin-bottom:.2rem; color:#555; }}
.config-field input[type=text],.config-field input[type=number],.config-field input[type=color],.config-field select {{ width:100%; padding:.35rem .5rem; border:1px solid #ddd; border-radius:4px; font-size:.83rem; }}
.config-empty {{ color:#aaa; font-size:.82rem; text-align:center; margin-top:2rem; padding:0 .75rem; line-height:1.5; }}
.drop-hint {{ color:#ccc; font-size:.75rem; text-align:center; padding:.75rem .5rem; pointer-events:none; }}
@media(max-width:900px) {{ .builder-workspace {{ grid-template-columns:160px 1fr; }} .builder-config {{ display:none; }} }}
</style>

<div class="builder-shell">
  <div class="builder-topbar">
    <input type="text" id="comp-name" value="{name}" placeholder="Composition name" style="max-width:220px">
    <select id="layout-picker" onchange="builderLayoutChanged(this.value)">
      {layout_options}
    </select>
    <button class="btn btn-primary" onclick="builderSave()">Save</button>
    {theme_note}
    <button class="btn" onclick="togglePreview()">Preview</button>
    <a href="/admin/appearance/builder" class="btn" style="margin-left:auto">← Back</a>
  </div>
  <div class="builder-workspace">
    <div class="builder-palette">
      <h3>Blocks</h3>
      <div id="palette-blocks">
        <div class="palette-block" data-block-type="text-block">📝 Text</div>
        <div class="palette-block" data-block-type="posts-grid">📰 Posts Grid</div>
        <div class="palette-block" data-block-type="nav-menu">🧭 Nav Menu</div>
        <div class="palette-block" data-block-type="contact-form">✉️ Contact Form</div>
      </div>
    </div>
    <div class="builder-canvas" id="canvas"></div>
    <div class="builder-config">
      <div class="config-pane">
        <div class="config-empty" id="config-empty">Select a block to configure it.</div>
        <div id="config-fields" style="display:none"></div>
      </div>
    </div>
  </div>
</div>

<div id="preview-section" style="display:none;margin-top:1.5rem">
  <h3 style="font-size:.85rem;font-weight:600;margin-bottom:.5rem">Live Preview</h3>
  <iframe id="builder-preview-frame" style="width:100%;height:600px;border:1px solid #ddd;border-radius:6px" title="Page preview"></iframe>
</div>

<script>
window.togglePreview = function() {{
  var s = document.getElementById("preview-section");
  s.style.display = s.style.display === "none" ? "block" : "none";
}};
</script>
<script src="/admin/static/builder.js"></script>"#,
        init_script = init_script,
        name = html_esc(&data.name),
        layout_options = layout_options,
        theme_note = theme_note,
    );

    admin_page("Visual Builder", "/admin/appearance", flash, &content, ctx)
}

pub fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
