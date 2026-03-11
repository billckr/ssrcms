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
        {cats}
        {tags}
        {users}
        {sites}
        {forms}
        {plugins}
        {documentation}
        {appearance}
        {menus}
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
        media = nav_link("/admin/media", "Media"),
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
        plugins = if ctx.can_manage_plugins { nav_link("/admin/plugins", "Plugins") } else { String::new() },
        documentation = if ctx.can_manage_settings || ctx.is_global_admin { nav_link("/admin/documentation", "Documentation") } else { String::new() },
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
    )
}

/// Minimal HTML escaping for values inserted into HTML attributes or text.
/// Returns the shared media-picker modal HTML + JS.
/// Supports pickerMode: 'featured', 'inline', 'browse'.
/// In 'browse' mode the Set/Insert confirm button is hidden.
pub fn media_picker_modal_html() -> String {
    String::from(r#"<div id="media-picker-modal" class="mpicker-overlay" style="display:none" onclick="if(event.target===this)closeMediaPicker()">
  <div class="mpicker-dialog">
    <div class="mpicker-header">
      <span class="mpicker-title">Media Library</span>
      <input type="text" id="mpicker-search" class="mpicker-search" placeholder="Search images&#x2026;" oninput="filterMedia(this.value)" autocomplete="off">
      <button type="button" class="mpicker-close" onclick="closeMediaPicker()" title="Close">&#x2715;</button>
    </div>
    <div class="mpicker-body">
      <div class="mpicker-grid" id="mpicker-grid"><p class="mpicker-loading">Loading&#x2026;</p></div>
      <div class="mpicker-detail" id="mpicker-detail"><div class="mpicker-detail-empty">Select an image to see details</div></div>
    </div>
  </div>
</div>
<script>
(function() {
  var allMedia = [];
  var selectedId = null;
  var selectedUrl = null;
  var selectedAlt = null;
  var loaded = false;
  var pickerMode = 'featured'; // 'featured', 'inline', or 'browse'
  window.openMediaPicker = function(mode) {
    pickerMode = mode || 'featured';
    var titleEl = document.querySelector('.mpicker-title');
    if (titleEl) titleEl.textContent = pickerMode === 'browse' ? 'Media Library' : 'Featured Image';
    if (pickerMode === 'featured') {
      var fidEl = document.getElementById('featured_image_id');
      var furlEl = document.getElementById('featured_image_url_field');
      selectedId  = fidEl  ? fidEl.value  || null : null;
      selectedUrl = furlEl ? furlEl.value || null : null;
    } else {
      selectedId = null;
      selectedUrl = null;
    }
    selectedAlt = null;
    // Reset the detail panel whenever there is no already-confirmed selection,
    // so a previously clicked-but-not-confirmed image is never shown again.
    if (!selectedId) {
      document.getElementById('mpicker-detail').innerHTML = '<div class="mpicker-detail-empty">Select an image to see details</div>';
    }
    document.getElementById('media-picker-modal').style.display = '';
    document.getElementById('mpicker-search').value = '';
    if (!loaded) {
      loaded = true;
      fetch('/admin/api/media')
        .then(function(r) { return r.json(); })
        .then(function(data) { allMedia = data; renderGrid(data); })
        .catch(function() {
          document.getElementById('mpicker-grid').innerHTML = '<p class="mpicker-loading">Failed to load images.</p>';
        });
    } else {
      renderGrid(allMedia);
    }
  };
  window.closeMediaPicker = function() {
    document.getElementById('media-picker-modal').style.display = 'none';
  };
  window.filterMedia = function(q) {
    var lower = q.toLowerCase();
    renderGrid(lower ? allMedia.filter(function(m) { return m.filename.toLowerCase().indexOf(lower) !== -1; }) : allMedia);
  };
  function escHtml(s) {
    return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }
  function renderGrid(items) {
    var grid = document.getElementById('mpicker-grid');
    if (!items.length) { grid.innerHTML = '<p class="mpicker-loading">No images found.</p>'; return; }
    grid.innerHTML = items.map(function(m) {
      var sel = m.id === selectedId ? ' mpicker-thumb-selected' : '';
      return '<div class="mpicker-thumb' + sel + '" onclick="pickThumb(this)"'
        + ' data-id="' + escHtml(m.id) + '"'
        + ' data-url="' + escHtml(m.url) + '"'
        + ' data-filename="' + escHtml(m.filename) + '"'
        + ' data-alt="' + escHtml(m.alt_text) + '"'
        + ' data-title="' + escHtml(m.title) + '"'
        + ' data-caption="' + escHtml(m.caption) + '"'
        + ' data-size="' + (m.file_size || 0) + '"'
        + ' data-mime="' + escHtml(m.mime_type) + '">'
        + '<img src="' + escHtml(m.url) + '" alt="' + escHtml(m.alt_text || m.filename) + '" loading="lazy">'
        + '</div>';
    }).join('');
  }
  window.pickThumb = function(el) {
    selectedId  = el.dataset.id;
    selectedUrl = el.dataset.url;
    selectedAlt = el.dataset.alt || el.dataset.filename;
    document.querySelectorAll('.mpicker-thumb').forEach(function(t) { t.classList.remove('mpicker-thumb-selected'); });
    el.classList.add('mpicker-thumb-selected');
    var confirmBtn = pickerMode !== 'browse'
      ? '<button type="button" class="btn btn-primary" style="width:100%;margin-top:1rem" onclick="confirmPick()">'
          + (pickerMode === 'inline' ? 'Insert Image' : 'Set Image')
          + '</button>'
      : '';
    var rawSize = parseInt(el.dataset.size || '0', 10);
    var sizeStr = rawSize >= 1048576
      ? (rawSize / 1048576).toFixed(1) + ' MB'
      : rawSize >= 1024
        ? (rawSize / 1024).toFixed(1) + ' KB'
        : rawSize + ' B';
    var mimeStr = el.dataset.mime || '—';
    document.getElementById('mpicker-detail').innerHTML =
      '<div class="mpicker-detail-img"><img src="' + escHtml(selectedUrl) + '" alt="' + escHtml(el.dataset.alt || el.dataset.filename) + '"></div>'
      + confirmBtn
      + '<div style="margin-top:1.25rem;display:flex;flex-direction:column;gap:0.75rem">'
      + '<div style="display:grid;grid-template-columns:auto 1fr;gap:.2rem .75rem;font-size:13px">'
      + '<span style="color:var(--muted)">Name</span><span style="word-break:break-all">' + escHtml(el.dataset.filename) + '</span>'
      + '<span style="color:var(--muted)">Size</span><span>' + escHtml(sizeStr) + '</span>'
      + '<span style="color:var(--muted)">Type</span><span>' + escHtml(mimeStr) + '</span>'
      + '</div>'
      + '<div>'
      + '<div style="display:flex;justify-content:space-between;margin-bottom:4px"><label style="font-size:12px;font-weight:600;color:var(--muted)">Alt Text <span style="font-weight:400">(screen readers)</span></label><span id="mpicker-alt-count" style="font-size:11px;color:var(--muted)">' + (35 - (el.dataset.alt || '').length) + '/35</span></div>'
      + '<input id="mpicker-alt-input" type="text" maxlength="35" placeholder="Describe this image..." value="' + escHtml(el.dataset.alt || '') + '" oninput="mpickerCount(\'mpicker-alt-input\',\'mpicker-alt-count\')">'
      + '</div>'
      + '<div>'
      + '<div style="display:flex;justify-content:space-between;margin-bottom:4px"><label style="font-size:12px;font-weight:600;color:var(--muted)">Title <span style="font-weight:400">(tooltip on hover)</span></label><span id="mpicker-title-count" style="font-size:11px;color:var(--muted)">' + (35 - (el.dataset.title || '').length) + '/35</span></div>'
      + '<input id="mpicker-title-input" type="text" maxlength="35" placeholder="Optional image title..." value="' + escHtml(el.dataset.title || '') + '" oninput="mpickerCount(\'mpicker-title-input\',\'mpicker-title-count\')">'
      + '</div>'
      + '<div style="flex:1;display:flex;flex-direction:column">'
      + '<div style="display:flex;justify-content:space-between;margin-bottom:4px"><label style="font-size:12px;font-weight:600;color:var(--muted)">Caption <span style="font-weight:400">(shown below image)</span></label><span id="mpicker-caption-count" style="font-size:11px;color:var(--muted)">' + (35 - (el.dataset.caption || '').length) + '/35</span></div>'
      + '<textarea id="mpicker-caption-input" rows="3" maxlength="35" placeholder="Optional caption..." oninput="mpickerCount(\'mpicker-caption-input\',\'mpicker-caption-count\')">' + escHtml(el.dataset.caption || '') + '</textarea>'
      + '</div>'
      + '<button type="button" class="btn btn-primary" style="width:100%;margin-top:0" onclick="saveMediaMeta()">Update</button>'
      + '</div>';
  };
  window.mpickerCount = function(inputId, countId) {
    var el = document.getElementById(inputId);
    var ct = document.getElementById(countId);
    if (!el || !ct) return;
    // Strip HTML characters — no markup allowed in alt/title/caption.
    var cleaned = el.value.replace(/[<>&"`]/g, '');
    if (cleaned !== el.value) {
      el.value = cleaned;
      el.style.outline = '2px solid var(--danger)';
      setTimeout(function() { el.style.outline = ''; }, 1200);
    }
    var remaining = 35 - el.value.length;
    ct.textContent = remaining + '/35';
    ct.style.color = remaining <= 5 ? 'var(--danger)' : 'var(--muted)';
  };
  function readCurrentMeta() {
    return {
      alt:     (document.getElementById('mpicker-alt-input')     || {}).value || '',
      title:   (document.getElementById('mpicker-title-input')   || {}).value || '',
      caption: (document.getElementById('mpicker-caption-input') || {}).value || ''
    };
  }
  window.saveMediaMeta = function(silent) {
    if (!selectedId) return;
    var meta = readCurrentMeta();
    var btn = silent ? null : document.querySelector('[onclick="saveMediaMeta()"]');
    if (btn) btn.textContent = 'Saving\u2026';
    selectedAlt = meta.alt.trim();
    var thumb = document.querySelector('.mpicker-thumb[data-id="' + selectedId + '"]');
    if (thumb) {
      thumb.dataset.alt     = meta.alt.trim();
      thumb.dataset.title   = meta.title.trim();
      thumb.dataset.caption = meta.caption.trim();
    }
    // Keep allMedia in sync so reopening the modal reflects saved values.
    var item = allMedia.find(function(m) { return m.id === selectedId; });
    if (item) {
      item.alt_text = meta.alt.trim();
      item.title    = meta.title.trim();
      item.caption  = meta.caption.trim();
    }
    fetch('/admin/api/media/' + selectedId + '/meta', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ alt_text: meta.alt.trim(), title: meta.title.trim(), caption: meta.caption.trim() })
    }).then(function() {
      if (btn) btn.textContent = 'Saved \u2713';
      setTimeout(function() { if (btn) btn.textContent = 'Update'; }, 1500);
    }).catch(function() {
      if (btn) btn.textContent = 'Error \u2014 try again';
    });
  };
  window.confirmFeaturedImage = function() {
    if (!selectedId) return;
    saveMediaMeta(true);
    document.getElementById('featured_image_id').value = selectedId;
    document.getElementById('featured_image_url_field').value = selectedUrl;
    var box = document.getElementById('featured-image-box');
    box.innerHTML = '<img src="' + escHtml(selectedUrl) + '" alt="Featured image" style="width:100%;height:100%;object-fit:cover;display:block">';
    box.classList.add('has-image');
    var rb = document.getElementById('fi-remove-btn');
    if (rb) rb.style.display = '';
    closeMediaPicker();
  };
  window.confirmPick = function() {
    if (!selectedId) return;
    if (pickerMode === 'inline') {
      var q = window._quillInstance;
      if (q) {
        var range = window._quillRange || q.getSelection(true);
        var imgHtml = '<img src="' + selectedUrl + '" alt="' + (selectedAlt || '').replace(/"/g, '&quot;') + '">';
        q.clipboard.dangerouslyPasteHTML(range.index, imgHtml, 'user');
        q.setSelection(range.index + 1, 0, 'silent');
      }
      closeMediaPicker();
    } else {
      confirmFeaturedImage();
    }
  };
  window.removeFeaturedImage = function() {
    selectedId = null;
    selectedUrl = null;
    document.getElementById('featured_image_id').value = '';
    document.getElementById('featured_image_url_field').value = '';
    var box = document.getElementById('featured-image-box');
    box.classList.remove('has-image');
    box.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" style="opacity:.35"><rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10.5" r="1.5"/><path d="M3 16l4.5-4.5 3 3 2.5-2.5 5 5"/></svg><span style="color:var(--muted);font-size:12px">No image selected</span>';
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
