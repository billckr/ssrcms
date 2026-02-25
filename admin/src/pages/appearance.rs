use crate::admin_page;

pub struct ThemeInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub active: bool,
    pub has_screenshot: bool,
    /// Origin of this theme: `"global"` (available to all sites) or `"site"` (site-specific).
    pub source: String,
    /// Whether the current user is permitted to delete this theme.
    /// Computed server-side; never shown for active themes.
    pub can_delete: bool,
    /// Number of sites currently using this theme (global themes only).
    pub in_use_by: usize,
}

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let cards: String = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found. Add a theme directory to <code>themes/</code> and restart the server.</p>
        </div>"#.to_string()
    } else {
        themes.iter().map(|t| render_card(t, ctx)).collect()
    };

    let content = format!(
        r#"<div class="theme-list">{cards}</div>
<div class="theme-upload-section">
  <h2>Upload Theme</h2>
  <p class="muted">Upload a <code>.zip</code> file containing a valid theme. The zip must include <code>theme.toml</code> and all required templates.</p>
  <form method="post" action="/admin/appearance/upload" enctype="multipart/form-data" class="upload-form">
    <div class="form-group">
      <label for="theme_zip">Theme zip file</label>
      <input type="file" id="theme_zip" name="file" accept=".zip" required>
    </div>
    <button type="submit" class="btn btn-primary">Upload &amp; Install</button>
  </form>
</div>"#
    );

    admin_page("Appearance", "/admin/appearance", flash, &content, ctx)
}

pub fn render(themes: &[ThemeInfo], ctx: &crate::PageContext) -> String {
    render_with_flash(themes, None, ctx)
}

// ── Theme file editor ─────────────────────────────────────────────────────────

pub struct EditorFile {
    pub rel_path: String,
    pub is_selected: bool,
    pub has_backup: bool,
}

pub fn render_theme_editor(
    theme_name: &str,
    files: &[EditorFile],
    selected: Option<&str>,
    content: &str,
    has_backup: bool,
    flash: Option<&str>,
    ctx: &crate::PageContext,
) -> String {
    let theme_esc = crate::html_escape(theme_name);

    let file_tree: String = files.iter().map(|f| {
        let active_class = if f.is_selected { " class=\"editor-file-active\"" } else { "" };
        let bak_dot = if f.has_backup {
            r#" <span class="editor-bak-dot" title="Original backup exists">●</span>"#
        } else { "" };
        format!(
            r#"<li><a href="/admin/appearance/editor/{theme}?file={file_enc}"{active}>{file_name}{bak}</a></li>"#,
            theme = theme_esc,
            file_enc = crate::html_escape(&f.rel_path),
            active = active_class,
            file_name = crate::html_escape(&f.rel_path),
            bak = bak_dot,
        )
    }).collect::<Vec<_>>().join("\n");

    let editor_panel = if let Some(rel) = selected {
        let rel_esc = crate::html_escape(rel);
        let content_esc = crate::html_escape(content);
        let restore_btn = if has_backup {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/restore" style="display:inline"
    onsubmit="return confirm('Restore original backup? Your current edits will be overwritten.')">
  <input type="hidden" name="file" value="{file}">
  <button type="submit" class="btn btn-sm btn-secondary">Restore original</button>
</form>"#,
                theme = theme_esc,
                file = rel_esc,
            )
        } else { String::new() };
        format!(
            r#"<div class="editor-toolbar">
  <span class="editor-filename">{file}</span>
  {restore}
  <a href="/admin/appearance" class="btn btn-sm btn-secondary" style="margin-left:auto">&#8592; Back to themes</a>
</div>
<form method="POST" action="/admin/appearance/editor/{theme}/save">
  <input type="hidden" name="file" value="{file}">
  <textarea id="editor-area" name="content" class="editor-textarea" spellcheck="false">{content}</textarea>
  <div class="editor-footer">
    <button type="submit" class="btn btn-primary">Save file</button>
    {restore2}
  </div>
</form>"#,
            file = rel_esc,
            theme = theme_esc,
            content = content_esc,
            restore = restore_btn.clone(),
            restore2 = restore_btn,
        )
    } else {
        format!(
            r#"<div class="editor-empty">
  <p>Select a file from the left panel to edit it.</p>
  <a href="/admin/appearance" class="btn btn-secondary">&#8592; Back to themes</a>
</div>"#
        )
    };

    let content_html = format!(
        r#"<div class="editor-layout">
  <div class="editor-sidebar">
    <div class="editor-sidebar-title">Theme: {theme}</div>
    <ul class="editor-file-list">{tree}</ul>
  </div>
  <div class="editor-main">{panel}</div>
</div>"#,
        theme = theme_esc,
        tree = file_tree,
        panel = editor_panel,
    );

    admin_page(
        &format!("Edit Theme: {}", crate::html_escape(theme_name)),
        "/admin/appearance",
        flash,
        &content_html,
        ctx,
    )
}

