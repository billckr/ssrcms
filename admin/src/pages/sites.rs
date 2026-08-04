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
    /// True when this site is the default_site_id of its non-super_admin owner.
    /// Shown as a blue "primary domain" badge in the super-admin system view only.
    pub is_primary_domain: bool,
    /// True when maintenance mode is currently on for this site.
    pub maintenance_mode: bool,
}

fn sites_pagination(page: i64, total_pages: i64, search_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let prev = if page > 1 {
        format!(r#"<a href="/admin/sites?page={}{search_qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="/admin/sites?page={}{search_qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(r#"<a href="/admin/sites?page={p}{search_qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Table + pagination only — swapped by the live-search JS, and reused for
/// the initial full-page render so both paths render identically.
pub fn sites_list_fragment(sites: &[SiteRow], page: i64, total_pages: i64, search: &str, ctx: &crate::PageContext) -> String {
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };
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

        // SSL status/provisioning is only shown to roles that can manage sites
        // (super_admin, site_admin) — editors and authors have no use for it and
        // the provision-ssl route itself is already restricted to those roles.
        let ssl_badge = if !ctx.can_manage_sites {
            String::new()
        } else if s.ssl_active {
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

        let maintenance_badge = if s.maintenance_mode {
            r#" <span class="ssl-badge" title="Maintenance mode is ON — visitors see a maintenance page">
                 <img src="/admin/static/icons/tool.svg" alt="Maintenance mode active" style="width:14px;height:14px;vertical-align:middle;filter:invert(60%) sepia(90%) saturate(600%) hue-rotate(2deg)">
               </span>"#
        } else {
            ""
        };

        let site_url = format!(
            "{scheme}://{hostname}",
            scheme = if s.ssl_active { "https" } else { "http" },
            hostname = s.hostname,
        );

        format!(
            r#"<tr>
              <td><a href="{site_url}" target="_blank" rel="noopener noreferrer">{hostname}</a>{default_badge} {ssl_badge}{maintenance_badge}</td>
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
                {users_link}
                {manage}
              </td>
            </tr>"#,
            id               = crate::html_escape(&s.id),
            hostname         = crate::html_escape(&s.hostname),
            site_url         = crate::html_escape(&site_url),
            default_badge    = if s.is_default {
                r#" <span class="badge-visiting" title="Primary domain — cannot be deleted">system</span>"#
            } else if s.is_primary_domain {
                r#" <span class="badge-primary-domain" title="Primary domain for this account">primary</span>"#
            } else {
                ""
            },
            ssl_badge        = ssl_badge,
            maintenance_badge = maintenance_badge,
            users_link       = if ctx.can_manage_users {
                format!(
                    r#"<a href="/admin/users?site={id}" class="icon-btn" title="View users for this site">
                  <img src="/admin/static/icons/users.svg" alt="Users">
                </a>"#,
                    id = crate::html_escape(&s.id),
                )
            } else {
                String::new()
            },
            admin_email      = s.admin_email.as_deref().map(|e| crate::html_escape(e)).unwrap_or_else(|| "<em>none</em>".to_string()),
            user_count       = s.user_count,
            subscriber_count = s.subscriber_count,
            post_count       = s.post_count,
            page_count       = s.page_count,
            manage           = manage_html,
        )
    }).collect::<Vec<_>>().join("\n");

    let rows = if rows.is_empty() {
        r#"<tr><td colspan="7" class="empty-state">No sites found.</td></tr>"#.to_string()
    } else {
        rows
    };

    format!(
        r#"<table class="data-table">
  <thead><tr><th>Site</th><th>Admin</th><th>Users</th><th>Subs</th><th>Posts</th><th>Pages</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}"#,
        rows = rows,
        pagination = sites_pagination(page, total_pages, &search_qs),
    )
}

