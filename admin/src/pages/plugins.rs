use crate::admin_page;

pub struct PluginRow {
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub author: String,
    pub description: String,
    pub hooks: Vec<(String, String)>,   // (hook_name, template_path)
    pub routes: Vec<String>,             // registered URL paths
    pub meta_fields: Vec<String>,        // declared meta field keys
}

pub fn render(plugins: &[PluginRow], current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str, can_manage_users: bool) -> String {
    let content = if plugins.is_empty() {
        r#"<div class="empty-state">
            <p>No plugins installed. Drop a plugin directory into <code>plugins/</code> and restart the server.</p>
        </div>"#.to_string()
    } else {
        let cards: String = plugins.iter().map(|p| render_card(p)).collect();
        format!(r#"<div class="plugin-list">{}</div>"#, cards)
    };

    admin_page("Plugins", "/admin/plugins", None, &content, current_site, is_global_admin, visiting_foreign_site, user_email, can_manage_users)
}

fn render_card(p: &PluginRow) -> String {
    let hooks_html = if p.hooks.is_empty() {
        "<span class=\"muted\">none</span>".to_string()
    } else {
        p.hooks.iter().map(|(hook, template)| {
            format!(
                r#"<li><code>{}</code> → <code>{}</code></li>"#,
                crate::html_escape(hook),
                crate::html_escape(template)
            )
        }).collect::<Vec<_>>().join("")
    };

    let routes_html = if p.routes.is_empty() {
        "<span class=\"muted\">none</span>".to_string()
    } else {
        p.routes.iter().map(|r| {
            format!(r#"<li><code>{}</code></li>"#, crate::html_escape(r))
        }).collect::<Vec<_>>().join("")
    };

    let meta_html = if p.meta_fields.is_empty() {
        "<span class=\"muted\">none</span>".to_string()
    } else {
        p.meta_fields.iter().map(|f| {
            format!(r#"<li><code>{}</code></li>"#, crate::html_escape(f))
        }).collect::<Vec<_>>().join("")
    };

    format!(
        r#"<div class="plugin-card">
  <div class="plugin-card-header">
    <span class="plugin-name">{name}</span>
    <span class="badge">{version}</span>
    <span class="badge badge-secondary">API {api}</span>
  </div>
  <p class="plugin-description">{desc}</p>
  <p class="plugin-author">by {author}</p>
  <div class="plugin-details">
    <div class="plugin-detail-group">
      <strong>Hooks</strong>
      <ul>{hooks}</ul>
    </div>
    <div class="plugin-detail-group">
      <strong>Routes</strong>
      <ul>{routes}</ul>
    </div>
    <div class="plugin-detail-group">
      <strong>Meta fields</strong>
      <ul>{meta}</ul>
    </div>
  </div>
</div>"#,
        name = crate::html_escape(&p.name),
        version = crate::html_escape(&p.version),
        api = crate::html_escape(&p.api_version),
        desc = crate::html_escape(&p.description),
        author = crate::html_escape(&p.author),
        hooks = hooks_html,
        routes = routes_html,
        meta = meta_html,
    )
}
