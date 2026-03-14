//! Admin sites management page.

pub struct SiteRow {
    pub id: String,
    pub hostname: String,
    /// Email of the site_admin who owns this site, if one is assigned.
    pub admin_email: Option<String>,
    /// Count of non-subscriber users (site_admin, editor, author).
    pub user_count: i64,
    /// Count of subscribers only.
    pub subscriber_count: i64,
    pub post_count: i64,
    pub page_count: i64,
    /// True for the first site created during CLI install — cannot be deleted.
    pub is_default: bool,
    /// True when the current user may edit settings / delete this site.
    pub can_manage: bool,
    /// True when a Caddy block exists for this hostname (SSL provisioned).
    pub ssl_active: bool,
}

pub fn render_list(
    sites: &[SiteRow],
    flash: Option<&str>,
    can_create: bool,
    ctx: &crate::PageContext,
) -> String {
    let rows = sites.iter().map(|s| {
        let manage_html = if s.can_manage {
            let delete_html = if s.is_default {
                String::new()
            } else {
                let confirm_msg = format!(
                    "Delete site '{}'? This will permanently delete all its content, media records, settings, and user assignments. This cannot be undone.",
                    s.hostname.replace('\'', "\\'")
                );
                format!(
                    r#"<form method="post" action="/admin/sites/{id}/delete" style="display:inline"
                          data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
                      <button type="submit" class="icon-btn icon-danger" title="Delete site">
                        <img src="/admin/static/icons/trash-2.svg" alt="Delete">
                      </button>
                    </form>"#,
                    id = crate::html_escape(&s.id),
                    confirm_msg = crate::html_escape(&confirm_msg),
                )
            };
            format!(
                r#"<a href="/admin/sites/{id}/settings" class="icon-btn" title="Site Settings">
                  <img src="/admin/static/icons/edit.svg" alt="Site Settings">
                </a>
                {delete}"#,
                id = crate::html_escape(&s.id),
                delete = delete_html,
            )
        } else {
            String::new()
        };

        let ssl_badge = if s.ssl_active {
            r#"<span class="ssl-badge ssl-active" title="SSL active — Caddy block provisioned">
                 <img src="/admin/static/icons/lock.svg" alt="SSL active" style="width:14px;height:14px;vertical-align:middle;filter:invert(35%) sepia(80%) saturate(500%) hue-rotate(95deg)">
               </span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/admin/sites/{id}/provision-ssl" style="display:inline"
                        onsubmit="return confirm('Add SSL (Caddy block) for {hostname_js}?\n\nEnsure DNS for this domain points to this server before proceeding.\nCaddy will provision the Let\'s Encrypt certificate automatically.')">
                     <button type="submit" class="ssl-badge ssl-inactive" title="SSL not provisioned — click to set up">
                       <img src="/admin/static/icons/lock.svg" alt="Provision SSL" style="width:14px;height:14px;vertical-align:middle;opacity:0.4">
                     </button>
                   </form>"#,
                id          = crate::html_escape(&s.id),
                hostname_js = crate::html_escape(&s.hostname),
            )
        };

        format!(
            r#"<tr>
              <td>{hostname}{default_badge} {ssl_badge}</td>
              <td style="color:var(--muted);font-size:0.875rem">{admin_email}</td>
              <td><span style="display:inline-block;background:#f3f4f6;color:#374151;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{user_count}</span></td>
              <td><span style="display:inline-block;background:#f3f4f6;color:#374151;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{subscriber_count}</span></td>
              <td><span style="display:inline-block;background:#f3f4f6;color:#374151;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{post_count}</span></td>
              <td><span style="display:inline-block;background:#f3f4f6;color:#374151;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{page_count}</span></td>
              <td class="actions">
                <form method="post" action="/admin/sites/switch" style="display:inline">
                  <input type="hidden" name="site_id" value="{id}">
                  <button type="submit" class="icon-btn" title="Switch to this site">
                    <img src="/admin/static/icons/log-in.svg" alt="Switch">
                  </button>
                </form>
                {manage}
              </td>
            </tr>"#,
            id               = crate::html_escape(&s.id),
            hostname         = crate::html_escape(&s.hostname),
            default_badge    = if s.is_default { r#" <span class="badge-visiting" title="Primary domain — cannot be deleted">system domain</span>"# } else { "" },
            ssl_badge        = ssl_badge,
            admin_email      = s.admin_email.as_deref().map(|e| crate::html_escape(e)).unwrap_or_else(|| "<em>none</em>".to_string()),
            user_count       = s.user_count,
            subscriber_count = s.subscriber_count,
            post_count       = s.post_count,
            page_count       = s.page_count,
            manage           = manage_html,
        )
    }).collect::<Vec<_>>().join("\n");

    let new_site_btn = if can_create {
        r#"<p style="margin-bottom:1rem"><a href="/admin/sites/new" class="btn btn-primary">New Site</a></p>"#
    } else {
        ""
    };

    let content = format!(
        r#"{new_site_btn}<table class="data-table">
  <thead><tr><th>Hostname</th><th>Admin</th><th>Users</th><th>Subs</th><th>Posts</th><th>Pages</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        new_site_btn = new_site_btn,
        rows = rows,
    );

    crate::admin_page("Sites", "/admin/sites", flash, &content, ctx)
}