pub fn render_list(
    sites: &[SiteRow],
    flash: Option<&str>,
    can_create: bool,
    page: i64,
    total_pages: i64,
    search: &str,
    ctx: &crate::PageContext,
) -> String {
    let new_site_btn = if can_create {
        r#"<a href="/admin/sites/new" class="icon-btn" title="New Site" aria-label="New Site"><img src="/admin/static/icons/file-plus.svg" alt=""></a>"#
    } else {
        ""
    };

    let fragment = sites_list_fragment(sites, page, total_pages, search, ctx);
    let live_search = crate::live_search_script("site-search", "sites-list", "/admin/sites?partial=1");

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <div class="icon-search-box" style="margin-bottom:0">
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
    <input id="site-search" type="search" placeholder="Search sites&hellip;" value="{search_val}" autocomplete="off">
  </div>
  <div style="display:flex;align-items:center;gap:.5rem">
    {new_site_btn}
  </div>
</div>
<div id="sites-list">{fragment}</div>
{live_search}"#,
        search_val = crate::html_escape(search),
        new_site_btn = new_site_btn,
        fragment = fragment,
        live_search = live_search,
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
    pub maintenance_mode: bool,
    pub maintenance_message: String,
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = format!(
        r#"<div style="max-width:720px">
<div class="profile-container">
  <p style="margin:0;padding:.65rem .85rem;background:#f8fafc;border:1px solid #e2e8f0;
            border-radius:6px;font-size:.875rem;color:#475569;line-height:1.6;">
    To rename the domain, use the CLI from the server:<br>
    <code style="font-size:.8rem;background:#f1f5f9;padding:.15rem .4rem;border-radius:4px;">synap-cli site rename --id {id} --hostname &lt;new-domain&gt;</code>
  </p>
</div>

<div class="profile-container">
  <h2>Settings</h2>
  <form method="post" action="/admin/sites/{id}/site-config" class="edit-form" id="site-settings-form">
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
    <button type="submit" id="save-settings-btn" class="btn btn-primary" disabled>Save Settings</button>
  </form>
</div>
<script>
(function() {{
  var form   = document.getElementById('site-settings-form');
  var saveBtn = document.getElementById('save-settings-btn');
  var fields = Array.prototype.slice.call(form.querySelectorAll('input, textarea'));
  var initial = fields.map(function(f) {{ return f.value; }});

  function syncSaveBtn() {{
    var changed = fields.some(function(f, i) {{ return f.value !== initial[i]; }});
    saveBtn.disabled = !changed;
  }}

  fields.forEach(function(f) {{
    f.addEventListener('input', syncSaveBtn);
  }});
}})();
</script>

<div class="profile-container">
  <h2>Maintenance Mode</h2>
  <p style="margin:0 0 1rem;font-size:.875rem;color:var(--muted)">
    Shows a maintenance page to visitors of this site. Takes effect immediately &mdash; no
    restart needed. <code>/admin/*</code> always stays reachable so you can turn it back off.
  </p>
  <form method="post" action="/admin/sites/{id}/maintenance" class="edit-form" id="maintenance-form"
        onsubmit="return confirm(document.getElementById('maintenance_mode').checked
          ? 'Enable maintenance mode? Visitors will see a maintenance page immediately.'
          : 'Disable maintenance mode? The site will be reachable by visitors again.')">
    <div class="form-group">
      <label style="display:inline;font-weight:400">
        <input type="checkbox" id="maintenance_mode" name="maintenance_mode" style="display:inline;width:auto;height:auto"{maintenance_checked}>
        Enable maintenance mode
      </label>
    </div>
    <div class="form-group">
      <label for="maintenance_message">Message</label>
      <textarea id="maintenance_message" name="maintenance_message" rows="3">{maintenance_message}</textarea>
    </div>
    <button type="submit" id="save-maintenance-btn" class="btn btn-primary" disabled>Save Maintenance</button>
  </form>
</div>
<script>
(function() {{
  var form = document.getElementById('maintenance-form');
  var btn  = document.getElementById('save-maintenance-btn');
  function snapshot() {{
    return Array.from(new FormData(form).entries()).map(function (e) {{ return e[0] + '=' + e[1]; }}).join('&');
  }}
  var initialSnapshot = snapshot();
  function checkChanged() {{
    btn.disabled = snapshot() === initialSnapshot;
  }}
  form.addEventListener('input', checkChanged);
  form.addEventListener('change', checkChanged);
}})();
</script>
</div>"#,
        id = crate::html_escape(&data.id),
        site_name = crate::html_escape(&data.site_name),
        site_description = crate::html_escape(&data.site_description),
        language = crate::html_escape(&data.language),
        posts_per_page = data.posts_per_page,
        date_format = crate::html_escape(&data.date_format),
        maintenance_checked = if data.maintenance_mode { " checked" } else { "" },
        maintenance_message = crate::html_escape(&data.maintenance_message),
    );

    crate::admin_page(&format!("Site Settings - {}", data.hostname), "/admin/sites", flash, &content, ctx)
}

/// An existing user selectable as the new site's admin.
pub struct UserOption {
    pub id: String,
    pub label: String,
}

pub struct NewSiteData {
    /// Preserved on validation failure so the admin doesn't have to retype it.
    pub hostname: String,
    /// "none" | "existing" | "new" — which Site Admin sub-form was active.
    pub user_assignment: String,
    pub existing_user_id: String,
    pub new_username: String,
    pub new_email: String,
    pub new_display_name: String,
    /// Assignable users (excludes super_admins, who can't be scoped to a site).
    pub existing_users: Vec<UserOption>,
}

impl Default for NewSiteData {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            user_assignment: "none".to_string(),
            existing_user_id: String::new(),
            new_username: String::new(),
            new_email: String::new(),
            new_display_name: String::new(),
            existing_users: Vec::new(),
        }
    }
}

pub fn render_new(data: &NewSiteData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let checked = |val: &str| if data.user_assignment == val { " checked" } else { "" };
    let existing_opts = data.existing_users.iter().map(|u| {
        let sel = if data.existing_user_id == u.id { " selected" } else { "" };
        format!(
            r#"<option value="{id}"{sel}>{label}</option>"#,
            id    = crate::html_escape(&u.id),
            label = crate::html_escape(&u.label),
            sel   = sel,
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<div class="card-boxed">
  <h2 class="card-boxed-header" style="display:flex;align-items:center;justify-content:space-between;gap:.5rem;">
    <span>New Site</span>
    <button type="submit" form="new-site-form" id="create-btn" class="icon-btn" title="Create Site" aria-label="Create Site" disabled>
      <img src="/admin/static/icons/file-plus.svg" alt="">
    </button>
  </h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites" class="edit-form" id="new-site-form" style="max-width:580px">
  <div class="form-group">
    <label for="hostname">Domain Name</label>
    <input type="text" id="hostname" name="hostname" required placeholder="example.com" autofocus
           value="{hostname}" oninput="hnUpdate()">
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

  <div class="form-group">
    <label>Site Admin</label>
    <div style="display:flex;gap:1.5rem;margin:0.4rem 0 0.75rem;flex-wrap:wrap">
      <label class="radio-label">
        <input type="radio" name="user_assignment" value="none"{none_checked} onchange="toggleUserFields()"> Assign later
      </label>
      <label class="radio-label">
        <input type="radio" name="user_assignment" value="existing"{existing_checked} onchange="toggleUserFields()"> Existing user
      </label>
      <label class="radio-label">
        <input type="radio" name="user_assignment" value="new"{new_checked} onchange="toggleUserFields()"> New user
      </label>
    </div>
    <div id="user-none">
      <small>You will be this site's admin and owner until you assign someone else.</small>
    </div>
    <div id="user-existing" style="display:none">
      <select name="existing_user_id" id="user-existing-select">
        <option value="" disabled selected>Select User</option>
        {existing_opts}
      </select>
      <small>The selected user will be the site admin.</small>
    </div>
    <div id="user-new" style="display:none">
      <div class="user-form-grid stacked">
        <div class="form-group">
          <label for="new_username">Username <span class="field-hint">(letters, numbers, hyphens only)</span></label>
          <input type="text" id="new_username" name="new_username" value="{new_username}" autocomplete="off"
                 pattern="[a-z0-9][a-z0-9\-]*[a-z0-9]|[a-z0-9]" title="Lowercase letters, numbers and hyphens only">
          <span id="new-username-hint" class="field-error" style="display:none">Only lowercase letters, numbers and hyphens allowed.</span>
        </div>
        <div class="form-group">
          <label for="new_display_name">Display Name</label>
          <input type="text" id="new_display_name" name="new_display_name" value="{new_display_name}" autocomplete="off">
        </div>
        <div class="form-group">
          <label for="new_email">Email</label>
          <input type="email" id="new_email" name="new_email" value="{new_email}" autocomplete="off">
          <small id="new-email-hint" style="color:#dc2626;display:none">Please enter a valid email address.</small>
        </div>
        <div class="form-group">
          <label for="new_password">Password</label>
          <input type="password" id="new_password" name="new_password" autocomplete="new-password">
        </div>
      </div>
      <div class="form-note" style="margin-bottom:1.25rem">
        <p><strong>New user requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="new-pw-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>8–12 characters</li>
          <li id="new-pw-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one uppercase letter</li>
          <li id="new-pw-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one number</li>
          <li id="new-pw-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one symbol: ! @ # $ % &amp;</li>
        </ul>
      </div>
      <small>A new account is created and assigned as this site's admin and owner.</small>
    </div>
  </div>

  <div class="form-actions">
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var hnReqs = [
    {{ id: 'hn-req-dot',    test: function(h) {{ return h.indexOf('.') !== -1; }} }},
    {{ id: 'hn-req-tld',    test: function(h) {{ var tld = h.split('.').pop(); return tld.length >= 2 && /^[a-z]+$/i.test(tld); }} }},
    {{ id: 'hn-req-chars',  test: function(h) {{ return /^[a-z0-9.\-]+$/i.test(h); }} }},
    {{ id: 'hn-req-hyphen', test: function(h) {{ return h.split('.').every(function(l) {{ return l.length > 0 && !l.startsWith('-') && !l.endsWith('-'); }}); }} }},
  ];
  var pwReqs = [
    {{ id: 'new-pw-req-len',   test: function(p) {{ return p.length >= 8 && p.length <= 12; }} }},
    {{ id: 'new-pw-req-upper', test: function(p) {{ return /[A-Z]/.test(p); }} }},
    {{ id: 'new-pw-req-num',   test: function(p) {{ return /[0-9]/.test(p); }} }},
    {{ id: 'new-pw-req-sym',   test: function(p) {{ return /[!@#$%&]/.test(p); }} }},
  ];
  var slugPattern = /^[a-z0-9][a-z0-9\-]*[a-z0-9]$|^[a-z0-9]$/;
  var usernameTouched = false;

  function isValidHostname(h) {{
    return /^(?:[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?\.)+[a-z]{{2,}}$/i.test(h);
  }}
  function isValidPassword(p) {{
    return p.length >= 8 && p.length <= 12 && /[A-Z]/.test(p) && /[0-9]/.test(p) && /[!@#$%&]/.test(p);
  }}
  function toSlug(s) {{
    return s.toLowerCase().replace(/[^a-z0-9\s-]/g, '').trim()
      .replace(/[\s]+/g, '-').replace(/-{{2,}}/g, '-').replace(/^-|-$/g, '');
  }}

  window.hnUpdate = function() {{
    var val = document.getElementById('hostname').value.trim();
    var allPass = val.length > 0;
    hnReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!val) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
        allPass = false;
      }} else if (req.test(val)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
        allPass = false;
      }}
    }});
    syncCreateBtn(allPass);
  }};

  function updateNewUserFeedback() {{
    var pw = document.getElementById('new_password').value;
    pwReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!pw) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (req.test(pw)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }});
    var uname = document.getElementById('new_username');
    var unameHint = document.getElementById('new-username-hint');
    if (unameHint) unameHint.style.display = (uname.value && !slugPattern.test(uname.value)) ? '' : 'none';
    var email = document.getElementById('new_email').value.trim();
    var emailHint = document.getElementById('new-email-hint');
    if (emailHint) emailHint.style.display = (email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) ? '' : 'none';
  }}

  function newUserComplete() {{
    var uname = document.getElementById('new_username').value.trim();
    var dname = document.getElementById('new_display_name').value.trim();
    var email = document.getElementById('new_email').value.trim();
    var pw    = document.getElementById('new_password').value;
    if (!uname || !dname || !email || !pw) return false;
    if (!slugPattern.test(uname)) return false;
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return false;
    if (!isValidPassword(pw)) return false;
    return true;
  }}

  function syncCreateBtn(hostnameOk) {{
    var assign = document.querySelector('input[name="user_assignment"]:checked').value;
    var userOk = true;
    if (assign === 'existing') {{
      userOk = !!document.getElementById('user-existing-select').value;
    }} else if (assign === 'new') {{
      updateNewUserFeedback();
      userOk = newUserComplete();
    }}
    document.getElementById('create-btn').disabled = !(hostnameOk && userOk);
  }}

  window.toggleUserFields = function() {{
    var val = document.querySelector('input[name="user_assignment"]:checked').value;
    document.getElementById('user-none').style.display     = val === 'none'     ? '' : 'none';
    document.getElementById('user-existing').style.display = val === 'existing' ? '' : 'none';
    document.getElementById('user-new').style.display      = val === 'new'      ? '' : 'none';
    hnUpdate();
  }};

  document.getElementById('user-existing-select').addEventListener('change', function() {{ hnUpdate(); }});
  ['new_email', 'new_password'].forEach(function(id) {{
    document.getElementById(id).addEventListener('input', function() {{ hnUpdate(); }});
  }});
  var unameEl = document.getElementById('new_username');
  var dnameEl = document.getElementById('new_display_name');
  unameEl.addEventListener('input', function() {{ usernameTouched = true; hnUpdate(); }});
  dnameEl.addEventListener('input', function() {{
    if (!usernameTouched) unameEl.value = toSlug(dnameEl.value);
    hnUpdate();
  }});

  toggleUserFields();
}})();
</script>"#,
        hostname            = crate::html_escape(&data.hostname),
        none_checked        = checked("none"),
        existing_checked    = checked("existing"),
        new_checked         = checked("new"),
        existing_opts       = existing_opts,
        new_username        = crate::html_escape(&data.new_username),
        new_email           = crate::html_escape(&data.new_email),
        new_display_name    = crate::html_escape(&data.new_display_name),
    );

    crate::admin_page("New Site", "/admin/sites", flash, &content, ctx)
}
