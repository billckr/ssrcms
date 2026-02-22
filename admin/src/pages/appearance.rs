use crate::admin_page;

pub struct ThemeInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub active: bool,
}

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>) -> String {
    let content = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found. Add a theme directory to <code>themes/</code> and restart the server.</p>
        </div>"#.to_string()
    } else {
        let cards: String = themes.iter().map(|t| render_card(t)).collect();
        format!(r#"<div class="theme-list">{}</div>"#, cards)
    };

    admin_page("Appearance", "/admin/appearance", flash, &content)
}

pub fn render(themes: &[ThemeInfo]) -> String {
    render_with_flash(themes, None)
}

fn render_card(t: &ThemeInfo) -> String {
    let active_class = if t.active { " active" } else { "" };
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
        name = crate::html_escape(&t.name),
        version = crate::html_escape(&t.version),
        desc = crate::html_escape(&t.description),
        author = crate::html_escape(&t.author),
        button = button_html,
    )
}