pub struct SiteSettingsData {
    pub id: String,
    pub hostname: String,
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = format!(
        r#"<p class="site-context-banner">Settings for: <strong>{hostname}</strong></p>
<form method="post" action="/admin/sites/{id}/settings" class="edit-form" id="hostname-form">
  <div class="form-group">
    <label for="hostname">Hostname</label>
    <input type="text" id="hostname-input" name="hostname" value="{hostname}" required>
  </div>
  <div class="form-actions">
    <button type="button" id="hostname-save-btn" class="btn btn-primary"
            onclick="showHostnameModal()" disabled>Save</button>
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
</form>

<!-- Hostname change checklist modal -->
<div id="hostname-modal" role="dialog" aria-modal="true" aria-labelledby="hm-title"
     style="display:none;position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,.45);
            align-items:center;justify-content:center;" hidden>
  <div style="background:#fff;border-radius:8px;padding:1.75rem 2rem;max-width:480px;width:90%;
              box-shadow:0 8px 32px rgba(0,0,0,.18);">
    <h2 id="hm-title" style="margin:0 0 .25rem;font-size:1.1rem;">Before changing the hostname</h2>
    <p style="margin:.25rem 0 1.25rem;color:#6b7280;font-size:.875rem;">
      Changing to <strong id="hm-new-hostname" style="color:#111827"></strong> takes effect
      immediately. Please confirm the following before continuing:
    </p>
    <ul style="margin:0 0 1.5rem;padding-left:1.25rem;display:flex;flex-direction:column;gap:.6rem;
               font-size:.875rem;color:#374151;list-style:none;padding:0;">
      <li class="hm-check-item">
        <label style="display:flex;align-items:flex-start;gap:.6rem;cursor:pointer;">
          <input type="checkbox" class="hm-check" style="margin-top:.15rem;flex-shrink:0;accent-color:#2b6cb0;">
          <span><strong>DNS</strong> — the new domain's A/CNAME record points to this server.</span>
        </label>
      </li>
      <li class="hm-check-item">
        <label style="display:flex;align-items:flex-start;gap:.6rem;cursor:pointer;">
          <input type="checkbox" class="hm-check" style="margin-top:.15rem;flex-shrink:0;accent-color:#2b6cb0;">
          <span><strong>Reverse proxy</strong> — Caddy (or your proxy) is configured to serve the new hostname.</span>
        </label>
      </li>
      <li class="hm-check-item">
        <label style="display:flex;align-items:flex-start;gap:.6rem;cursor:pointer;">
          <input type="checkbox" class="hm-check" style="margin-top:.15rem;flex-shrink:0;accent-color:#2b6cb0;">
          <span><strong>Local dev</strong> — if this is a local domain, <code>/etc/hosts</code> is updated.</span>
        </label>
      </li>
      <li class="hm-check-item">
        <label style="display:flex;align-items:flex-start;gap:.6rem;cursor:pointer;">
          <input type="checkbox" class="hm-check" style="margin-top:.15rem;flex-shrink:0;accent-color:#2b6cb0;">
          <span><strong>SSL</strong> — Caddy will provision a certificate automatically once DNS resolves.</span>
        </label>
      </li>
    </ul>
    <p style="margin:0 0 1.25rem;padding:.65rem .85rem;background:#fffbeb;border:1px solid #fcd34d;
              border-radius:6px;font-size:.8rem;color:#92400e;line-height:1.5;">
      Your admin session stays active during the transition. Navigate to
      <strong id="hm-new-link"></strong> when the new domain is live.
    </p>
    <div style="display:flex;justify-content:flex-end;gap:.75rem;">
      <button type="button" class="btn btn-secondary" onclick="closeHostnameModal()">Go back</button>
      <button type="button" id="hm-confirm-btn" class="btn btn-primary"
              onclick="submitHostnameForm()" disabled>Save hostname</button>
    </div>
  </div>
</div>

<script>
  function showHostnameModal() {{
    var newHost = document.getElementById('hostname-input').value.trim();
    if (!newHost) return;
    document.getElementById('hm-new-hostname').textContent = newHost;
    document.getElementById('hm-new-link').textContent = 'http://' + newHost + '/admin';
    // Reset checkboxes and confirm button each time the modal opens.
    document.querySelectorAll('.hm-check').forEach(function(cb) {{ cb.checked = false; }});
    updateConfirmBtn();
    var modal = document.getElementById('hostname-modal');
    modal.hidden = false;
    modal.style.display = 'flex';
  }}
  function closeHostnameModal() {{
    var modal = document.getElementById('hostname-modal');
    modal.hidden = true;
    modal.style.display = 'none';
  }}
  function updateConfirmBtn() {{
    var all = Array.from(document.querySelectorAll('.hm-check'));
    document.getElementById('hm-confirm-btn').disabled = !all.every(function(cb) {{ return cb.checked; }});
  }}
  function submitHostnameForm() {{
    closeHostnameModal();
    document.getElementById('hostname-form').submit();
  }}
  document.querySelectorAll('.hm-check').forEach(function(cb) {{
    cb.addEventListener('change', updateConfirmBtn);
  }});
  // Enable Save button only when the hostname has actually changed.
  var originalHostname = document.getElementById('hostname-input').value;
  document.getElementById('hostname-input').addEventListener('input', function() {{
    document.getElementById('hostname-save-btn').disabled =
      this.value.trim() === originalHostname.trim();
  }});
</script>

<p class="site-context-banner" style="margin-top:2rem">Site Settings</p>
<form method="post" action="/admin/sites/{id}/site-config" class="edit-form">
  <div class="form-group">
    <label for="site_name">Site Name</label>
    <input type="text" id="site_name" name="site_name" value="{site_name}" required>
    <small>The display name shown in the browser tab, header, and footer.</small>
  </div>
  <div class="form-group">
    <label for="site_description">Site Description</label>
    <textarea id="site_description" name="site_description" rows="3">{site_description}</textarea>
  </div>
  <div class="form-group">
    <label for="language">Language</label>
    <input type="text" id="language" name="language" value="{language}">
  </div>
  <div class="form-group">
    <label for="posts_per_page">Posts Per Page</label>
    <input type="number" id="posts_per_page" name="posts_per_page" value="{posts_per_page}" min="1" max="100">
  </div>
  <div class="form-group">
    <label for="date_format">Date Format</label>
    <input type="text" id="date_format" name="date_format" value="{date_format}">
    <small>Uses chrono format strings, e.g. "%B %-d, %Y" &rarr; January 1, 2026</small>
  </div>
  <button type="submit" class="btn btn-primary">Save Settings</button>
</form>"#,
        id = crate::html_escape(&data.id),
        hostname = crate::html_escape(&data.hostname),
        site_name = crate::html_escape(&data.site_name),
        site_description = crate::html_escape(&data.site_description),
        language = crate::html_escape(&data.language),
        posts_per_page = data.posts_per_page,
        date_format = crate::html_escape(&data.date_format),
    );

