pub mod components;
pub mod pages;

/// The admin CSS, inlined into every page.
const ADMIN_CSS: &str = include_str!("../style/admin.css");

/// Wrap a rendered content HTML string in the full admin page shell.
/// The sidebar nav, head, and body wrapper are all here.
pub fn admin_page(title: &str, current_path: &str, flash: Option<&str>, content: &str) -> String {
    let flash_html = match flash {
        Some(msg) => format!(r#"<div class="flash success">{}</div>"#, html_escape(msg)),
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
        {settings}
      </ul>
      <div class="sidebar-footer">
        <a href="/admin/logout">Log out</a>
      </div>
    </nav>
    <main class="admin-main">
      <header class="admin-header">
        <button class="hamburger" onclick="toggleSidebar()" aria-label="Open navigation">
          <span></span><span></span><span></span>
        </button>
        <h1>{title}</h1>
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
        users = nav_link("/admin/users", "Users"),
        plugins = nav_link("/admin/plugins", "Plugins"),
        settings = nav_link("/admin/settings", "Settings"),
        flash_html = flash_html,
        content = content,
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
