//! Admin system settings page.

pub fn render(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = r#"
<style>
/* ── Settings tabs ── */
.settings-tabs {
  display: flex;
  gap: 0;
  border-bottom: 2px solid var(--border);
  margin-bottom: 1.75rem;
}

.settings-tab-btn {
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
}

.settings-tab-btn:hover {
  color: var(--text);
}

.settings-tab-btn.active {
  color: var(--primary);
  border-bottom-color: var(--primary);
  font-weight: 600;
}

.settings-panel {
  display: none;
  max-width: 560px;
}

.settings-panel.active {
  display: block;
}

.settings-section-title {
  font-size: .7rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: .07em;
  color: var(--muted);
  margin: 1.75rem 0 .75rem;
  padding-bottom: .4rem;
  border-bottom: 1px solid var(--border);
}

.settings-section-title:first-child {
  margin-top: 0;
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: .6rem 0;
  border-bottom: 1px solid var(--border);
}

.toggle-row:last-child { border-bottom: none; }

.toggle-label { font-size: .875rem; }
.toggle-label small { display: block; color: var(--muted); font-size: .8rem; margin-top: .1rem; }

.toggle-switch {
  position: relative;
  width: 36px;
  height: 20px;
  flex-shrink: 0;
}

.toggle-switch input { opacity: 0; width: 0; height: 0; }

.toggle-track {
  position: absolute;
  inset: 0;
  background: #cbd5e1;
  border-radius: 20px;
  cursor: pointer;
  transition: background .2s;
}

.toggle-track::after {
  content: '';
  position: absolute;
  left: 3px;
  top: 3px;
  width: 14px;
  height: 14px;
  background: #fff;
  border-radius: 50%;
  transition: transform .2s;
  box-shadow: 0 1px 3px rgba(0,0,0,.2);
}

.toggle-switch input:checked + .toggle-track {
  background: var(--primary);
}

.toggle-switch input:checked + .toggle-track::after {
  transform: translateX(16px);
}
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
      <label for="sg-site-name">Site Name</label>
      <input type="text" id="sg-site-name" name="site_name" value="Synaptic Signals">
      <small>Shown in the browser tab and outbound emails.</small>
    </div>
    <div class="form-group">
      <label for="sg-admin-email">Admin Email</label>
      <input type="email" id="sg-admin-email" name="admin_email" value="admin@example.com">
      <small>Used as the reply-to address for system notifications.</small>
    </div>

    <p class="settings-section-title">Localisation</p>
    <div class="form-group">
      <label for="sg-timezone">Timezone</label>
      <select id="sg-timezone" name="timezone">
        <option value="UTC" selected>UTC</option>
        <option value="America/New_York">America/New_York</option>
        <option value="America/Chicago">America/Chicago</option>
        <option value="America/Denver">America/Denver</option>
        <option value="America/Los_Angeles">America/Los_Angeles</option>
        <option value="Europe/London">Europe/London</option>
        <option value="Europe/Paris">Europe/Paris</option>
        <option value="Asia/Tokyo">Asia/Tokyo</option>
        <option value="Australia/Sydney">Australia/Sydney</option>
      </select>
    </div>
    <div class="form-group">
      <label for="sg-date-format">Date Format</label>
      <input type="text" id="sg-date-format" name="date_format" value="%B %-d, %Y">
      <small>Uses chrono format strings. Example: <code>%B %-d, %Y</code> → January 1, 2026</small>
    </div>

    <p class="settings-section-title">Content</p>
    <div class="form-group">
      <label for="sg-ppp">Posts Per Page</label>
      <input type="number" id="sg-ppp" name="posts_per_page" min="1" max="100" value="10" style="width:100px">
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
  <p style="color:var(--muted);font-size:.875rem;font-style:italic;margin:0 0 1.5rem">
    Advanced settings — coming soon. Upload size limit and additional options
    will be available here. Max upload size will default to 25 MB.
  </p>
</div>

<script>
(function () {
  var tabs    = document.querySelectorAll('.settings-tab-btn');
  var panels  = document.querySelectorAll('.settings-panel');

  function activate(tabName) {
    tabs.forEach(function (btn) {
      var on = btn.dataset.tab === tabName;
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-selected', on ? 'true' : 'false');
    });
    panels.forEach(function (panel) {
      panel.classList.toggle('active', panel.id === 'tab-' + tabName);
    });
    // Persist across page loads.
    try { sessionStorage.setItem('settings-tab', tabName); } catch (e) {}
  }

  tabs.forEach(function (btn) {
    btn.addEventListener('click', function () { activate(btn.dataset.tab); });
  });

  // Restore last active tab.
  try {
    var saved = sessionStorage.getItem('settings-tab');
    if (saved) activate(saved);
  } catch (e) {}
}());
</script>
"#;
    crate::admin_page("System Settings", "/admin/settings", flash, content, ctx)
}
