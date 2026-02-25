pub mod components;
pub mod pages;

/// The admin CSS, inlined into every page.
const ADMIN_CSS: &str = include_str!("../style/admin.css");

/// Context passed to every admin page shell and render function.
/// Built once per handler from `AdminUser`; passed by reference — never recomputed.
#[derive(Debug, Clone)]
pub struct PageContext {
    pub current_site: String,
    pub user_email: String,
    /// Agency-level super-admin with unrestricted cross-site access.
    pub is_global_admin: bool,
    /// Super-admin viewing a site they do not own.
    pub visiting_foreign_site: bool,
    /// Can view, create, edit, and delete users.
    pub can_manage_users: bool,
    /// Can create new sites and edit site-level settings.
    pub can_manage_sites: bool,
    /// Can activate, configure, and remove plugins.
    pub can_manage_plugins: bool,
    /// Can edit site settings (name, description, etc.).
    pub can_manage_settings: bool,
    /// Can create, edit, publish, and delete content.
    pub can_manage_content: bool,
    /// Can manage themes (appearance).
    pub can_manage_appearance: bool,
}

/// Wrap a rendered content HTML string in the full admin page shell.
/// The sidebar nav, head, and body wrapper are all here.
pub fn admin_page(title: &str, current_path: &str, flash: Option<&str>, content: &str, ctx: &PageContext) -> String {
    let visiting_badge = if ctx.visiting_foreign_site && !ctx.current_site.is_empty() {
        format!(
            r#"<span class="badge-visiting">Super Admin → {}</span>"#,
            html_escape(&ctx.current_site)
        )
    } else {
        String::new()
    };
    let site_indicator = if ctx.current_site.is_empty() {
        String::new()
    } else {
        format!(
            r#"<a href="/admin/sites" class="site-indicator">{}</a>"#,
            html_escape(&ctx.current_site)
        )
    };
    let flash_html = match flash {
        Some(msg) => {
            // Detect error messages by looking for error indicators
            let is_error = msg.starts_with("Error") 
                || msg.contains("error") 
                || msg.contains("does not") 
                || msg.contains("incorrect")
                || msg.contains("must")
                || msg.contains("cannot")
                || msg.contains("invalid")
                || msg.contains("failed")
                || msg.contains("Failed");
            let class = if is_error { "error" } else { "success" };
            format!(r#"<div class="flash {}">{}</div>"#, class, html_escape(msg))
        }
        None => String::new(),
    };

    let nav_link = |href: &str, label: &str| -> String {
        let active = if current_path.starts_with(href) && href != "/admin" {
            " class=\"active\""
        } else if current_path == href {
            " class=\"active\""
        } else {
            ""
        };
        format!(r#"<li><a href="{}"{}>{}</a></li>"#, href, active, label)
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — Synaptic Admin</title>
  <style>{css}</style>
</head>
<body>
  <div class="sidebar-overlay" onclick="closeSidebar()"></div>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <div class="brand">Synaptic</div>
      <ul>
        {dash}
        {posts}
        {pages}
        {media}
        {cats}
        {tags}
        {users}
        {plugins}
        {appearance}
        {settings}
        {sites}
      </ul>
      <div class="sidebar-footer">
        <a href="/admin/profile">{user_email}</a>
        <a href="/admin/logout">Log out</a>
      </div>
    </nav>
    <main class="admin-main">
      <header class="admin-header">
        <button class="hamburger" onclick="toggleSidebar()" aria-label="Open navigation">
          <span></span><span></span><span></span>
        </button>
        <h1>{title}</h1>
        {visiting_badge}
        {site_indicator}
      </header>
      {flash_html}
      <div class="admin-content">
        {content}
      </div>
    </main>
  </div>
  <script>
    function toggleSidebar() {{
      document.body.classList.toggle('sidebar-open');
    }}
    function closeSidebar() {{
      document.body.classList.remove('sidebar-open');
    }}
    document.querySelectorAll('.admin-sidebar a').forEach(function(a) {{
      a.addEventListener('click', closeSidebar);
    }});
  </script>
</body>
</html>"#,
        title = html_escape(title),
        css = ADMIN_CSS,
        dash = nav_link("/admin", "Dashboard"),
        posts = nav_link("/admin/posts", "Posts"),
        pages = nav_link("/admin/pages", "Pages"),
        media = nav_link("/admin/media", "Media"),
        cats = nav_link("/admin/categories", "Categories"),
        tags = nav_link("/admin/tags", "Tags"),
        users = if ctx.can_manage_users { nav_link("/admin/users", "Users") } else { String::new() },
        plugins = nav_link("/admin/plugins", "Plugins"),
        appearance = nav_link("/admin/appearance", "Appearance"),
        settings = nav_link("/admin/settings", "Settings"),
        sites = nav_link("/admin/sites", "Sites"),
        flash_html = flash_html,
        content = content,
        visiting_badge = visiting_badge,
        site_indicator = site_indicator,
        user_email = html_escape(&ctx.user_email),
    )
}

/// Minimal HTML escaping for values inserted into HTML attributes or text.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#x27;")
}
