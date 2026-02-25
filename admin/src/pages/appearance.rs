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

    // Build <select> options — files with a backup get a ★ marker
    let options: String = {
        let mut o = format!(r#"<option value="">— select a file —</option>"#);
        for f in files {
            let sel = if f.is_selected { " selected" } else { "" };
            let marker = if f.has_backup { " ★" } else { "" };
            o.push_str(&format!(
                r#"<option value="{val}"{sel}>{label}</option>"#,
                val   = crate::html_escape(&f.rel_path),
                sel   = sel,
                label = crate::html_escape(&format!("{}{}", &f.rel_path, marker)),
            ));
        }
        o
    };

    let file_picker = format!(
        r#"<form method="GET" action="/admin/appearance/editor/{theme}" style="display:contents;">
  <select name="file" class="editor-file-select" onchange="this.form.submit()"
          aria-label="Select theme file" title="Navigate to file">
    {options}
  </select>
</form>"#,
        theme = theme_esc,
        options = options,
    );

    // Top toolbar — always visible
    let toolbar = format!(
        r#"<div class="editor-topbar">
  <a href="/admin/appearance" class="btn btn-sm btn-secondary">&#8592; Themes</a>
  {picker}
</div>"#,
        picker = file_picker,
    );

    // Editor body — shown only when a file is selected
    let body = if let Some(rel) = selected {
        let rel_esc  = crate::html_escape(rel);
        let content_esc = crate::html_escape(content);

        let restore_btn = if has_backup {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/restore" style="display:contents"
     onsubmit="return confirm('Restore the original backup? Your current edits will be overwritten.')">
  <input type="hidden" name="file" value="{file}">
  <button type="submit" class="btn btn-sm btn-secondary">Restore original</button>
</form>"#,
                theme = theme_esc,
                file  = rel_esc,
            )
        } else {
            String::new()
        };

        format!(
            r#"<div class="editor-meta">
  <span class="editor-filename">{file}</span>
  {restore}
</div>
<form method="POST" action="/admin/appearance/editor/{theme}/save" class="editor-form" id="save-form">
  <input type="hidden" name="file" value="{file}">
  <textarea name="content" class="editor-textarea" spellcheck="false" autocorrect="off" autocapitalize="off">{content}</textarea>
</form>
<div class="editor-actions">
  <button type="submit" form="save-form" class="btn btn-primary">Save file</button>
  {restore2}
</div>"#,
            file     = rel_esc,
            theme    = theme_esc,
            content  = content_esc,
            restore  = restore_btn.clone(),
            restore2 = restore_btn,
        )
    } else {
        r#"<div class="editor-hint">Select a file above to start editing.</div>"#.to_string()
    };

    let content_html = format!(
        r#"<div class="editor-wrap">{toolbar}{body}</div>"#,
        toolbar = toolbar,
        body    = body,
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