    crate::admin_page("Edit Hostname", "/admin/sites", flash, &content, ctx)
}

pub fn render_new(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = r#"<form method="post" action="/admin/sites" class="edit-form" id="new-site-form">
  <div class="form-group">
    <label for="hostname">Domain Name</label>
    <input type="text" id="hostname" name="hostname" required placeholder="example.com" autofocus
           oninput="hnUpdate()">
    <small>The domain this site will respond to</small>
  </div>
  <div class="form-note" style="margin-bottom:1.25rem">
    <p><strong>Domain requirements:</strong></p>
    <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
      <li id="hn-req-dot"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Contains at least one dot (e.g. example<strong>.com</strong>)</li>
      <li id="hn-req-tld"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>TLD is 2 or more letters (e.g. .com, .io, .co.uk)</li>
      <li id="hn-req-chars"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Letters, numbers, and hyphens only — no spaces or symbols</li>
      <li id="hn-req-hyphen"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>No label starts or ends with a hyphen</li>
    </ul>
  </div>
  <div class="form-actions">
    <button type="submit" id="create-btn" class="btn btn-primary" disabled>Create Site</button>
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
</form>
<script>
(function() {
  var hnReqs = [
    { id: 'hn-req-dot',    test: function(h) { return h.indexOf('.') !== -1; } },
    { id: 'hn-req-tld',    test: function(h) { var tld = h.split('.').pop(); return tld.length >= 2 && /^[a-z]+$/i.test(tld); } },
    { id: 'hn-req-chars',  test: function(h) { return /^[a-z0-9.\-]+$/i.test(h); } },
    { id: 'hn-req-hyphen', test: function(h) { return h.split('.').every(function(l) { return l.length > 0 && !l.startsWith('-') && !l.endsWith('-'); }); } },
  ];

  window.hnUpdate = function() {
    var val = document.getElementById('hostname').value.trim();
    var allPass = val.length > 0;
    hnReqs.forEach(function(req) {
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!val) {
        li.style.color = ''; if (dot) dot.textContent = '·';
        allPass = false;
      } else if (req.test(val)) {
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      } else {
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
        allPass = false;
      }
    });
    document.getElementById('create-btn').disabled = !allPass;
  };
})();
</script>"#;

    crate::admin_page("New Site", "/admin/sites", flash, content, ctx)
}
