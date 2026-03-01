//! Admin system settings page.

pub fn render(
    flash: Option<&str>,
    app_name: &str,
    timezone: &str,
    admin_email: &str,
    max_upload_mb: u64,
    ctx: &crate::PageContext,
) -> String {
    let app_name_escaped = crate::html_escape(app_name);
    let admin_email_escaped = crate::html_escape(admin_email);

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
</style>

<!-- Tab bar -->
<div class="settings-tabs" role="tablist">
  <button class="settings-tab-btn active" role="tab" aria-selected="true"  aria-controls="tab-general"  data-tab="general">General</button>
  <button class="settings-tab-btn"        role="tab" aria-selected="false" aria-controls="tab-security" data-tab="security">Security</button>
  <button class="settings-tab-btn"        role="tab" aria-selected="false" aria-controls="tab-advanced" data-tab="advanced">Advanced</button>
</div>

<!-- General -->
<div id="tab-general" class="settings-panel active" role="tabpanel">
  <form method="post" action="/admin/settings" class="edit-form">
    <input type="hidden" name="tab" value="general">

    <p class="settings-section-title">Identity</p>
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

<!-- Security -->
<div id="tab-security" class="settings-panel" role="tabpanel">
  <p style="color:var(--muted);font-size:.875rem;font-style:italic;margin:0">
    Security settings — coming soon. Session timeouts, login lockout, and password
    policy configuration will be available here once the underlying features are built.
  </p>
</div>

<!-- Advanced -->
<div id="tab-advanced" class="settings-panel" role="tabpanel">
  <p class="settings-section-title">Uploads</p>
  <div class="form-group">
    <label for="sa-max-upload">Max Upload Size (MB)</label>
    <input type="number" id="sa-max-upload" value="{max_upload_mb}" readonly
           style="width:100px;opacity:.7;cursor:not-allowed" title="Set via MAX_UPLOAD_MB in .env or synaptic.toml">
    <small>Set via <code>MAX_UPLOAD_MB</code> in <code>.env</code> or <code>synaptic.toml</code>. Requires a restart to change. Applies to media and theme zip uploads.</small>
  </div>
</div>

<script>
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
    );

    crate::admin_page("System Settings", "/admin/settings", flash, &content, ctx)
}
