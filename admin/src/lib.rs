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
    pub user_role: String,
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
    /// Can create, edit, and delete categories and tags.
    pub can_manage_taxonomies: bool,
    /// Can view, export, and delete form submissions.
    pub can_manage_forms: bool,
    /// Can create, edit, and delete pages (not available to the author role).
    pub can_manage_pages: bool,
    /// Number of unread form submissions across all forms on this site.
    pub unread_forms_count: i64,
    /// Number of posts in "pending review" state on this site (shown as a sidebar badge).
    /// For editors/admins: all pending posts on the site. For authors: their own pending posts.
    pub pending_review_count: i64,
    /// Admin chrome brand label — from app_settings.app_name.
    pub app_name: String,
}

/// Wrap a rendered content HTML string in the full admin page shell.
/// The sidebar nav, head, and body wrapper are all here.
pub fn admin_page(title: &str, current_path: &str, flash: Option<&str>, content: &str, ctx: &PageContext) -> String {
    let visiting_badge = if ctx.visiting_foreign_site && !ctx.current_site.is_empty() {
        let site = html_escape(&ctx.current_site);
        format!(
            r#"<a href="/admin/sites/go-home" class="badge-visiting" title="Return to your admin panel"><svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path><polyline points="16 17 21 12 16 7"></polyline><line x1="21" y1="12" x2="9" y2="12"></line></svg>Super Admin &rarr; {site}</a>"#
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

    let media_nav = "<li><a href=\"#\" onclick=\"openMediaBrowser();return false;\">Media</a></li>".to_string();

    let media_browser_modal = r#"<div id="media-browser-modal" class="mpicker-overlay" style="display:none" onclick="if(event.target===this)closeMediaBrowser()">
  <div class="mpicker-dialog" style="display:flex;flex-direction:column">
    <div class="mpicker-header">
      <span class="mpicker-title">Media Library</span>
      <button type="button" class="btn btn-primary" style="font-size:13px;padding:.3rem .85rem" onclick="closeMediaBrowser()">Close</button>
    </div>
    <iframe id="media-browser-frame" src="about:blank" style="flex:1;width:100%;border:none;display:block;min-height:0"></iframe>
  </div>
</div>"#;

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
      <div class="brand">{app_name}</div>
      <ul>
        {dash}
        {posts}
        {pages}
        {media}
        {menus}
        {cats}
        {tags}
        {forms}
        {users}
        {sites}
        {plugins}
        {documentation}
        {appearance}
        {settings}
      </ul>
      <div class="sidebar-footer">
        <a href="{profile_or_home}">{user_email}</a>
        <span class="sidebar-user-role">{user_role}</span>
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
  {media_browser_modal}
  <script>
    function toggleSidebar() {{
      document.body.classList.toggle('sidebar-open');
    }}
    function closeSidebar() {{
      document.body.classList.remove('sidebar-open');
    }}
    document.querySelectorAll('.admin-sidebar a').forEach(function(a) {{
      a.addEventListener('click', function(e) {{
        if (a.getAttribute('href') !== '#') closeSidebar();
      }});
    }});
    function openMediaBrowser() {{
      var frame = document.getElementById('media-browser-frame');
      if (frame.getAttribute('data-loaded') !== '1') {{
        frame.src = '/admin/media?browser=1';
        frame.setAttribute('data-loaded', '1');
      }}
      document.getElementById('media-browser-modal').style.display = '';
    }}
    function closeMediaBrowser() {{
      document.getElementById('media-browser-modal').style.display = 'none';
      var frame = document.getElementById('media-browser-frame');
      frame.src = 'about:blank';
      frame.removeAttribute('data-loaded');
    }}
  </script>
</body>
</html>"#,
        title = html_escape(title),
        css = ADMIN_CSS,
        app_name = html_escape(&ctx.app_name),
        dash = nav_link("/admin", "Dashboard"),
        posts = {
            let pending_badge = if ctx.pending_review_count > 0 {
                format!(
                    r#" <span class="badge-unread" style="margin-left:.4rem;font-size:10px;padding:.1rem .45rem;box-shadow:none;background:#fef3c7;color:#92400e;border:1px solid #fcd34d;border-radius:3px;animation:none">{}</span>"#,
                    ctx.pending_review_count
                )
            } else {
                String::new()
            };
            let active = if current_path.starts_with("/admin/posts") { " class=\"active\"" } else { "" };
            format!(r#"<li><a href="/admin/posts"{}>{}</a></li>"#, active, format!("Posts{}", pending_badge))
        },
        pages = if ctx.can_manage_pages { nav_link("/admin/pages", "Pages") } else { String::new() },
        media = media_nav,
        cats = if ctx.can_manage_taxonomies { nav_link("/admin/categories", "Categories") } else { String::new() },
        tags = if ctx.can_manage_taxonomies { nav_link("/admin/tags", "Tags") } else { String::new() },
        users = if ctx.can_manage_users { nav_link("/admin/users", "Users") } else { String::new() },
        sites = nav_link("/admin/sites", "Sites"),
        forms = if ctx.can_manage_forms {
            let badge = if ctx.unread_forms_count > 0 {
                format!(
                    r#" <span class="badge-unread" style="margin-left:.4rem;font-size:10px;padding:.1rem .45rem;box-shadow:none">{}</span>"#,
                    ctx.unread_forms_count
                )
            } else {
                String::new()
            };
            let active = if current_path.starts_with("/admin/forms") { " class=\"active\"" } else { "" };
            format!(r#"<li><a href="/admin/forms"{}>{}</a></li>"#,
                active,
                format!("Forms{}", badge)
            )
        } else { String::new() },
        plugins = String::new(), // plugins disabled pre-launch
        documentation = if ctx.is_global_admin { nav_link("/admin/documentation", "Documentation") } else { String::new() },
        appearance = if ctx.can_manage_appearance { nav_link("/admin/appearance", "Appearance") } else { String::new() },
        menus = if ctx.can_manage_appearance { nav_link("/admin/menus", "Menus") } else { String::new() },
        settings = if ctx.can_manage_settings { nav_link("/admin/settings", "System Settings") } else { String::new() },
        flash_html = flash_html,
        content = content,
        visiting_badge = visiting_badge,
        site_indicator = site_indicator,
        profile_or_home = if ctx.visiting_foreign_site { "/admin/sites/go-home?next=/admin/profile" } else { "/admin/profile" },
        user_email = html_escape(&ctx.user_email),
        user_role  = html_escape(&ctx.user_role),
        media_browser_modal = media_browser_modal,
    )
}

/// Minimal HTML escaping for values inserted into HTML attributes or text.
/// Minimal HTML shell for the media picker iframe (no admin sidebar/header).
/// Used when `/admin/media?picker=1` is requested.
pub fn picker_page(content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Media Library</title>
  <style>{css}</style>
  <style>
    html, body {{ height: 100%; margin: 0; overflow: hidden; background: var(--surface); }}
    .mm-layout {{ height: 100vh !important; border: none !important; border-radius: 0 !important; box-shadow: none !important; }}
  </style>
</head>
<body>
  {content}
</body>
</html>"#,
        css     = ADMIN_CSS,
        content = content,
    )
}

/// Returns the shared media-picker modal HTML + JS.
/// Opens the full media manager in an iframe; selection is returned via postMessage.
/// Supports pickerMode: 'featured' (set featured image), 'inline' (Quill image insert),
/// and 'audio' (Quill audio insert).
pub fn media_picker_modal_html() -> String {
    String::from(r#"<div id="media-picker-modal" class="mpicker-overlay" style="display:none" onclick="if(event.target===this)closeMediaPicker()">
  <div class="mpicker-dialog" style="display:flex;flex-direction:column">
    <div class="mpicker-header">
      <span class="mpicker-title">Media Library</span>
      <button type="button" class="btn btn-primary" style="font-size:13px;padding:.3rem .85rem" onclick="closeMediaPicker()">Close</button>
    </div>
    <iframe id="media-picker-frame" src="about:blank" style="flex:1;width:100%;border:none;display:block;min-height:0"></iframe>
  </div>
</div>
<script>
(function() {
  var pickerMode = 'featured'; // 'featured', 'inline', or 'audio'

  function escHtml(s) {
    return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }

  window.openMediaPicker = function(mode) {
    pickerMode = mode || 'featured';
    var frame = document.getElementById('media-picker-frame');
    // Always reload so the correct type filter and fresh state is applied.
    var src = '/admin/media?picker=1';
    if (pickerMode === 'audio') src += '&type=audio';
    frame.src = src;
    frame.setAttribute('data-loaded', '1');
    // After the iframe loads, push the localised button label into it.
    var label = pickerMode === 'audio' ? 'Insert Audio' : 'Set Image';
    frame.addEventListener('load', function onLoad() {
      frame.removeEventListener('load', onLoad);
      try { frame.contentWindow.postMessage({ type: 'pickerSetLabel', label: label }, '*'); } catch(e) {}
    });
    document.getElementById('media-picker-modal').style.display = '';
  };

  window.closeMediaPicker = function() {
    document.getElementById('media-picker-modal').style.display = 'none';
    var frame = document.getElementById('media-picker-frame');
    frame.src = 'about:blank';
    frame.removeAttribute('data-loaded');
  };

  // Receive the selected media back from the picker iframe.
  window.addEventListener('message', function(e) {
    if (!e.data || e.data.type !== 'featuredImageSelected') return;
    var id   = e.data.id   || '';
    var path = e.data.path || '';
    var alt  = e.data.alt  || '';
    if (pickerMode === 'inline') {
      var q = window._quillInstance;
      if (q) {
        var range = window._quillRange || q.getSelection(true);
        var imgHtml = '<img src="' + path + '" alt="' + alt.replace(/"/g, '&quot;') + '">';
        q.clipboard.dangerouslyPasteHTML(range.index, imgHtml, 'user');
        q.setSelection(range.index + 1, 0, 'silent');
        if (window.refreshInlineMediaList) window.refreshInlineMediaList();
      }
    } else if (pickerMode === 'audio') {
      var q = window._quillInstance;
      if (q) {
        var range = window._quillRange || q.getSelection(true) || {index: q.getLength(), length: 0};
        // insertEmbed uses the registered AudioBlot so Quill preserves
        // the <audio controls> element instead of stripping it.
        q.insertEmbed(range.index, 'audio', path, 'user');
        q.setSelection(range.index + 1, 0, 'silent');
        if (window.refreshInlineMediaList) window.refreshInlineMediaList();
      }
    } else {
      var fidEl  = document.getElementById('featured_image_id');
      var furlEl = document.getElementById('featured_image_url_field');
      if (fidEl)  fidEl.value  = id;
      if (furlEl) furlEl.value = path;
      var box = document.getElementById('featured-image-box');
      if (box) {
        box.innerHTML = '<img src="' + escHtml(path) + '" alt="Featured image" style="width:100%;height:100%;object-fit:cover;display:block">';
        box.classList.add('has-image');
      }
      var rb = document.getElementById('fi-remove-btn');
      if (rb) rb.style.display = '';
    }
    closeMediaPicker();
  });

  window.removeFeaturedImage = function() {
    var fidEl  = document.getElementById('featured_image_id');
    var furlEl = document.getElementById('featured_image_url_field');
    if (fidEl)  fidEl.value  = '';
    if (furlEl) furlEl.value = '';
    var box = document.getElementById('featured-image-box');
    if (box) {
      box.classList.remove('has-image');
      box.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" style="opacity:.35"><rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10.5" r="1.5"/><path d="M3 16l4.5-4.5 3 3 2.5-2.5 5 5"/></svg><span style="color:var(--muted);font-size:12px">No image selected</span>';
    }
    var rb = document.getElementById('fi-remove-btn');
    if (rb) rb.style.display = 'none';
  };
})();
</script>"#)
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#x27;")
}

/// Generate the live-search `<script>` block used by list pages with a search input.
///
/// - `input_id`   — `id` of the search `<input>` element
/// - `list_id`    — `id` of the `<div>` whose `innerHTML` is replaced on each keystroke
/// - `url_prefix` — URL prefix to which `&search=<encoded-term>` is appended,
///                  e.g. `"/admin/posts?partial=1"` or `"/account/my-comments?partial=1"`
///
/// Debounces input at 300 ms; on each firing replaces `list_id` innerHTML with
/// the fetched HTML fragment. No JS framework or build pipeline dependency.
///
/// Migration note: when any consuming page is ported to Leptos/WASM, replace this
/// with a reactive signal + server function — the UX will be identical but fully in Rust.
pub fn live_search_script(input_id: &str, list_id: &str, url_prefix: &str) -> String {
    format!(
        r#"<script>
(function () {{
  var input = document.getElementById('{input_id}');
  var list  = document.getElementById('{list_id}');
  if (!input || !list) return;
  var timer;
  input.addEventListener('input', function () {{
    clearTimeout(timer);
    timer = setTimeout(function () {{
      var url = '{url_prefix}&search=' + encodeURIComponent(input.value);
      fetch(url)
        .then(function (r) {{ return r.text(); }})
        .then(function (html) {{ list.innerHTML = html; }})
        .catch(function () {{}});
    }}, 300);
  }});
}})();
</script>"#,
        input_id   = input_id,
        list_id    = list_id,
        url_prefix = url_prefix,
    )
}
