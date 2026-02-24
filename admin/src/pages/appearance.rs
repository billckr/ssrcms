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

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>, current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str, can_manage_users: bool) -> String {
    let cards: String = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found. Add a theme directory to <code>themes/</code> and restart the server.</p>
        </div>"#.to_string()
    } else {
        themes.iter().map(|t| render_card(t, is_global_admin)).collect()
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

    admin_page("Appearance", "/admin/appearance", flash, &content, current_site, is_global_admin, visiting_foreign_site, user_email, can_manage_users)
}

pub fn render(themes: &[ThemeInfo], current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str, can_manage_users: bool) -> String {
    render_with_flash(themes, None, current_site, is_global_admin, visiting_foreign_site, user_email, can_manage_users)
}

fn render_card(t: &ThemeInfo, is_global_admin: bool) -> String {
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

    let delete_html = if t.can_delete {
        format!(
            r#"<form method="post" action="/admin/appearance/delete" style="display:inline;"
    onsubmit="return confirm('Delete theme \'{name}\'? This cannot be undone.');">
    <input type="hidden" name="theme" value="{name_escaped}">
    <button type="submit" class="btn btn-danger">Delete</button>
</form>"#,
            name = t.name.replace('\'', "\\'"),
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
    {activate}{delete}
  </div>
</div>"#,
        active = active_class,
        screenshot = screenshot_html,
        name = crate::html_escape(&t.name),
        version = crate::html_escape(&t.version),
        source_badge = {
            let source_label = if t.source == "global" { "global" } else { "site" };
            let in_use_badge = if is_global_admin && t.source == "global" && t.in_use_by > 0 {
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
        delete = delete_html,
    )
}
