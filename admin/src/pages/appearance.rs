use crate::admin_page;

pub struct ThemeInfo {
    /// Folder name — used as the key for all operations (URLs, forms, DB, screenshots).
    /// Always matches the on-disk directory name exactly.
    pub name: String,
    /// Human-readable display name from theme.toml `name` field.
    /// May differ in capitalisation or spacing from the folder name.
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub active: bool,
    pub has_screenshot: bool,
    /// Origin of this theme: `"global"`, `"private"` (super_admin only), or `"site"`.
    pub source: String,
    /// Whether the current user is permitted to delete this theme.
    /// Computed server-side; never shown for active themes.
    pub can_delete: bool,
    /// Number of sites currently using this theme (global themes only).
    pub in_use_by: usize,
    /// True when a site copy of this global theme already exists in the site's theme folder.
    /// Only meaningful in the global filter view; always false for site themes.
    pub has_site_copy: bool,
    /// True when this theme originated from themes/private/ (even if now in a site folder).
    /// Used to keep the Private badge visible on site copies of private themes.
    pub is_private_origin: bool,
}

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>, ctx: &crate::PageContext, filter: &str) -> String {
    let cards: String = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found.</p>
        </div>"#.to_string()
    } else {
        themes.iter().map(|t| render_card(t, ctx, filter)).collect()
    };

    let sel_my      = if filter != "global" && filter != "private" { " selected" } else { "" };
    let sel_global  = if filter == "global"  { " selected" } else { "" };
    let sel_private = if filter == "private" { " selected" } else { "" };

    let toolbar = if ctx.can_manage_appearance {
        // Super admins get a three-option dropdown (My Themes = global+private, Global, Private).
        // Site admins get the two-option dropdown (My Themes, Global Themes).
        let filter_options = if ctx.is_global_admin {
            format!(
                r#"<option value="my"{sel_my}>My Themes</option>
      <option value="global"{sel_global}>Global Themes</option>
      <option value="private"{sel_private}>Private Themes</option>"#,
                sel_my = sel_my,
                sel_global = sel_global,
                sel_private = sel_private,
            )
        } else {
            format!(
                r#"<option value="my"{sel_my}>My Themes</option>
      <option value="global"{sel_global}>Global Themes</option>"#,
                sel_my = sel_my,
                sel_global = sel_global,
            )
        };
        format!(
            r#"<div class="appearance-toolbar">
  <form method="GET" action="/admin/appearance" style="display:contents">
    <select name="filter" class="appearance-filter-select" onchange="this.form.submit()" aria-label="Theme filter">
      {filter_options}
    </select>
  </form>
  <a href="/admin/appearance/create" class="btn btn-primary">+ Create Theme</a>
</div>"#,
            filter_options = filter_options,
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{toolbar}<div class="theme-list">{cards}</div>
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
    // Visibility radio is only shown to super_admin.
    let visibility_section = if ctx.is_global_admin {
        r#"<div class="form-group">
    <label>Visibility</label>
    <div class="radio-group">
      <label class="radio-label">
        <input type="radio" name="visibility" value="private" checked>
        <span>
          <strong>Private</strong> — only you can see, edit, and assign this theme.
          It will not appear in any site admin's theme library.
        </span>
      </label>
      <label class="radio-label">
        <input type="radio" name="visibility" value="public">
        <span>
          <strong>Public</strong> — listed in the global theme library.
          Any site admin can get a copy.
        </span>
      </label>
    </div>
  </div>"#
    } else {
        ""
    };

    let content = format!(
        r#"<form method="POST" action="/admin/appearance/create" class="form-section" style="max-width:520px;">
  <div class="form-group">
    <label for="name">Theme name <span class="required">*</span></label>
    <input type="text" id="name" name="name" required maxlength="64"
           placeholder="my-theme" pattern="[^/\\\.][^/\\]*"
           title="No slashes, backslashes, or leading dots. Max 64 characters.">
    <p class="muted">Used as the folder name. Letters, numbers, hyphens, and underscores only.</p>
  </div>
  <div class="form-group">
    <label for="description">Description — 30 chars max</label>
    <input type="text" id="description" name="description" maxlength="30" placeholder="A minimal starter theme">
  </div>
  <div class="form-group">
    <label for="author">Author</label>
    <input type="text" id="author" name="author" maxlength="100" placeholder="Your name">
  </div>
  {visibility}
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Create Theme</button>
    <a href="/admin/appearance" class="btn btn-secondary">Cancel</a>
  </div>
</form>"#,
        visibility = visibility_section,
    );

    admin_page("Create Theme", "/admin/appearance", flash, &content, ctx)
}

pub fn render(themes: &[ThemeInfo], ctx: &crate::PageContext) -> String {
    render_with_flash(themes, None, ctx, "my")
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
    is_readonly: bool,
    // Which directory this theme lives in: "site", "global", or "private".
    // Threaded through every form so saves always target the correct copy.
    source: &str,
) -> String {
    let theme_esc = crate::html_escape(theme_name);
    let source_esc = crate::html_escape(source);

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
  <input type="hidden" name="source" value="{source}">
  <select name="file" class="editor-file-select" onchange="this.form.submit()"
          aria-label="Select theme file" title="Navigate to file">
    {options}
  </select>
</form>"#,
        theme = theme_esc,
        source = source_esc,
        options = options,
    );

    let new_file_form = if ctx.can_manage_appearance && !is_readonly {
        format!(
            r#"<button type="button" class="btn btn-sm btn-secondary"
        onclick="document.getElementById('new-file-form').style.display='flex'">+ New file</button>
<div id="new-file-form" style="display:none;align-items:center;gap:.5rem;flex-wrap:wrap;margin-top:.5rem;">
  <form method="POST" action="/admin/appearance/editor/{theme}/new-file"
        style="display:contents">
    <input type="hidden" name="source" value="{source}">
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
            source = source_esc,
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

    // Read-only notice shown when a site admin views a global theme.
    let readonly_notice = if is_readonly {
        r#"<div class="editor-notice editor-notice--warning">
  <strong>Global theme — read only.</strong>
  This is a shared global theme. Activate it to get your own editable copy.
</div>"#.to_string()
    } else {
        String::new()
    };

    // Editor body — shown only when a file is selected
    let body = if let Some(rel) = selected {
        let rel_esc  = crate::html_escape(rel);
        let content_esc = crate::html_escape(content);

        let restore_btn = if has_backup {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/restore" style="display:contents"
     onsubmit="return confirm('Restore the original backup? Your current edits will be overwritten.')">
  <input type="hidden" name="file" value="{file}">
  <input type="hidden" name="source" value="{source}">
  <button type="submit" class="btn btn-sm btn-secondary">Restore original</button>
</form>"#,
                theme  = theme_esc,
                file   = rel_esc,
                source = source_esc,
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
  <input type="hidden" name="source" value="{source}">
  <button type="submit" class="btn btn-sm btn-danger">Delete file</button>
</form>"#,
                theme    = theme_esc,
                file     = rel_esc,
                file_js  = rel_esc,
                source   = source_esc,
            )
        } else {
            String::new()
        };

        // In readonly mode suppress all write actions.
        let (restore_btn, delete_btn) = if is_readonly {
            (String::new(), String::new())
        } else {
            (restore_btn, delete_btn)
        };

        let del_btn = delete_btn.clone();
        let del_btn2 = delete_btn;
        let ro = if is_readonly { " readonly" } else { "" };
        let save_btn = if is_readonly { "" } else { r#"<button type="submit" form="save-form" class="btn btn-primary">Save file</button>"# };
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
  <input type="hidden" name="source" value="{source}">
  <textarea name="content" class="editor-textarea" spellcheck="false" autocorrect="off" autocapitalize="off"{ro}>{content}</textarea>
</form>
<div class="editor-actions">
  {save_btn}
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
            source   = source_esc,
            restore  = restore_btn.clone(),
            restore2 = restore_btn,
        )
    } else {
        r#"<div class="editor-hint">Select a file above to start editing.</div>"#.to_string()
    };

    let content_html = format!(
        r#"<div class="editor-wrap">{toolbar}{readonly_notice}{body}</div>"#,
        toolbar         = toolbar,
        readonly_notice = readonly_notice,
        body            = body,
    );

    admin_page(
        &format!("Edit Theme: {}", crate::html_escape(theme_name)),
        "/admin/appearance",
        flash,
        &content_html,
        ctx,
    )
}

fn render_card(t: &ThemeInfo, ctx: &crate::PageContext, filter: &str) -> String {
    // name_esc  — folder name, used for all functional references (URLs, forms, DB)
    // label_esc — display name from theme.toml, used only for visible text
    let name_esc  = crate::html_escape(&t.name);
    let label_esc = crate::html_escape(&t.display_name);

    let screenshot_html = if t.has_screenshot {
        format!(
            r#"<div class="theme-screenshot"><img src="/admin/theme-screenshot/{name}" alt="{label} preview"></div>"#,
            name  = name_esc,
            label = label_esc,
        )
    } else {
        format!(
            r#"<div class="theme-screenshot theme-screenshot-placeholder"><span>{label}</span></div>"#,
            label = label_esc,
        )
    };

    // ── Theme card badges ─────────────────────────────────────────────────────
    // All metadata badges live here in the header, right of the version badge.
    // Keep them together: [version] [Private] [site count] [any future badge].
    // Do NOT scatter new badges elsewhere in the card.
    let private_badge = if t.source == "private" || t.is_private_origin {
        r#"<span class="badge badge-private" title="Originated from a private theme">Private</span>"#
    } else {
        ""
    };

    let in_use_badge = if ctx.is_global_admin && t.source == "global" && t.in_use_by > 0 {
        format!(
            r#"<span class="badge" title="Active on {} site(s) — cannot delete">{}</span>"#,
            t.in_use_by, t.in_use_by,
        )
    } else {
        String::new()
    };

    let header = format!(
        r#"<div class="theme-card-header">
    <span class="theme-name">{label}</span>
    <span class="badge">{version}</span>{private_badge}{in_use_badge}
  </div>
  <p class="theme-description">{desc}</p>
  <p class="theme-author">by {author}</p>"#,
        label        = label_esc,
        version      = crate::html_escape(&t.version),
        private_badge = private_badge,
        in_use_badge  = in_use_badge,
        desc         = crate::html_escape(&t.description),
        author       = crate::html_escape(&t.author),
    );

    // ── Global / Private library views ───────────────────────────────────────
    // In these views all users see "Get Theme" to copy to their site folder
    // without activating. Private tab also shows an Edit button so super_admin
    // can edit the private original directly without getting a site copy first.
    if filter == "global" || filter == "private" {
        let source_val = crate::html_escape(&t.source);
        let get_html = if t.has_site_copy {
            r#"<span class="badge badge-in-use">In My Themes</span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/admin/appearance/get-theme" style="display:inline;">
    <input type="hidden" name="theme" value="{name}">
    <input type="hidden" name="source" value="{source}">
    <button type="submit" class="btn btn-primary">Get Theme</button>
</form>"#,
                name   = name_esc,
                source = source_val,
            )
        };

        // Private themes: super_admin can edit the private original directly.
        let edit_html = if filter == "private" {
            format!(
                r#"<a href="/admin/appearance/editor/{name}?source=private" class="btn btn-edit">Edit</a>"#,
                name = name_esc,
            )
        } else {
            String::new()
        };

        return format!(
            r#"<div class="theme-card">
  {screenshot}
  {header}
  <div class="theme-actions">{get}{edit}</div>
</div>"#,
            screenshot = screenshot_html,
            header     = header,
            get        = get_html,
            edit       = edit_html,
        );
    }

    // ── My Themes view (and super admin everywhere) ───────────────────────────
    let active_class = if t.active { " active" } else { "" };

    let activate_html = if t.active {
        String::new()
    } else {
        format!(
            r#"<form method="post" action="/admin/appearance/activate" style="display:inline;">
    <input type="hidden" name="theme" value="{name}">
    <button type="submit" class="btn btn-primary">Activate</button>
</form>"#,
            name = name_esc,
        )
    };

    let edit_html = format!(
        r#"<a href="/admin/appearance/editor/{name}?source={source}" class="btn btn-edit">Edit</a>"#,
        name   = name_esc,
        source = crate::html_escape(&t.source),
    );

    let delete_html = if t.can_delete {
        // Site themes use "Remove" language — the user can get a fresh copy from
        // Global Themes any time. Global/private themes are permanently deleted.
        let (btn_label, confirm_msg) = if t.source == "site" {
            (
                "Remove",
                format!(
                    "Remove &quot;{name}&quot; from My Themes?\n\nYour local copy and any edits will be deleted. You can get a fresh copy from Global Themes at any time.",
                    name = name_esc,
                ),
            )
        } else {
            (
                "Delete",
                format!("Permanently delete theme &quot;{name}&quot;? This cannot be undone.", name = name_esc),
            )
        };
        format!(
            r#"<form method="post" action="/admin/appearance/delete" style="display:inline;"
                data-confirm="{confirm}" onsubmit="return confirm(this.dataset.confirm)">
    <input type="hidden" name="theme" value="{name}">
    <input type="hidden" name="source" value="{source}">
    <button type="submit" class="btn btn-danger">{label}</button>
</form>"#,
            confirm = confirm_msg,
            name    = name_esc,
            source  = crate::html_escape(&t.source),
            label   = btn_label,
        )
    } else {
        String::new()
    };

    format!(
        r#"<div class="theme-card{active}">
  {screenshot}
  {header}
  <div class="theme-actions">
    {activate}{edit}{delete}
  </div>
</div>"#,
        active     = active_class,
        screenshot = screenshot_html,
        header     = header,
        activate   = activate_html,
        edit       = edit_html,
        delete     = delete_html,
    )
}
