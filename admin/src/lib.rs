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
    pub is_impersonating: bool,
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
    /// Admin chrome brand label — from app_settings.app_name.
    pub app_name: String,
    /// Public URL of a custom admin sidebar logo, if one was found at startup.
    /// `None` falls back to rendering `app_name` as text.
    pub logo_url: Option<String>,
}

/// Wrap a rendered content HTML string in the full admin page shell.
/// The sidebar nav, head, and body wrapper are all here.
pub fn admin_page(title: &str, current_path: &str, flash: Option<&str>, content: &str, ctx: &PageContext) -> String {
    let visiting_badge = if ctx.is_impersonating && !ctx.current_site.is_empty() {
        let site = html_escape(&ctx.current_site);
        format!(
            r#"<a href="/admin/sites/go-home" class="badge-visiting" title="Return to your admin panel"><svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path><polyline points="16 17 21 12 16 7"></polyline><line x1="21" y1="12" x2="9" y2="12"></line></svg>super admin &rarr; {site}</a>"#
        )
    } else {
        String::new()
    };
    let site_indicator = if ctx.current_site.is_empty() || ctx.is_impersonating {
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
                || msg.contains("Failed")
                || msg.contains("already exists");
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

    let brand_html = match &ctx.logo_url {
        Some(url) => format!(
            r#"<img class="brand-logo" src="{}" alt="{}">"#,
            html_escape(url),
            html_escape(&ctx.app_name)
        ),
        None => html_escape(&ctx.app_name),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — Synaptic Admin</title>
  <script>
    // Applies the saved theme before first paint so there's no flash of the
    // wrong theme — must run synchronously in <head>, ahead of <style>.
    (function() {{
      try {{
        var pref = localStorage.getItem('admin-theme') || 'system';
        var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        if (dark) {{
          document.documentElement.setAttribute('data-theme', 'dark');
        }}
      }} catch (e) {{}}
    }})();
  </script>
  <style>{css}</style>
</head>
<body>
  <div class="sidebar-overlay" onclick="closeSidebar()"></div>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <a class="brand" href="/admin">{brand_html}</a>
      <ul>
        {dash}
        {sites}
        {users}
        {posts}
        {pages}
        {menus}
        {media}
        {cats}
        {tags}
        {form_designer}
        {plugins}
        {appearance}
        {forms}
        {builder}
        {documentation}
        {settings}
      </ul>
      <div class="sidebar-footer">
        <a href="{profile_or_home}" class="sidebar-user-email">{user_email}</a>
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
        <div class="header-menu">
          <button type="button" class="icon-btn" onclick="toggleHeaderMenu()" title="Menu" aria-label="Menu" aria-haspopup="true" aria-expanded="false" id="header-menu-btn">
            <img src="/admin/static/icons/list.svg" alt="">
          </button>
          <div class="header-menu-dropdown" id="header-menu-dropdown">
            <div class="theme-switch" role="group" aria-label="Theme" id="theme-switch">
              <button type="button" class="theme-switch-btn" data-theme-choice="light" onclick="setTheme('light')" title="Light mode" aria-label="Light mode">
                <img src="/admin/static/icons/sun.svg" alt="">
              </button>
              <button type="button" class="theme-switch-btn" data-theme-choice="system" onclick="setTheme('system')" title="Match system" aria-label="Match system">
                <img src="/admin/static/icons/monitor.svg" alt="">
              </button>
              <button type="button" class="theme-switch-btn" data-theme-choice="dark" onclick="setTheme('dark')" title="Dark mode" aria-label="Dark mode">
                <img src="/admin/static/icons/moon.svg" alt="">
              </button>
            </div>
            <div class="header-menu-divider"></div>
            <a href="/admin/logout" class="header-menu-item">
              <img src="/admin/static/icons/log-out.svg" alt="">
              <span>Log out</span>
            </a>
          </div>
        </div>
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
    function applyTheme(pref) {{
      var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
      if (dark) {{
        document.documentElement.setAttribute('data-theme', 'dark');
      }} else {{
        document.documentElement.removeAttribute('data-theme');
      }}
      var switchEl = document.getElementById('theme-switch');
      if (switchEl) {{
        var btns = switchEl.querySelectorAll('.theme-switch-btn');
        for (var i = 0; i < btns.length; i++) {{
          btns[i].classList.toggle('active', btns[i].getAttribute('data-theme-choice') === pref);
        }}
      }}
    }}
    function setTheme(pref) {{
      try {{ localStorage.setItem('admin-theme', pref); }} catch (e) {{}}
      applyTheme(pref);
    }}
    applyTheme((function() {{
      try {{ return localStorage.getItem('admin-theme') || 'system'; }} catch (e) {{ return 'system'; }}
    }})());
    function toggleHeaderMenu() {{
      var dropdown = document.getElementById('header-menu-dropdown');
      var btn = document.getElementById('header-menu-btn');
      var open = dropdown.classList.toggle('open');
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    }}
    document.addEventListener('click', function(e) {{
      var menu = document.querySelector('.header-menu');
      if (menu && !menu.contains(e.target)) {{
        document.getElementById('header-menu-dropdown').classList.remove('open');
        document.getElementById('header-menu-btn').setAttribute('aria-expanded', 'false');
      }}
    }});
    document.addEventListener('keydown', function(e) {{
      if (e.key === 'Escape') {{
        document.getElementById('header-menu-dropdown').classList.remove('open');
        document.getElementById('header-menu-btn').setAttribute('aria-expanded', 'false');
      }}
    }});
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
    (function() {{
      var flash = document.querySelector('.flash');
      if (flash) {{
        setTimeout(function() {{
          flash.style.transition = 'opacity .4s ease';
          flash.style.opacity = '0';
          setTimeout(function() {{ flash.remove(); }}, 400);
        }}, 5000);
      }}
      // Strip one-shot flash-message query params so a refresh doesn't
      // re-show the message (params vary by page: flash, success, saved).
      var oneShotParams = ['flash', 'success', 'saved', 'error'];
      if (oneShotParams.some(function(p) {{ return window.location.search.indexOf(p + '=') !== -1; }})) {{
        var url = new URL(window.location.href);
        oneShotParams.forEach(function(p) {{ url.searchParams.delete(p); }});
        window.history.replaceState({{}}, '', url.pathname + url.search + url.hash);
      }}
    }})();
  </script>
</body>
</html>"#,
        title = html_escape(title),
        css = ADMIN_CSS,
        brand_html = brand_html,
        dash = nav_link("/admin", "Dashboard"),
        posts = nav_link("/admin/posts", "Posts"),
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
            let active = if current_path.starts_with("/admin/form-data-analytics") { " class=\"active\"" } else { "" };
            format!(r#"<li><a href="/admin/form-data-analytics"{}>{}</a></li>"#,
                active,
                format!("Data - Analytics{}", badge)
            )
        } else { String::new() },
        form_designer = if ctx.can_manage_forms { nav_link("/admin/form-designer", "Forms") } else { String::new() },
        plugins = String::new(), // plugins disabled pre-launch
        documentation = if ctx.is_global_admin { nav_link("/admin/documentation", "Documentation") } else { String::new() },
        appearance = if ctx.can_manage_appearance { nav_link("/admin/appearance", "Appearance") } else { String::new() },
        menus = if ctx.can_manage_appearance { nav_link("/admin/menus", "Menus") } else { String::new() },
        builder = if ctx.can_manage_appearance { nav_link("/admin/builder", "Page Builder") } else { String::new() },
        settings = if ctx.can_manage_settings { nav_link("/admin/settings", "System Settings") } else { String::new() },
        flash_html = flash_html,
        content = content,
        visiting_badge = visiting_badge,
        site_indicator = site_indicator,
        profile_or_home = if ctx.is_impersonating { "/admin/sites/go-home?next=/admin/profile" } else { "/admin/profile" },
        user_email = html_escape(&ctx.user_email),
        media_browser_modal = media_browser_modal,
    )
}

