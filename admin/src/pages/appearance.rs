use crate::admin_page;

pub struct ThemeInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub active: bool,
    pub has_screenshot: bool,
}

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>) -> String {
    let cards: String = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found. Add a theme directory to <code>themes/</code> and restart the server.</p>
        </div>"#.to_string()
    } else {
        themes.iter().map(render_card).collect()
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

    admin_page("Appearance", "/admin/appearance", flash, &content)
}

pub fn render(themes: &[ThemeInfo]) -> String {
    render_with_flash(themes, None)
}

fn render_card(t: &ThemeInfo) -> String {
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

    let button_html = if t.active {
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

    format!(
        r#"<div class="theme-card{active}">
  {screenshot}
  <div class="theme-card-header">
    <span class="theme-name">{name}</span>
    <span class="badge">{version}</span>
  </div>
  <p class="theme-description">{desc}</p>
  <p class="theme-author">by {author}</p>
  <div class="theme-actions">
    {button}
  </div>
</div>"#,
        active = active_class,
        screenshot = screenshot_html,
        name = crate::html_escape(&t.name),
        version = crate::html_escape(&t.version),
        desc = crate::html_escape(&t.description),
        author = crate::html_escape(&t.author),
        button = button_html,
    )
}
