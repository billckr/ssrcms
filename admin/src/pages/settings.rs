//! Admin system settings page.

use uuid::Uuid;

pub fn render(
    flash: Option<&str>,
    app_name: &str,
    timezone: &str,
    max_upload_mb: u64,
    default_theme: &str,
    sites: &[(Uuid, String)],
    ctx: &crate::PageContext,
) -> String {
    let app_name_escaped = crate::html_escape(app_name);

    let current_logo_preview = match &ctx.logo_url {
        Some(url) => format!(
            r#"<img src="{url}" alt="Current logo" style="height:38px;max-width:100%">"#,
            url = crate::html_escape(url),
        ),
        None => format!(
            r#"<span style="font-weight:600;color:#f8fafc">{app_name_escaped}</span> <span class="field-hint" style="color:#94a3b8">(text — no custom logo uploaded)</span>"#,
        ),
    };
    // No nested <form> possible (already inside the upload form, and a
    // second top-level <form> couldn't share this icon-pill visually) — a
    // plain button that posts via fetch(), same pattern as the form
    // designer/post editor's delete buttons.
    let reset_logo_btn = if ctx.logo_url.is_some() {
        r#"<button type="button" class="icon-btn" title="Reset to Text" aria-label="Reset to Text" onclick="resetLogoConfirm()">
              <img src="/admin/static/icons/rotate-ccw.svg" alt="">
            </button>"#.to_string()
    } else {
        String::new()
    };

    // Only the app owner sees the Welcome panel at all right now (dashboard.rs
    // gates it on is_global_admin too) — no point offering a "bring it back"
    // control to anyone who could never have seen it. Delegating this to other
    // roles is a later "we'll adjust this" item, not a decision yet.
    let welcome_panel_card = if ctx.is_global_admin {
        r#"<div class="card-boxed">
    <h2 class="card-boxed-header">Welcome Panel</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/settings">
      <input type="hidden" name="tab" value="welcome_panel">
      <div class="card-boxed-section">
        <p class="form-note" style="margin:0 0 .75rem">
          Dismissed the dashboard's Welcome panel? Bring it back for your own account &mdash;
          it'll show again next time you visit the Dashboard.
        </p>
        <div class="icon-pill" style="margin-top:0">
          <button type="submit" class="icon-btn" title="Show Welcome Panel Again" aria-label="Show Welcome Panel Again">
            <img src="/admin/static/icons/refresh-cw.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>"#.to_string()
    } else {
        String::new()
    };

    let theme_options = [
        ("system", "Match system"),
        ("light", "Light"),
        ("dark", "Dark"),
    ]
    .iter()
    .map(|(value, label)| {
        let selected = if *value == default_theme { " selected" } else { "" };
        format!(r#"<option value="{value}"{selected}>{label}</option>"#)
    })
    .collect::<Vec<_>>()
    .join("\n        ");

    let site_options = sites
        .iter()
        .map(|(id, hostname)| {
            format!(r#"<option value="{id}">{}</option>"#, crate::html_escape(hostname))
        })
        .collect::<Vec<_>>()
        .join("\n        ");

    // Build timezone <option> list
    let tz_options = [
        "UTC",
        "America/New_York",
        "America/Chicago",
        "America/Denver",
        "America/Los_Angeles",
        "Europe/London",
        "Europe/Paris",
        "Asia/Tokyo",
        "Australia/Sydney",
    ]
    .iter()
    .map(|tz| {
        let selected = if *tz == timezone { " selected" } else { "" };
        format!(r#"<option value="{tz}"{selected}>{tz}</option>"#)
    })
    .collect::<Vec<_>>()
    .join("\n        ");

    let content = format!(r#"
<style>
/* Tab bar look/underline is the shared .page-tabs/.page-tab (admin.css) —
   this page used to duplicate that CSS under its own .settings-tabs/
   .settings-tab-btn names, hand-kept in visual sync. Only the panel
   show/hide (below) stays page-local, since that's not shared anywhere. */
.settings-panel {{
  display: none;
  max-width: 560px;
}}

.settings-panel.active {{
  display: block;
}}

.settings-section-title {{
  font-size: .7rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: .07em;
  color: var(--muted);
  margin: 1.75rem 0 .75rem;
  padding-bottom: .4rem;
  border-bottom: 1px solid var(--border);
}}

.settings-section-title:first-child {{
  margin-top: 0;
}}

/* ── Deploy Test Data ── */
.card-boxed-section.section-danger {{
  border-color: var(--danger);
}}

.dt-spinner {{
  display: inline-block;
  width: 14px;
  height: 14px;
  margin-left: .5rem;
  border: 2px solid var(--border);
  border-top-color: var(--primary);
  border-radius: 50%;
  vertical-align: middle;
  animation: dt-spin .7s linear infinite;
}}

.dt-spinner[hidden] {{
  display: none;
}}

@keyframes dt-spin {{
  to {{ transform: rotate(360deg); }}
}}

.dt-result {{
  white-space: pre-wrap;
  font-size: .75rem;
  background: var(--bg-subtle, rgba(127,127,127,.08));
  border-radius: 6px;
  padding: .5rem .6rem;
  margin-top: .75rem;
  max-height: 220px;
  overflow-y: auto;
}}

.dt-result:empty {{
  display: none;
}}
</style>

<!-- Tab bar -->
<div class="page-tabs" role="tablist">
  <button type="button" class="page-tab active" role="tab" aria-selected="true"  aria-controls="tab-general"  data-tab="general">General</button>
  <button type="button" class="page-tab"        role="tab" aria-selected="false" aria-controls="tab-security" data-tab="security">Security</button>
  <button type="button" class="page-tab"        role="tab" aria-selected="false" aria-controls="tab-advanced" data-tab="advanced">Advanced</button>
  <button type="button" class="page-tab"        role="tab" aria-selected="false" aria-controls="tab-widgets"  data-tab="widgets">Widgets</button>
</div>

<!-- General -->
<div id="tab-general" class="settings-panel active" role="tabpanel" style="max-width:720px">
  <div class="card-boxed">
    <h2 class="card-boxed-header">General</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/settings" class="edit-form general-settings-form">
      <input type="hidden" name="tab" value="general">

      <div class="card-boxed-section">
        <div class="form-group" style="max-width:360px">
          <label for="sg-app-name">App Name</label>
          <input type="text" id="sg-app-name" name="app_name" value="{app_name}">
          <small>Shown in the admin sidebar top-left. Set to your agency or CMS brand name. Only displayed as text when no custom logo is uploaded — see Sidebar Logo below.</small>
        </div>
        <div class="icon-pill" style="margin-top:1rem">
          <button type="submit" id="general-save-btn" class="icon-btn" title="Save General" aria-label="Save General" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>

  {welcome_panel_card}

  <div class="card-boxed">
    <h2 class="card-boxed-header">Sidebar Logo</h2>
    <div class="card-boxed-body">
      <div class="card-boxed-section card-boxed-section-hidden">
        <label>Current</label>
        <div style="display:flex;align-items:center;padding:.75rem 1rem;margin-top:.4rem;background:#1e293b;border-radius:6px;max-width:360px">
          {current_logo_preview}
        </div>
      </div>
      <div class="card-boxed-section">
        <form method="post" action="/admin/settings/logo" enctype="multipart/form-data" id="logo-upload-form">
          <div class="form-group" style="max-width:360px">
            <label for="sg-logo-file">Replace with a new image</label>
            <input type="file" id="sg-logo-file" name="file" accept=".svg,.png,.webp,image/svg+xml,image/png,image/webp">
            <small>SVG, PNG, or WebP — max 2 MB. Renders at a fixed 38px sidebar height, so any aspect ratio works.</small>
          </div>
          <div class="icon-pill" style="margin-top:1rem">
            <button type="submit" id="logo-upload-btn" class="icon-btn" title="Upload Logo" aria-label="Upload Logo" disabled>
              <img src="/admin/static/icons/save.svg" alt="">
            </button>
            {reset_logo_btn}
          </div>
        </form>
      </div>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Localisation</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/settings" class="edit-form localisation-settings-form">
      <input type="hidden" name="tab" value="localisation">

      <div class="card-boxed-section">
        <div class="form-group" style="max-width:360px">
          <label for="sg-timezone">Timezone</label>
          <select id="sg-timezone" name="timezone">
            {tz_options}
          </select>
          <small>App-wide timezone — used for admin timestamps and scheduled publishing.</small>
        </div>
        <div class="icon-pill" style="margin-top:1rem">
          <button type="submit" id="localisation-save-btn" class="icon-btn" title="Save Localisation" aria-label="Save Localisation" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Appearance</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/settings" class="edit-form appearance-settings-form">
      <input type="hidden" name="tab" value="appearance">

      <div class="card-boxed-section">
        <div class="form-group" style="max-width:360px">
          <label for="sg-default-theme">Default Theme</label>
          <select id="sg-default-theme" name="default_theme">
            {theme_options}
          </select>
          <small>
            Fallback appearance for the login, signup, and admin sign-in pages, and for anyone
            who hasn't yet picked their own theme. Signed-in users can still override it per
            browser from the light/system/dark switch in the header menu.
          </small>
        </div>
        <div class="icon-pill" style="margin-top:1rem">
          <button type="submit" id="appearance-save-btn" class="icon-btn" title="Save Appearance" aria-label="Save Appearance" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>
</div>

<!-- Security -->
<div id="tab-security" class="settings-panel" role="tabpanel" style="max-width:720px">
  <div class="card-boxed">
    <h2 class="card-boxed-header">Security</h2>
    <div class="card-boxed-body">
      <p style="color:var(--muted);font-size:.875rem;font-style:italic;margin:0">
        Security settings — coming soon. Session timeouts, login lockout, and password
        policy configuration will be available here once the underlying features are built.
      </p>
    </div>
  </div>
</div>

<!-- Advanced -->
<div id="tab-advanced" class="settings-panel" role="tabpanel" style="max-width:720px">
  <div class="card-boxed">
    <h2 class="card-boxed-header">Uploads</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/settings" class="edit-form uploads-settings-form">
      <input type="hidden" name="tab" value="uploads">
      <div class="card-boxed-section">
        <div class="form-group">
          <label for="sa-max-upload">Max Upload Size (MB)</label>
          <input type="number" id="sa-max-upload" name="max_upload_mb" value="{max_upload_mb}" min="1" max="1000" style="width:100px">
          <small>Applies to media and theme zip uploads. Takes effect immediately, no restart needed.</small>
        </div>
        <div class="icon-pill" style="margin-top:1rem">
          <button type="submit" id="uploads-save-btn" class="icon-btn" title="Save Uploads" aria-label="Save Uploads" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Seed Users</h2>
    <div class="card-boxed-body">
  <div class="card-boxed-section">
    <div class="user-form-grid stacked">
      <div class="form-group">
        <label for="dt-site-users">Target site</label>
        <select id="dt-site-users">
          <option value="" selected disabled>Select site&hellip;</option>
          {site_options}
        </select>
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-user-role">Role</label>
        <select id="dt-user-role">
          <option value="subscriber">Subscriber</option>
          <option value="author">Author</option>
          <option value="editor">Editor</option>
          <option value="admin">Admin</option>
        </select>
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-user-count">Count</label>
        <input type="number" id="dt-user-count" value="5" min="1" max="200">
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-user-password">Password (optional)</label>
        <input type="text" id="dt-user-password" placeholder="random per user">
      </div>
    </div>
    <div style="margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" onclick="seedUsers()" id="dtUserBtn" title="Seed Users" aria-label="Seed Users" disabled>
          <img src="/admin/static/icons/users.svg" alt="">
        </button>
      </div>
      <span class="dt-spinner" id="dtUserSpinner" hidden></span>
      <pre id="dtUserResult" class="dt-result"></pre>
    </div>
  </div>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Seed Posts / Pages</h2>
    <div class="card-boxed-body">
  <div class="card-boxed-section">
    <div class="user-form-grid stacked">
      <div class="form-group">
        <label for="dt-site-posts">Target site</label>
        <select id="dt-site-posts">
          <option value="" selected disabled>Select site&hellip;</option>
          {site_options}
        </select>
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-post-author">Author email</label>
        <input type="email" id="dt-post-author" placeholder="author@example.com">
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-post-type">Type</label>
        <select id="dt-post-type">
          <option value="post">Post</option>
          <option value="page">Page</option>
        </select>
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-post-count">Count</label>
        <input type="number" id="dt-post-count" value="10" min="1" max="200">
      </div>
      <div class="form-group" style="max-width:280px">
        <label for="dt-post-status">Status</label>
        <select id="dt-post-status">
          <option value="mixed">Mixed</option>
          <option value="published">Published</option>
          <option value="draft">Draft</option>
          <option value="pending">Pending</option>
        </select>
      </div>
      <div class="form-group">
        <label style="display:inline;font-weight:400"><input type="checkbox" id="dt-post-extras" style="display:inline;width:auto;height:auto"> Create + assign categories/tags</label>
      </div>
    </div>
    <div style="margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" onclick="seedPosts()" id="dtPostBtn" title="Seed Posts" aria-label="Seed Posts" disabled>
          <img src="/admin/static/icons/file-text.svg" alt="">
        </button>
      </div>
      <span class="dt-spinner" id="dtPostSpinner" hidden></span>
      <pre id="dtPostResult" class="dt-result"></pre>
    </div>
  </div>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Clear Test Data</h2>
    <div class="card-boxed-body">
  <div class="card-boxed-section section-danger">
    <div class="user-form-grid stacked">
      <div class="form-group">
        <label for="dt-site-clear">Target site</label>
        <select id="dt-site-clear">
          {site_options}
        </select>
      </div>
      <p style="font-size:.8rem;color:var(--muted);margin:0">
        Deletes all posts, pages, comments, taxonomies, form submissions, media rows, and nav
        menus for the selected site. Site settings are not affected. This cannot be undone.
      </p>
      <div class="form-group">
        <label style="display:inline;font-weight:400">
          <input type="checkbox" id="dt-clear-users" style="display:inline;width:auto;height:auto">
          Also delete users created by seeding (never touches real/pre-existing users)
        </label>
      </div>
    </div>
    <div style="margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn icon-danger" onclick="clearTestData()" id="dtClearBtn" title="Clear Test Data" aria-label="Clear Test Data">
          <img src="/admin/static/icons/delete.svg" alt="">
        </button>
      </div>
      <span class="dt-spinner" id="dtClearSpinner" hidden></span>
      <pre id="dtClearResult" class="dt-result"></pre>
    </div>
  </div>
    </div>
  </div>
</div>

<!-- Widgets -->
<div id="tab-widgets" class="settings-panel" role="tabpanel" style="max-width:720px">
  <div class="card-boxed">
    <h2 class="card-boxed-header">Widgets</h2>
    <div class="card-boxed-body">
      <p style="color:var(--muted);font-size:.875rem;font-style:italic;margin:0">
        Widgets — coming soon.
      </p>
    </div>
  </div>
</div>

<script>
function dtSiteId(selectId) {{ return document.getElementById(selectId).value; }}

function dtSetBusy(btn, spinner, busy) {{
  btn.disabled = busy;
  spinner.hidden = !busy;
}}

function dtLinkSiteSelect(selectId, btnId) {{
  var select = document.getElementById(selectId);
  var btn = document.getElementById(btnId);
  select.addEventListener('change', function () {{ btn.disabled = !select.value; }});
}}
dtLinkSiteSelect('dt-site-users', 'dtUserBtn');
dtLinkSiteSelect('dt-site-posts', 'dtPostBtn');

function dtPost(path, body, resultEl) {{
  return fetch(path, {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify(body),
  }})
    .then(function (r) {{ return r.json().then(function (data) {{ return {{ status: r.status, data: data }}; }}); }})
    .catch(function (err) {{
      resultEl.textContent = 'Request failed: ' + err;
      return null;
    }});
}}

window.seedUsers = function () {{
  var btn = document.getElementById('dtUserBtn');
  var spinner = document.getElementById('dtUserSpinner');
  var resultEl = document.getElementById('dtUserResult');
  var body = {{
    site_id: dtSiteId('dt-site-users'),
    role: document.getElementById('dt-user-role').value,
    count: parseInt(document.getElementById('dt-user-count').value, 10) || 1,
    password: document.getElementById('dt-user-password').value || null,
  }};
  resultEl.textContent = '';
  dtSetBusy(btn, spinner, true);
  dtPost('/admin/settings/dev-tools/seed-users', body, resultEl).then(function (res) {{
    dtSetBusy(btn, spinner, false);
    if (!res) return;
    if (!res.data.ok) {{
      resultEl.textContent = 'Error: ' + (res.data.error || 'unknown error');
      return;
    }}
    resultEl.textContent = 'Created ' + res.data.created + ', skipped ' + res.data.skipped + '.';
  }});
}};

window.seedPosts = function () {{
  var btn = document.getElementById('dtPostBtn');
  var spinner = document.getElementById('dtPostSpinner');
  var resultEl = document.getElementById('dtPostResult');
  var body = {{
    site_id: dtSiteId('dt-site-posts'),
    author_email: document.getElementById('dt-post-author').value,
    post_type: document.getElementById('dt-post-type').value,
    count: parseInt(document.getElementById('dt-post-count').value, 10) || 1,
    status: document.getElementById('dt-post-status').value,
    extras: document.getElementById('dt-post-extras').checked,
  }};
  resultEl.textContent = '';
  dtSetBusy(btn, spinner, true);
  dtPost('/admin/settings/dev-tools/seed-posts', body, resultEl).then(function (res) {{
    dtSetBusy(btn, spinner, false);
    if (!res) return;
    if (!res.data.ok) {{
      resultEl.textContent = 'Error: ' + (res.data.error || 'unknown error');
      return;
    }}
    resultEl.textContent = 'Created ' + res.data.created + ', skipped ' + res.data.skipped +
      ', ' + res.data.assigned + ' category/tag assignments.';
  }});
}};

window.clearTestData = function () {{
  var deleteUsers = document.getElementById('dt-clear-users').checked;
  var msg = 'Delete ALL posts, comments, taxonomies, form submissions, media, and nav menus for this site?' +
    (deleteUsers ? ' This will also delete users created by seeding.' : '') +
    ' This cannot be undone.';
  if (!confirm(msg)) {{
    return;
  }}
  var btn = document.getElementById('dtClearBtn');
  var spinner = document.getElementById('dtClearSpinner');
  var resultEl = document.getElementById('dtClearResult');
  var body = {{ site_id: dtSiteId('dt-site-clear'), delete_users: deleteUsers }};
  resultEl.textContent = '';
  dtSetBusy(btn, spinner, true);
  dtPost('/admin/settings/dev-tools/clear', body, resultEl).then(function (res) {{
    dtSetBusy(btn, spinner, false);
    if (!res) return;
    if (!res.data.ok) {{
      resultEl.textContent = 'Error: ' + (res.data.error || 'unknown error');
      return;
    }}
    resultEl.textContent = 'Cleared.' + (deleteUsers ? ' Deleted ' + res.data.deleted_users + ' seeded user(s).' : '');
  }});
}};

(function () {{
  var tabs    = document.querySelectorAll('.page-tab[data-tab]');
  var panels  = document.querySelectorAll('.settings-panel');

  function activate(tabName) {{
    tabs.forEach(function (btn) {{
      var on = btn.dataset.tab === tabName;
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-selected', on ? 'true' : 'false');
    }});
    panels.forEach(function (panel) {{
      panel.classList.toggle('active', panel.id === 'tab-' + tabName);
    }});
    // Persist across page loads.
    try {{ sessionStorage.setItem('settings-tab', tabName); }} catch (e) {{}}
  }}

  tabs.forEach(function (btn) {{
    btn.addEventListener('click', function () {{ activate(btn.dataset.tab); }});
  }});

  // Which tab to land on: an explicit ?tab= in the URL wins (so a direct
  // link like /admin/settings?tab=advanced works, same convention as the
  // Form Designer editor's tabs and /admin/analytics?tab=...), else fall
  // back to the last tab used this session, else the default (General,
  // already marked active in the HTML).
  var wantedTab = new URLSearchParams(window.location.search).get('tab');
  if (wantedTab && document.querySelector('.page-tab[data-tab="' + wantedTab + '"]')) {{
    activate(wantedTab);
  }} else {{
    try {{
      var saved = sessionStorage.getItem('settings-tab');
      if (saved) activate(saved);
    }} catch (e) {{}}
  }}
}}());

function dtEnableOnChange(formSelector, btnId) {{
  var form = document.querySelector(formSelector);
  var btn  = document.getElementById(btnId);
  function snapshot() {{
    return Array.from(new FormData(form).entries()).map(function (e) {{ return e[0] + '=' + e[1]; }}).join('&');
  }}
  var initialSnapshot = snapshot();
  function checkChanged() {{
    btn.disabled = snapshot() === initialSnapshot;
  }}
  form.addEventListener('input', checkChanged);
  form.addEventListener('change', checkChanged);
}}
dtEnableOnChange('.general-settings-form', 'general-save-btn');
dtEnableOnChange('.localisation-settings-form', 'localisation-save-btn');
dtEnableOnChange('.appearance-settings-form', 'appearance-save-btn');
dtEnableOnChange('.uploads-settings-form', 'uploads-save-btn');

// Logo upload: dtEnableOnChange's FormData-snapshot diff can't tell a
// selected file apart from an empty one (a File serializes to the same
// "[object File]" string either way) — just check input.files directly.
(function() {{
  var fileInput = document.getElementById('sg-logo-file');
  var uploadBtn = document.getElementById('logo-upload-btn');
  if (fileInput && uploadBtn) {{
    fileInput.addEventListener('change', function() {{
      uploadBtn.disabled = fileInput.files.length === 0;
    }});
  }}
}})();
window.resetLogoConfirm = function() {{
  if (!confirm('Remove the custom logo and go back to showing the app name as text?')) return;
  fetch('/admin/settings/logo/reset', {{ method: 'POST' }}).then(function(r) {{
    window.location.href = r.url || '/admin/settings';
  }});
}};
</script>
"#,
        app_name = app_name_escaped,
        current_logo_preview = current_logo_preview,
        reset_logo_btn = reset_logo_btn,
        welcome_panel_card = welcome_panel_card,
        tz_options = tz_options,
        max_upload_mb = max_upload_mb,
        site_options = site_options,
    );

    crate::admin_page("System Settings", "/admin/settings", flash, &content, ctx)
}