/// Experimental /admin2 shell: same sidebar as [`admin_page`], but the header
/// is `position: fixed` to the viewport top instead of living inside a
/// `.admin-main` scroll container — content scrolls underneath it. Sidebar
/// nav links point at the real /admin/* routes since this is just a layout
/// spike, not a full parallel admin section.
pub fn admin2_page(ctx: &PageContext) -> String {
    let brand_html = match &ctx.logo_url {
        Some(url) => format!(
            r#"<img class="brand-logo" src="{}" alt="{}">"#,
            html_escape(url),
            html_escape(&ctx.app_name)
        ),
        None => html_escape(&ctx.app_name),
    };

    let nav_link = |href: &str, label: &str| -> String {
        format!(r#"<li><a href="{}">{}</a></li>"#, href, label)
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Admin2 — Synaptic Admin</title>
  <script>
    (function() {{
      try {{
        if (localStorage.getItem('admin-theme') === 'dark') {{
          document.documentElement.setAttribute('data-theme', 'dark');
        }}
      }} catch (e) {{}}
    }})();
  </script>
  <style>{css}</style>
</head>
<body>
  <div class="sidebar-overlay" onclick="closeSidebar()"></div>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <a class="brand" href="/admin">{brand_html}</a>
      <ul>
        {dash}
        {posts}
        {media}
      </ul>
    </nav>
    <main class="admin2-main">
      <header class="admin2-header">
        <button class="hamburger" onclick="toggleSidebar()" aria-label="Open navigation">
          <span></span><span></span><span></span>
        </button>
        <h1>Admin2 — fixed header layout</h1>
      </header>
      <div class="admin2-content">
        <p>Scroll this page — the header above stays pinned to the top of the viewport.</p>
        {filler}
      </div>
    </main>
  </div>
  <script>
    function toggleSidebar() {{ document.body.classList.toggle('sidebar-open'); }}
    function closeSidebar() {{ document.body.classList.remove('sidebar-open'); }}
  </script>
</body>
</html>"#,
        css = ADMIN_CSS,
        brand_html = brand_html,
        dash = nav_link("/admin", "Dashboard"),
        posts = nav_link("/admin/posts", "Posts"),
        media = "<li><a href=\"/admin/media\">Media</a></li>",
        filler = "<p style=\"height:1800px\">Filler content to force scrolling for the layout test.</p>",
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
  <script>
    // Same pre-paint theme detection as admin_page (see lib.rs) — this iframe
    // gets its own document, so without this it always renders light mode
    // regardless of what the parent admin page (and localStorage) say.
    (function() {{
      try {{
        var pref = localStorage.getItem('admin-theme') || 'system';
        var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        if (dark) {{
          document.documentElement.setAttribute('data-theme', 'dark');
        }}
      }} catch (e) {{}}
    }})();
  </script>
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
  var pickerMode = 'featured'; // 'featured', 'inline', 'audio', or 'customizer_image'
  var currentTargetId = null; // hidden-input id, only used by 'customizer_image'

  function escHtml(s) {
    return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }

  window.openMediaPicker = function(mode, targetId) {
    pickerMode = mode || 'featured';
    currentTargetId = targetId || null;
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
    } else if (pickerMode === 'customizer_image' && currentTargetId) {
      var hidden = document.getElementById(currentTargetId);
      if (hidden) {
        hidden.value = path;
        // Setting .value on a hidden input doesn't fire input/change by
        // itself — dispatch both so the customizer's dirty-check script
        // (which listens for bubbling input/change) sees this as a change.
        hidden.dispatchEvent(new Event('input', { bubbles: true }));
        hidden.dispatchEvent(new Event('change', { bubbles: true }));
      }
      var preview = document.getElementById(currentTargetId + '-preview');
      if (preview) {
        preview.style.backgroundImage = "url('" + escHtml(path) + "')";
        preview.classList.add('has-image');
      }
      var clearBtn = document.getElementById(currentTargetId + '-clear');
      if (clearBtn) clearBtn.style.display = '';
    } else {
      var fidEl  = document.getElementById('featured_image_id');
      var furlEl = document.getElementById('featured_image_url_field');
      var fclrEl = document.getElementById('featured_image_cleared');
      if (fidEl)  fidEl.value  = id;
      if (furlEl) furlEl.value = path;
      if (fclrEl) fclrEl.value = '';
      var box = document.getElementById('featured-image-box');
      if (box) {
        box.innerHTML = '<img src="' + escHtml(path) + '" alt="Featured image" style="width:100%;height:100%;object-fit:cover;display:block">';
        box.classList.add('has-image');
      }
      var rb = document.getElementById('fi-remove-btn');
      if (rb) rb.style.display = '';
      if (window.markDirty) window.markDirty();
    }
    closeMediaPicker();
  });

  window.removeFeaturedImage = function() {
    var fidEl  = document.getElementById('featured_image_id');
    var furlEl = document.getElementById('featured_image_url_field');
    var fclrEl = document.getElementById('featured_image_cleared');
    if (fidEl)  fidEl.value  = '';
    if (furlEl) furlEl.value = '';
    if (fclrEl) fclrEl.value = '1';
    var box = document.getElementById('featured-image-box');
    if (box) {
      box.classList.remove('has-image');
      box.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" style="opacity:.35"><rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10.5" r="1.5"/><path d="M3 16l4.5-4.5 3 3 2.5-2.5 5 5"/></svg><span style="color:var(--muted);font-size:12px">No image selected</span>';
    }
    var rb = document.getElementById('fi-remove-btn');
    if (rb) rb.style.display = 'none';
    if (window.markDirty) window.markDirty();
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