fn render_card(t: &ThemeInfo, ctx: &crate::PageContext) -> String {
    let active_class = if t.active { " active" } else { "" };

    let screenshot_html = if t.has_screenshot {
        format!(
            r#"<div class="theme-screenshot"><img src="/admin/theme-screenshot/{}" alt="{} preview"></div>"#,
            crate::html_escape(&t.name),
            crate::html_escape(&t.name),
        )
    } else {
        format!(
            r#"<div class="theme-screenshot theme-screenshot-placeholder"><span>{}</span></div>"#,
            crate::html_escape(&t.name),
        )
    };

    let activate_html = if t.active {
        r#"<button class="btn btn-secondary" disabled>Active</button>"#.to_string()
    } else {
        format!(
            r#"<form method="post" action="/admin/appearance/activate" style="display:inline;">
    <input type="hidden" name="theme" value="{}">
    <button type="submit" class="btn btn-primary">Activate</button>
</form>"#,
            crate::html_escape(&t.name)
        )
    };

    let edit_html = format!(
        r#"<a href="/admin/appearance/editor/{}" class="btn btn-sm btn-secondary">Edit files</a>"#,
        crate::html_escape(&t.name)
    );

    let delete_html = if t.can_delete {
        format!(
            r#"<form method="post" action="/admin/appearance/delete" style="display:inline;"
                data-confirm="Delete theme &quot;{name}&quot;? This cannot be undone." onsubmit="return confirm(this.dataset.confirm)">
    <input type="hidden" name="theme" value="{name_escaped}">
    <button type="submit" class="btn btn-danger">Delete</button>
</form>"#,
            name = crate::html_escape(&t.name),
            name_escaped = crate::html_escape(&t.name),
        )
    } else {
        String::new()
    };

    format!(
        r#"<div class="theme-card{active}">
  {screenshot}
  <div class="theme-card-header">
    <span class="theme-name">{name}</span>
    <span class="badge">{version}</span>
    {source_badge}
  </div>
  <p class="theme-description">{desc}</p>
  <p class="theme-author">by {author}</p>
  <div class="theme-actions">
    {activate}{edit}{delete}
  </div>
</div>"#,
        active = active_class,
        screenshot = screenshot_html,
        name = crate::html_escape(&t.name),
        version = crate::html_escape(&t.version),
        source_badge = {
            let source_label = if t.source == "global" { "global" } else { "site" };
            let in_use_badge = if ctx.is_global_admin && t.source == "global" && t.in_use_by > 0 {
                format!(
                    r#" <span class="badge badge-in-use" title="Active on {n} site(s) — cannot delete">used by {n} site{s}</span>"#,
                    n = t.in_use_by,
                    s = if t.in_use_by == 1 { "" } else { "s" },
                )
            } else {
                String::new()
            };
            format!(r#"<span class="badge">{source_label}</span>{in_use_badge}"#)
        },
        desc = crate::html_escape(&t.description),
        author = crate::html_escape(&t.author),
        activate = activate_html,
        edit = edit_html,
        delete = delete_html,
    )
}
