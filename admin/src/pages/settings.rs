//! Admin system settings page.

use uuid::Uuid;

pub fn render(
    flash: Option<&str>,
    app_name: &str,
    timezone: &str,
    admin_email: &str,
    max_upload_mb: u64,
    sites: &[(Uuid, String)],
    ctx: &crate::PageContext,
) -> String {
    let app_name_escaped = crate::html_escape(app_name);
    let admin_email_escaped = crate::html_escape(admin_email);

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
/* ── Settings tabs ── */
.settings-tabs {{
  display: flex;
  gap: 0;
  border-bottom: 2px solid var(--border);
  margin-bottom: 1.75rem;
}}

.settings-tab-btn {{
  padding: .55rem 1.1rem;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -2px;
  font-size: .875rem;
  font-weight: 500;
  color: var(--muted);
  cursor: pointer;
  transition: color .15s, border-color .15s;
  white-space: nowrap;
}}

.settings-tab-btn:hover {{
  color: var(--text);
}}

.settings-tab-btn.active {{
  color: var(--primary);
  border-bottom-color: var(--primary);
  font-weight: 600;
}}

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
.dt-card {{
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1rem 1.1rem;
  margin-bottom: 1rem;
}}

.dt-card h4 {{
  margin: 0 0 .75rem;
  font-size: .9rem;
}}

.dt-card label {{
  display: block;
  font-size: .8rem;
  color: var(--muted);
  margin-bottom: .6rem;
}}

.dt-card input, .dt-card select {{
  display: block;
  margin-top: .2rem;
}}

.dt-card.dt-danger {{
  border-color: #d9534f;
}}

.dt-card .btn-danger {{
  background: #d9534f;
  color: #fff;
  border: none;
  border-radius: 6px;
  padding: .5rem 1rem;
  cursor: pointer;
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

.dt-card pre {{
  white-space: pre-wrap;
  font-size: .75rem;
  background: var(--bg-subtle, rgba(127,127,127,.08));
  border-radius: 6px;
  padding: .5rem .6rem;
  margin-top: .75rem;
  max-height: 220px;
  overflow-y: auto;
}}

.dt-card pre:empty {{
  display: none;
}}
</style>

<!-- Tab bar -->
<div class="settings-tabs" role="tablist">
  <button class="settings-tab-btn active" role="tab" aria-selected="true"  aria-controls="tab-general"  data-tab="general">General</button>
  <button class="settings-tab-btn"        role="tab" aria-selected="false" aria-controls="tab-security" data-tab="security">Security</button>
  <button class="settings-tab-btn"        role="tab" aria-selected="false" aria-controls="tab-advanced" data-tab="advanced">Advanced</button>
</div>

<!-- General -->
<div id="tab-general" class="settings-panel active" role="tabpanel" style="max-width:720px">
  <div class="profile-container">
    <h2>General</h2>
    <form method="post" action="/admin/settings" class="edit-form">
      <input type="hidden" name="tab" value="general">

      <div class="form-group">
        <label for="sg-app-name">App Name</label>
        <input type="text" id="sg-app-name" name="app_name" value="{app_name}">
        <small>Shown in the admin sidebar top-left. Set to your agency or CMS brand name.</small>
      </div>
      <div class="form-group">
        <label for="sg-admin-email">Admin Email</label>
        <input type="email" id="sg-admin-email" value="{admin_email}" readonly
               style="opacity:.7;cursor:not-allowed" title="Set via ADMIN_EMAIL in .env or synaptic.toml">
        <small>Set via <code>ADMIN_EMAIL</code> in <code>.env</code> or <code>synaptic.toml</code>. Requires a restart to change.</small>
      </div>

      <p class="settings-section-title">Localisation</p>
      <div class="form-group">
        <label for="sg-timezone">Timezone</label>
        <select id="sg-timezone" name="timezone">
          {tz_options}
        </select>
        <small>App-wide timezone — used for admin timestamps and scheduled publishing.</small>
      </div>

      <div class="form-actions" style="margin-top:1.5rem">
        <button type="submit" class="btn btn-primary">Save General</button>
      </div>
    </form>
  </div>
</div>

<!-- Security -->
<div id="tab-security" class="settings-panel" role="tabpanel">
  <p style="color:var(--muted);font-size:.875rem;font-style:italic;margin:0">
    Security settings — coming soon. Session timeouts, login lockout, and password
    policy configuration will be available here once the underlying features are built.
  </p>
</div>

<!-- Advanced -->
<div id="tab-advanced" class="settings-panel" role="tabpanel" style="max-width:720px">
  <div class="profile-container">
    <h2>Uploads</h2>
    <div class="form-group">
      <label for="sa-max-upload">Max Upload Size (MB)</label>
      <input type="number" id="sa-max-upload" value="{max_upload_mb}" readonly
             style="width:100px;opacity:.7;cursor:not-allowed" title="Set via MAX_UPLOAD_MB in .env or synaptic.toml">
      <small>Set via <code>MAX_UPLOAD_MB</code> in <code>.env</code> or <code>synaptic.toml</code>. Requires a restart to change. Applies to media and theme zip uploads.</small>
    </div>
  </div>

  <div class="profile-container">
    <h2>Deploy Test Data</h2>
    <div class="form-group">
      <label for="dt-site">Target site</label>
      <select id="dt-site">
        {site_options}
      </select>
      <small>All actions below apply to this site.</small>
    </div>

  <div class="dt-card">
    <h4>Seed users</h4>
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
    <button type="button" class="btn" onclick="seedUsers()" id="dtUserBtn">Seed Users</button>
    <span class="dt-spinner" id="dtUserSpinner" hidden></span>
    <pre id="dtUserResult"></pre>
  </div>

  <div class="dt-card">
    <h4>Seed posts / pages</h4>
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
    <button type="button" class="btn" onclick="seedPosts()" id="dtPostBtn">Seed Posts</button>
    <span class="dt-spinner" id="dtPostSpinner" hidden></span>
    <pre id="dtPostResult"></pre>
  </div>

  <div class="dt-card dt-danger">
    <h4>Clear test data</h4>
    <p style="font-size:.8rem;color:var(--muted);margin:0 0 .75rem">
      Deletes all posts, pages, comments, taxonomies, form submissions, media rows, and nav
      menus for the selected site. Site settings are not affected. This cannot be undone.
    </p>
    <div class="form-group">
      <label style="display:inline;font-weight:400">
        <input type="checkbox" id="dt-clear-users" style="display:inline;width:auto;height:auto">
        Also delete users created by seeding (never touches real/pre-existing users)
      </label>
    </div>
    <button type="button" class="btn-danger" onclick="clearTestData()" id="dtClearBtn">Clear Test Data</button>
    <span class="dt-spinner" id="dtClearSpinner" hidden></span>
    <pre id="dtClearResult"></pre>
  </div>
  </div>
</div>

<script>
function dtSiteId() {{ return document.getElementById('dt-site').value; }}

function dtSetBusy(btn, spinner, busy) {{
  btn.disabled = busy;
  spinner.hidden = !busy;
}}

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
    site_id: dtSiteId(),
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
    site_id: dtSiteId(),
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
    var lines = ['Created ' + res.data.created + ', skipped ' + res.data.skipped +
      ', ' + res.data.assigned + ' category/tag assignments.'];
    (res.data.urls || []).forEach(function (u) {{ lines.push(u); }});
    resultEl.textContent = lines.join('\\n');
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
  var body = {{ site_id: dtSiteId(), delete_users: deleteUsers }};
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
  var tabs    = document.querySelectorAll('.settings-tab-btn');
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

  // Restore last active tab.
  try {{
    var saved = sessionStorage.getItem('settings-tab');
    if (saved) activate(saved);
  }} catch (e) {{}}
}}());
</script>
"#,
        app_name = app_name_escaped,
        admin_email = admin_email_escaped,
        tz_options = tz_options,
        max_upload_mb = max_upload_mb,
        site_options = site_options,
    );

    crate::admin_page("System Settings", "/admin/settings", flash, &content, ctx)
}
