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

    let create_btn = if ctx.can_manage_appearance {
        r#"<div class="appearance-header" style="margin-bottom:1rem;">
  <a href="/admin/appearance/create" class="btn btn-primary">+ Create Theme</a>
</div>"#
    } else {
        ""
    };

    let content = format!(
        r#"{create_btn}<div class="theme-list">{cards}</div>
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

pub fn render_create_theme_form(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = r#"<form method="POST" action="/admin/appearance/create" class="form-section" style="max-width:520px;">
  <div class="form-group">
    <label for="name">Theme name <span class="required">*</span></label>
    <input type="text" id="name" name="name" required maxlength="64"
           placeholder="my-theme" pattern="[^/\\\.][^/\\]*"
           title="No slashes, backslashes, or leading dots. Max 64 characters.">
    <p class="muted">Used as the folder name. Letters, numbers, hyphens, and underscores only.</p>
  </div>
  <div class="form-group">
    <label for="description">Description</label>
    <input type="text" id="description" name="description" maxlength="200" placeholder="A minimal starter theme">
  </div>
  <div class="form-group">
    <label for="author">Author</label>
    <input type="text" id="author" name="author" maxlength="100" placeholder="Your name">
  </div>
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Create Theme</button>
    <a href="/admin/appearance" class="btn btn-secondary">Cancel</a>
  </div>
</form>"#;

    admin_page("Create Theme", "/admin/appearance", flash, content, ctx)
}

pub fn render(themes: &[ThemeInfo], ctx: &crate::PageContext) -> String {
    render_with_flash(themes, None, ctx)
}

// ── Theme file editor ─────────────────────────────────────────────────────────

pub struct EditorFile {
    pub rel_path: String,
    pub is_selected: bool,
    pub has_backup: bool,
    /// Formatted last-modified time, only populated when `has_backup` is true.
    pub edited_at: Option<String>,
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

    let new_file_form = if ctx.can_manage_appearance {
        format!(
            r#"<button type="button" class="btn btn-sm btn-secondary"
        onclick="document.getElementById('new-file-form').style.display='flex'">+ New file</button>
<div id="new-file-form" style="display:none;align-items:center;gap:.5rem;flex-wrap:wrap;margin-top:.5rem;">
  <form method="POST" action="/admin/appearance/editor/{theme}/new-file"
        style="display:contents">
    <input type="text" name="filename" placeholder="e.g. partials/header or custom"
           required style="flex:1;min-width:180px;">
    <select name="ext" style="width:auto;">
      <option value=".html">.html</option>
      <option value=".css">.css</option>
      <option value=".js">.js</option>
    </select>
    <button type="submit" class="btn btn-sm btn-primary">Create</button>
    <button type="button" class="btn btn-sm btn-secondary"
            onclick="document.getElementById('new-file-form').style.display='none'">Cancel</button>
  </form>
</div>"#,
            theme = theme_esc,
        )
    } else {
        String::new()
    };

    // Top toolbar — always visible
    let toolbar = format!(
        r#"<div class="editor-topbar">
  <a href="/admin/appearance" class="btn btn-sm btn-secondary">&#8592; Themes</a>
  {picker}
  {new_file_form}
</div>"#,
        picker = file_picker,
        new_file_form = new_file_form,
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

        let is_required = matches!(rel,
            "templates/base.html" | "templates/index.html" | "templates/single.html" |
            "templates/page.html" | "templates/archive.html" | "templates/search.html" |
            "templates/404.html"
        );
        let delete_btn = if !is_required {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/delete-file" style="display:contents"
     onsubmit="return confirm('Delete {file_js}? This cannot be undone.')">
  <input type="hidden" name="file" value="{file}">
  <button type="submit" class="btn btn-sm btn-danger">Delete file</button>
</form>"#,
                theme    = theme_esc,
                file     = rel_esc,
                file_js  = rel_esc,
            )
        } else {
            String::new()
        };

        let del_btn = delete_btn.clone();
        let del_btn2 = delete_btn;
        let edited_at = if has_backup {
            files.iter()
                .find(|f| f.rel_path == rel)
                .and_then(|f| f.edited_at.as_deref())
                .map(|d| format!(r#" <span class="editor-edited-at">Edited: {d}</span>"#))
                .unwrap_or_default()
        } else {
            String::new()
        };
        format!(
            r#"<div class="editor-meta">
  <span class="editor-filename">{file}</span>{edited_at}
  {restore}
  {del_btn}
</div>
<form method="POST" action="/admin/appearance/editor/{theme}/save" class="editor-form" id="save-form">
  <input type="hidden" name="file" value="{file}">
  <textarea name="content" class="editor-textarea" spellcheck="false" autocorrect="off" autocapitalize="off">{content}</textarea>
</form>
<div class="editor-actions">
  <button type="submit" form="save-form" class="btn btn-primary">Save file</button>
  {restore2}
  {del_btn2}
</div>
<div class="editor-comment-hint">
  <strong>Tera comments:</strong> <code>&#123;# comment #&#125;</code> — use inside <code>&#123;% block %&#125;</code> tags only.
  <code>&#123;% extends %&#125;</code> must be the very first line of the file — nothing (not even a comment) may appear before it.
  CSS/HTML comments (<code>&lt;!-- --&gt;</code>, <code>/* */</code>) outside of blocks will also break parsing.
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
