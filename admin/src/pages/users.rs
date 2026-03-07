//! Admin user management page.

/// Map a stored role value to a human-readable display label.
fn role_display(role: &str) -> &str {
    match role {
        "super_admin" => "Super Admin",
        "site_admin"  => "Site Admin",
        "admin"       => "Admin",
        "editor"      => "Editor",
        "author"      => "Author",
        "subscriber"  => "Subscriber",
        other         => other,
    }
}

/// Map a role value to an extra badge CSS class for colour coding.
fn role_badge_class(role: &str) -> &str {
    match role {
        "super_admin" => "badge-super-admin",
        "site_admin"  => "badge-site-admin",
        "admin"       => "badge-admin",
        _             => "",
    }
}

pub struct SiteOption {
    pub id: String,
    pub hostname: String,
    /// UUID of the current non-super_admin site owner, if one exists.
    /// Used to drive the displacement modal on the site access page.
    pub existing_admin_id: Option<String>,
    /// Display name of the existing site admin (for the modal message).
    pub existing_admin_name: Option<String>,
}

pub struct UserRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub display_name: String,
    pub is_protected: bool,
    /// True when the user's global role is super_admin.
    /// Used to hide the site-access button regardless of site role display.
    pub is_super_admin: bool,
    /// Site hostnames this user belongs to. Populated for both staff and subscribers.
    pub site_hostnames: Vec<String>,
    /// Site UUIDs parallel to site_hostnames. Used to render switch-site links for admins.
    pub site_ids: Vec<String>,
}

pub struct UserEdit {
    pub id: Option<String>,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub bio: String,
    /// Available sites for assignment — only populated on the new-user form for global admins.
    pub sites: Vec<SiteOption>,
    /// True when editing an existing super_admin — role field becomes read-only.
    pub is_super_admin_target: bool,
}

pub fn render_list(
    staff: &[UserRow],
    subscribers: &[UserRow],
    flash: Option<&str>,
    current_user_id: &str,
    can_manage_access: bool,
    active_tab: &str,
    available_sites: &[SiteOption],
    selected_site_id: &str,
    ctx: &crate::PageContext,
) -> String {
    let is_subscribers = active_tab == "subscribers";

    // ── Tab bar ───────────────────────────────────────────────────────────────
    let staff_active = if !is_subscribers { " active" } else { "" };
    let sub_active   = if  is_subscribers { " active" } else { "" };
    let tabs = format!(
        r#"<div class="page-tabs">
  <a href="/admin/users?tab=site-users" class="page-tab{staff_active}">Site Users <span class="badge" style="margin-left:.35rem;font-size:.75rem;padding:.1rem .45rem">{staff_count}</span></a>
  <a href="/admin/users?tab=subscribers" class="page-tab{sub_active}">Subscribers <span class="badge" style="margin-left:.35rem;font-size:.75rem;padding:.1rem .45rem">{sub_count}</span></a>
</div>"#,
        staff_active = staff_active,
        sub_active   = sub_active,
        staff_count  = staff.len(),
        sub_count    = subscribers.len(),
    );

    // ── Site filter dropdowns (global admin only) ─────────────────────────────
    let site_filter_staff = if ctx.is_global_admin && !available_sites.is_empty() {
        let opts = available_sites.iter().map(|s| {
            let sel = if s.id == selected_site_id { " selected" } else { "" };
            format!(
                r#"<option value="{id}"{sel}>{hostname}</option>"#,
                id = crate::html_escape(&s.id),
                hostname = crate::html_escape(&s.hostname),
                sel = sel,
            )
        }).collect::<Vec<_>>().join("\n");
        format!(
            r#"<form method="GET" action="/admin/users" style="display:inline-flex;align-items:center;margin:0">
  <input type="hidden" name="tab" value="site-users">
  <select name="site" onchange="this.form.submit()" style="height:2.25rem;padding:0 .5rem;border:1px solid var(--border,#e5e7eb);border-radius:6px;font-size:.875rem;background:#fff;cursor:pointer">
    <option value="">All Sites</option>
    {opts}
  </select>
</form>"#,
            opts = opts,
        )
    } else {
        String::new()
    };

    let site_filter_subs = if ctx.is_global_admin && !available_sites.is_empty() {
        let opts = available_sites.iter().map(|s| {
            let sel = if s.id == selected_site_id { " selected" } else { "" };
            format!(
                r#"<option value="{id}"{sel}>{hostname}</option>"#,
                id = crate::html_escape(&s.id),
                hostname = crate::html_escape(&s.hostname),
                sel = sel,
            )
        }).collect::<Vec<_>>().join("\n");
        format!(
            r#"<form method="GET" action="/admin/users" style="display:inline-flex;align-items:center;margin:0">
  <input type="hidden" name="tab" value="subscribers">
  <select name="site" onchange="this.form.submit()" style="height:2.25rem;padding:0 .5rem;border:1px solid var(--border,#e5e7eb);border-radius:6px;font-size:.875rem;background:#fff;cursor:pointer">
    <option value="">All Sites</option>
    {opts}
  </select>
</form>"#,
            opts = opts,
        )
    } else {
        String::new()
    };

    // ── Site Users table ──────────────────────────────────────────────────────
    let staff_rows = staff.iter().map(|u| {
        let site_access_btn = if can_manage_access && !u.is_super_admin {
            format!(
                r#"<a href="/admin/users/{id}/site-access" class="icon-btn" title="Manage site access">
                  <img src="/admin/static/icons/users.svg" alt="Site Access">
                </a>"#,
                id = crate::html_escape(&u.id),
            )
        } else {
            String::new()
        };
        let delete_btn = if u.id != current_user_id && !u.is_protected {
            let warn_msg = format!(
                "Delete user \\u2018{}\\u2019? This will permanently delete all their posts and pages. This cannot be undone.",
                u.display_name.replace('\'', "\\'"),
            );
            format!(
                r#"<form method="POST" action="/admin/users/{id}/delete" style="display:inline" data-confirm="{warn_msg}" onsubmit="return confirm(this.dataset.confirm)">
                  <input type="hidden" name="tab" value="site-users">
                  <button class="icon-btn icon-danger" title="Delete user" type="submit">
                    <img src="/admin/static/icons/delete.svg" alt="Delete">
                  </button>
                </form>"#,
                id = crate::html_escape(&u.id),
                warn_msg = crate::html_escape(&warn_msg),
            )
        } else {
            String::new()
        };
        let cb = if u.id != current_user_id && !u.is_protected {
            format!(
                r#"<input type="checkbox" class="bulk-cb-staff" value="{}" aria-label="Select">"#,
                crate::html_escape(&u.id),
            )
        } else {
            String::new()
        };
        let domain_badges = if u.site_hostnames.is_empty() {
            r#"<span style="display:inline-block;background:#fed7aa;color:#c2410c;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap">Unassigned</span>"#.to_string()
        } else if can_manage_access {
            u.site_hostnames.iter().zip(u.site_ids.iter()).map(|(h, sid)| {
                format!(
                    r#"<form method="POST" action="/admin/sites/switch" style="display:inline;margin:.1rem .15rem .1rem 0">
                      <input type="hidden" name="site_id" value="{sid}">
                      <button type="submit" title="Switch to {h}" style="display:inline-block;background:#e2e8f0;color:#64748b;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap;border:none;cursor:pointer;font-family:inherit;line-height:1.4">
                        {h}
                      </button>
                    </form>"#,
                    sid = crate::html_escape(sid),
                    h = crate::html_escape(h),
                )
            }).collect::<Vec<_>>().join("")
        } else {
            u.site_hostnames.iter().map(|h| {
                format!(
                    r#"<span style="display:inline-block;background:#e2e8f0;color:#64748b;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;margin:.1rem .15rem .1rem 0;white-space:nowrap">{}</span>"#,
                    crate::html_escape(h),
                )
            }).collect::<Vec<_>>().join("")
        };
        format!(
            r#"<tr>
              <td style="width:2rem;text-align:center">{cb}</td>
              <td><a href="/admin/users/{id}/edit">{display_name}</a></td>
              <td>{username}</td>
              <td>{email}</td>
              <td>{domain_badges}</td>
              <td><span class="badge {badge_class}">{role}</span></td>
              <td class="actions">
                <a href="/admin/users/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
                {site_access_btn}
                {delete_btn}
              </td>
            </tr>"#,
            cb = cb,
            id = crate::html_escape(&u.id),
            display_name = crate::html_escape(&u.display_name),
            username = crate::html_escape(&u.username),
            email = crate::html_escape(&u.email),
            domain_badges = domain_badges,
            role = crate::html_escape(role_display(&u.role)),
            badge_class = role_badge_class(&u.role),
            site_access_btn = site_access_btn,
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    // ── Subscribers table ─────────────────────────────────────────────────────
    let sub_rows = subscribers.iter().map(|u| {
        let delete_btn = if u.id != current_user_id && !u.is_protected {
            let warn_msg = format!(
                "Delete subscriber \\u2018{}\\u2019? This cannot be undone.",
                u.display_name.replace('\'', "\\'"),
            );
            format!(
                r#"<form method="POST" action="/admin/users/{id}/delete" style="display:inline" data-confirm="{warn_msg}" onsubmit="return confirm(this.dataset.confirm)">
                  <input type="hidden" name="tab" value="subscribers">
                  <button class="icon-btn icon-danger" title="Delete" type="submit">
                    <img src="/admin/static/icons/delete.svg" alt="Delete">
                  </button>
                </form>"#,
                id = crate::html_escape(&u.id),
                warn_msg = crate::html_escape(&warn_msg),
            )
        } else {
            String::new()
        };
        let domain_badges = if u.site_hostnames.is_empty() {
            r#"<span style="color:var(--muted);font-size:0.8rem">—</span>"#.to_string()
        } else {
            u.site_hostnames.iter().map(|h| {
                format!(
                    r#"<span style="display:inline-block;background:#e2e8f0;color:#64748b;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;margin:.1rem .15rem .1rem 0;white-space:nowrap">{}</span>"#,
                    crate::html_escape(h),
                )
            }).collect::<Vec<_>>().join("")
        };
        let cb = if u.id != current_user_id && !u.is_protected {
            format!(
                r#"<input type="checkbox" class="bulk-cb-subs" value="{}" aria-label="Select">"#,
                crate::html_escape(&u.id),
            )
        } else {
            String::new()
        };
        format!(
            r#"<tr>
              <td style="width:2rem;text-align:center">{cb}</td>
              <td><a href="/admin/users/{id}/edit">{display_name}</a></td>
              <td>{username}</td>
              <td>{email}</td>
              <td>{domain_badges}</td>
              <td class="actions">
                <a href="/admin/users/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
                {delete_btn}
              </td>
            </tr>"#,
            cb = cb,
            id = crate::html_escape(&u.id),
            display_name = crate::html_escape(&u.display_name),
            username = crate::html_escape(&u.username),
            email = crate::html_escape(&u.email),
            domain_badges = domain_badges,
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    // Shared bulk-delete + select-all script (handles both tabs).
    let bulk_script = r#"<script>
(function() {
  var selAll = document.getElementById('select-all-staff');
  var btnS   = document.getElementById('bulk-delete-btn-staff');
  var cntS   = document.getElementById('bulk-count-staff');
  function syncStaff() {
    var n = document.querySelectorAll('.bulk-cb-staff:checked').length;
    if (btnS) { btnS.style.display = n > 0 ? '' : 'none'; }
    if (cntS) { cntS.textContent = n; }
  }
  if (selAll) {
    selAll.addEventListener('change', function() {
      document.querySelectorAll('.bulk-cb-staff').forEach(function(c) { c.checked = selAll.checked; });
      syncStaff();
    });
  }
  document.querySelectorAll('.bulk-cb-staff').forEach(function(c) {
    c.addEventListener('change', function() { if (!c.checked && selAll) selAll.checked = false; syncStaff(); });
  });

  var selAllS = document.getElementById('select-all-subs');
  var btnSu   = document.getElementById('bulk-delete-btn-subs');
  var cntSu   = document.getElementById('bulk-count-subs');
  function syncSubs() {
    var n = document.querySelectorAll('.bulk-cb-subs:checked').length;
    if (btnSu) { btnSu.style.display = n > 0 ? '' : 'none'; }
    if (cntSu) { cntSu.textContent = n; }
  }
  if (selAllS) {
    selAllS.addEventListener('change', function() {
      document.querySelectorAll('.bulk-cb-subs').forEach(function(c) { c.checked = selAllS.checked; });
      syncSubs();
    });
  }
  document.querySelectorAll('.bulk-cb-subs').forEach(function(c) {
    c.addEventListener('change', function() { if (!c.checked && selAllS) selAllS.checked = false; syncSubs(); });
  });
})();

function bulkDeleteUsers(tab) {
  var cls = tab === 'subscribers' ? '.bulk-cb-subs:checked' : '.bulk-cb-staff:checked';
  var checked = document.querySelectorAll(cls);
  if (!checked.length) return;
  if (!confirm('Delete ' + checked.length + ' user(s)? This cannot be undone.')) return;
  var ids = Array.from(checked).map(function(c) { return c.value; }).join(',');
  var f = document.createElement('form');
  f.method = 'POST'; f.action = '/admin/users/bulk-delete';
  [['ids', ids], ['tab', tab]].forEach(function(pair) {
    var i = document.createElement('input');
    i.type = 'hidden'; i.name = pair[0]; i.value = pair[1];
    f.appendChild(i);
  });
  document.body.appendChild(f);
  f.submit();
}
</script>"#;

    let content = if !is_subscribers {
        let empty_msg = if staff.is_empty() {
            r#"<tr><td colspan="7" style="text-align:center;color:var(--muted);padding:2rem">No users yet.</td></tr>"#
        } else { "" };
        format!(
            r#"{tabs}
<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <a href="/admin/users/new" class="btn btn-primary">New User</a>
  {site_filter_staff}
  <button id="bulk-delete-btn-staff" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDeleteUsers('site-users')">Delete Selected (<span id="bulk-count-staff">0</span>)</button>
</div>
<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all-staff" title="Select all" aria-label="Select all"></th>
    <th>Name</th><th>Username</th><th>Email</th><th>Domain</th><th>Role</th><th>Actions</th>
  </tr></thead>
  <tbody>{staff_rows}{empty_msg}</tbody>
</table>
{bulk_script}"#,
            tabs = tabs,
            site_filter_staff = site_filter_staff,
            staff_rows = staff_rows,
            empty_msg = empty_msg,
            bulk_script = bulk_script,
        )
    } else {
        let empty_msg = if subscribers.is_empty() {
            r#"<tr><td colspan="6" style="text-align:center;color:var(--muted);padding:2rem">No subscribers yet.</td></tr>"#
        } else { "" };
        format!(
            r#"{tabs}
<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  {site_filter_subs}
  <button id="bulk-delete-btn-subs" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDeleteUsers('subscribers')">Delete Selected (<span id="bulk-count-subs">0</span>)</button>
</div>
<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all-subs" title="Select all" aria-label="Select all"></th>
    <th>Name</th><th>Username</th><th>Email</th><th>Domain</th><th>Actions</th>
  </tr></thead>
  <tbody>{sub_rows}{empty_msg}</tbody>
</table>
{bulk_script}"#,
            tabs = tabs,
            site_filter_subs = site_filter_subs,
            sub_rows = sub_rows,
            empty_msg = empty_msg,
            bulk_script = bulk_script,
        )
    };

    crate::admin_page("Users", "/admin/users", flash, &content, ctx)
}

pub fn render_editor(user: &UserEdit, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let title = if user.id.is_none() {
        "New User"
    } else if user.role == "subscriber" {
        "Edit Subscriber"
    } else {
        "Edit User"
    };
    let action = match &user.id {
        Some(id) => format!("/admin/users/{}/edit", id),
        None => "/admin/users/new".to_string(),
    };

    // Role field: read-only display for super_admin targets; dropdown for everyone else.
    // Global admin creates/edits site-scoped users using site role values (admin/editor/author/subscriber).
    // "admin" here means site_users.role = 'admin' (site admin), NOT users.role = 'super_admin'.
    let is_new = user.id.is_none();
    let role_field = if user.is_super_admin_target {
        r#"<div class="form-group">
  <label>Role</label>
  <p style="margin:0;padding:0.4rem 0">Super Admin</p>
  <input type="hidden" name="role" value="super_admin">
</div>"#.to_string()
    } else {
        let roles: &[(&str, &str)] = if ctx.is_global_admin {
            &[
                ("admin",       "Site Admin"),
                ("editor",      "Editor"),
                ("author",      "Author"),
                ("subscriber",  "Subscriber"),
            ]
        } else {
            &[
                ("editor",     "Editor"),
                ("author",     "Author"),
                ("subscriber", "Subscriber"),
            ]
        };
        // On new-user form: prepend a disabled placeholder; on edit: pre-select current role.
        let placeholder = if is_new {
            r#"<option value="" disabled selected>Select Role</option>"#.to_string()
        } else {
            String::new()
        };
        let role_options = roles.iter().map(|(value, label)| {
            let selected = if !is_new && *value == user.role { " selected" } else { "" };
            format!(r#"<option value="{value}"{selected}>{label}</option>"#)
        }).collect::<Vec<_>>().join("");

        if is_new {
            // New user: plain dropdown, no lock needed.
            format!(r#"<div class="form-group">
  <label for="role">Role</label>
  <select id="role" name="role" required>{placeholder}{role_options}</select>
</div>"#)
        } else {
            // Edit: lock the dropdown behind a checkbox to prevent accidental role changes.
            // A hidden input always submits the current role; the checkbox + select
            // override it only when the admin explicitly opts in.
            let current_role = crate::html_escape(&user.role);
            format!(r#"<div class="form-group">
  <label for="role-enable">Role</label>
  <input type="hidden" id="role-hidden" name="role" value="{current_role}">
  <div style="display:flex;align-items:center;gap:0.6rem;margin-bottom:0.4rem">
    <input type="checkbox" id="role-enable" onchange="
      var sel = document.getElementById('role-select');
      var hid = document.getElementById('role-hidden');
      sel.disabled = !this.checked;
      hid.disabled = this.checked;
    ">
    <label for="role-enable" style="font-weight:400;margin:0;cursor:pointer">Change role</label>
  </div>
  <select id="role-select" name="role" disabled>{role_options}</select>
  <small>Role is locked to prevent accidental changes. Check the box above to edit it.</small>
</div>"#)
        }
    };

    let password_hint = if user.id.is_some() {
        r#"<small>Leave blank to keep the current password.</small>"#
    } else {
        ""
    };

    // Site-assignment section — shown for new users when the admin has sites to offer.
    // Global admin: always shown (can also create new sites).
    // Site admin: only shown when they own 2+ sites (single-site admins auto-assign).
    // Both see the same UI; the dropdown is populated with their respective site list.
    let site_section = if is_new && (ctx.is_global_admin || !user.sites.is_empty()) {
        let site_opts = user.sites.iter().map(|s| {
            format!(
                r#"<option value="{}">{}</option>"#,
                crate::html_escape(&s.id),
                crate::html_escape(&s.hostname),
            )
        }).collect::<Vec<_>>().join("\n");
        format!(r#"
<div class="form-group" style="margin-top:0.5rem">
  <label>Site Assignment</label>
  <div style="display:flex;gap:1.5rem;margin:0.4rem 0 0.75rem;flex-wrap:wrap">
    <label class="radio-label">
      <input type="radio" name="site_assignment" value="none" checked onchange="toggleSiteFields()"> None
    </label>
    <label class="radio-label">
      <input type="radio" name="site_assignment" value="existing" onchange="toggleSiteFields()"> Existing site
    </label>
    <label class="radio-label">
      <input type="radio" name="site_assignment" value="new" onchange="toggleSiteFields()"> New site
    </label>
  </div>
  <div id="site-existing" style="display:none">
    <select name="existing_site_id" id="site-existing-select">
      <option value="" disabled selected>Select Site</option>
      {site_opts}
    </select>
  </div>
  <div id="site-new" style="display:none">
    <input type="text" name="new_hostname" id="new-hostname-input" placeholder="example.com">
    <small id="hostname-hint" style="color:#dc2626;display:none">Must be a valid domain (e.g. example.com, my-site.com, sub.example.com)</small>
    <small>The domain this site will respond to (e.g. client.example.com)</small>
  </div>
</div>
<script>
function toggleSiteFields() {{
  var val = document.querySelector('input[name="site_assignment"]:checked').value;
  document.getElementById('site-existing').style.display = val === 'existing' ? '' : 'none';
  document.getElementById('site-new').style.display     = val === 'new'      ? '' : 'none';
}}
</script>"#,
            site_opts = site_opts,
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"<style>
.user-form-grid {{
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0 1.25rem;
}}
.user-form-grid .form-group {{ margin-bottom: 1rem; }}
.user-form-grid .span-2 {{ grid-column: 1 / -1; }}
@media (max-width: 540px) {{
  .user-form-grid {{ grid-template-columns: 1fr; }}
  .user-form-grid .span-2 {{ grid-column: 1; }}
}}
</style>
<div class="profile-container">
  <h2>{form_title}</h2>
  <form method="POST" action="{action}" style="max-width:580px">
    <div class="user-form-grid">
      <div class="form-group">
        <label for="username">Username</label>
        <input type="text" id="username" name="username" value="{username}" required autocomplete="off"{autofocus}>
      </div>
      <div class="form-group">
        <label for="display_name">Full Name</label>
        <input type="text" id="display_name" name="display_name" value="{display_name}" required autocomplete="off">
      </div>
      <div class="form-group">
        <label for="email">Email</label>
        <input type="email" id="email" name="email" value="{email}" required autocomplete="off">
        <small id="email-hint" style="color:#dc2626;display:none">Please enter a valid email address.</small>
      </div>
      <div class="form-group">
        <label for="password">Password</label>
        <input type="password" id="password" name="password" autocomplete="new-password">
        {password_hint}
      </div>
      <div class="form-group">
        {role_field_inner}
      </div>
      {site_section}
    </div>
    <div class="form-note" style="margin-bottom:1.25rem">
      <p><strong>Password requirements:</strong></p>
      <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
        <li id="pw-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>8–12 characters</li>
        <li id="pw-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one uppercase letter</li>
        <li id="pw-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one number</li>
        <li id="pw-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one symbol: ! @ # $ % &amp;</li>
      </ul>
    </div>
    <div style="display:flex;gap:0.75rem">
      <button type="submit" id="save-btn" class="btn btn-primary"{save_disabled}>Save</button>
      <a href="/admin/users" class="btn btn-secondary">Cancel</a>
    </div>
  </form>
<script>
(function () {{
  var form = document.querySelector('form[action="{action}"]');
  if (!form) return;
  var pwInput = form.querySelector('#password');
  var isNew   = {is_new_js};
  form.addEventListener('submit', function (e) {{
    // Role-change confirmation — only fires when the checkbox is checked.
    var roleCheckbox = form.querySelector('#role-enable');
    if (roleCheckbox && roleCheckbox.checked) {{
      var sel = form.querySelector('#role-select');
      var newRole = sel ? sel.options[sel.selectedIndex].text : '';
      if (!confirm('Role change: set to "' + newRole + '". Continue?')) {{
        e.preventDefault(); window.location.reload(); return;
      }}
    }}
    var pw = pwInput ? pwInput.value : '';
    if (!pw && !isNew) return; // blank on edit = keep current, no validation needed
    if (!pw && isNew) {{ e.preventDefault(); alert('Password is required.'); return; }}
    var err = validatePw(pw);
    if (err) {{ e.preventDefault(); alert(err); }}
  }});
  function validatePw(pw) {{
    if (pw.length < 8)  return 'Password must be at least 8 characters.';
    if (pw.length > 12) return 'Password must be no more than 12 characters.';
    if (!/[A-Z]/.test(pw))       return 'Password must contain at least one uppercase letter.';
    if (!/[0-9]/.test(pw))       return 'Password must contain at least one number.';
    if (!/[!@#$%&]/.test(pw))    return 'Password must contain at least one symbol: ! @ # $ % &';
    return null;
  }}
  // ── Real-time validation (new user form only) ────────────────────────────
  if (isNew) {{
    var saveBtn = document.getElementById('save-btn');

    // Password requirements checklist.
    var pwReqs = [
      {{ id: 'pw-req-len',   test: function(p) {{ return p.length >= 8 && p.length <= 12; }} }},
      {{ id: 'pw-req-upper', test: function(p) {{ return /[A-Z]/.test(p); }} }},
      {{ id: 'pw-req-num',   test: function(p) {{ return /[0-9]/.test(p); }} }},
      {{ id: 'pw-req-sym',   test: function(p) {{ return /[!@#$%&]/.test(p); }} }},
    ];
    var updateFeedback = function() {{
      var pw = pwInput ? pwInput.value : '';
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
      // Email hint — show only when field has content but is invalid.
      var emailEl = document.getElementById('email');
      var emailVal = emailEl ? emailEl.value.trim() : '';
      var hint = document.getElementById('email-hint');
      if (hint) {{
        hint.style.display = (emailVal && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(emailVal)) ? '' : 'none';
      }}
      // Hostname hint — show when "new site" is selected and value is not a valid domain.
      var assignEl = document.querySelector('input[name="site_assignment"]:checked');
      var hnEl = document.getElementById('new-hostname-input');
      var hnHint = document.getElementById('hostname-hint');
      if (hnHint && hnEl) {{
        var hnVal = hnEl.value.trim();
        hnHint.style.display = (assignEl && assignEl.value === 'new' && hnVal && !isValidHostname(hnVal)) ? '' : 'none';
      }}
    }};
    function isValidHostname(h) {{
      return /^(?:[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?\.)+[a-z]{{2,}}$/i.test(h);
    }}

    var checkComplete = function() {{
      var unameEl = document.getElementById('username');
      var dnameEl = document.getElementById('display_name');
      var emailEl = document.getElementById('email');
      var uname = unameEl ? unameEl.value.trim() : '';
      var dname = dnameEl ? dnameEl.value.trim() : '';
      var email = emailEl ? emailEl.value.trim() : '';
      var pw    = pwInput ? pwInput.value : '';
      if (!uname || !dname || !email || !pw) return false;
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return false;
      if (validatePw(pw)) return false;
      var roleEl = document.getElementById('role');
      if (roleEl && !roleEl.value) return false;
      var assign = document.querySelector('input[name="site_assignment"]:checked');
      if (assign && assign.value === 'existing') {{
        var siteSel = document.getElementById('site-existing-select');
        if (!siteSel || !siteSel.value) return false;
      }} else if (assign && assign.value === 'new') {{
        var hnInput = document.querySelector('input[name="new_hostname"]');
        if (!hnInput || !hnInput.value.trim()) return false;
        if (!isValidHostname(hnInput.value.trim())) return false;
      }}
      return true;
    }};
    var syncSaveBtn = function() {{
      updateFeedback();
      if (saveBtn) saveBtn.disabled = !checkComplete();
    }};
    ['username', 'display_name', 'email', 'password'].forEach(function(fid) {{
      var el = document.getElementById(fid);
      if (el) el.addEventListener('input', syncSaveBtn);
    }});
    var roleEl = document.getElementById('role');
    if (roleEl) roleEl.addEventListener('change', syncSaveBtn);
    document.querySelectorAll('input[name="site_assignment"]').forEach(function(r) {{
      r.addEventListener('change', syncSaveBtn);
    }});
    var siteSel = document.getElementById('site-existing-select');
    if (siteSel) siteSel.addEventListener('change', syncSaveBtn);
    var hnInput = document.querySelector('input[name="new_hostname"]');
    if (hnInput) hnInput.addEventListener('input', syncSaveBtn);
    syncSaveBtn();
  }}
}}());
</script>
</div>"#,
        form_title        = title,
        action            = action,
        username          = crate::html_escape(&user.username),
        display_name      = crate::html_escape(&user.display_name),
        email             = crate::html_escape(&user.email),
        role_field_inner  = role_field,
        site_section      = site_section,
        password_hint     = password_hint,
        is_new_js         = if is_new { "true" } else { "false" },
        autofocus         = if is_new { " autofocus" } else { "" },
        save_disabled     = if is_new { " disabled" } else { "" },
    );

    crate::admin_page(title, "/admin/users", flash, &content, ctx)
}

// ── Site access management ──────────────────────────────────────────────────

pub struct SiteAssignmentRow {
    pub site_id: String,
    pub hostname: String,
    pub role: String,
}

pub struct SiteAccessData {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    /// Current site assignments for this user.
    pub assignments: Vec<SiteAssignmentRow>,
    /// Sites the acting admin can assign this user to (their owned/managed sites).
    pub available_sites: Vec<SiteOption>,
}

pub fn render_site_access(
    data: &SiteAccessData,
    flash: Option<&str>,
    ctx: &crate::PageContext,
) -> String {
    let assignment_rows = if data.assignments.is_empty() {
        "<tr><td colspan=\"3\"><em>No site assignments yet.</em></td></tr>".to_string()
    } else {
        data.assignments.iter().map(|a| {
            format!(
                r#"<tr>
                  <td>{hostname}</td>
                  <td><span class="badge">{role}</span></td>
                  <td class="actions">
                    <form method="post" action="/admin/users/{user_id}/site-access/remove" style="display:inline"
                          data-confirm="Remove {hostname} from site access?" onsubmit="return confirm(this.dataset.confirm)">
                      <input type="hidden" name="site_id" value="{site_id}">
                      <button type="submit" class="icon-btn icon-danger" title="Remove from site">
                        <img src="/admin/static/icons/trash-2.svg" alt="Remove">
                      </button>
                    </form>
                  </td>
                </tr>"#,
                user_id   = crate::html_escape(&data.user_id),
                site_id   = crate::html_escape(&a.site_id),
                hostname  = crate::html_escape(&a.hostname),
                role      = crate::html_escape(role_display(&a.role)),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let site_options = data.available_sites.iter().map(|s| {
        let existing_id   = s.existing_admin_id.as_deref().unwrap_or("");
        let existing_name = s.existing_admin_name.as_deref().unwrap_or("");
        format!(
            r#"<option value="{id}" data-existing-admin-id="{eid}" data-existing-admin-name="{ename}">{hostname}</option>"#,
            id       = crate::html_escape(&s.id),
            hostname = crate::html_escape(&s.hostname),
            eid      = crate::html_escape(existing_id),
            ename    = crate::html_escape(existing_name),
        )
    }).collect::<Vec<_>>().join("\n");

    let add_form = if data.available_sites.is_empty() {
        "<p><em>No sites available to assign.</em></p>".to_string()
    } else {
        format!(
            r#"<form id="site-access-form" method="post" action="/admin/users/{user_id}/site-access/add"
  style="display:flex;gap:0.75rem;align-items:flex-end;flex-wrap:wrap;margin-top:1.5rem">
  <input type="hidden" name="displaced_action" id="displaced-action-field" value="">
  <div class="form-group" style="margin:0;flex:1;min-width:160px">
    <label>Site</label>
    <select name="site_id" id="site-select" style="width:100%">{site_opts}</select>
  </div>
  <div class="form-group" style="margin:0;flex:1;min-width:140px">
    <label>Role</label>
    <select name="role" id="role-select" style="width:100%">
      {site_admin_opt}
      <option value="editor">Editor</option>
      <option value="author">Author</option>
    </select>
  </div>
  <div class="form-group" style="margin:0">
    <button type="submit" class="btn btn-primary">Assign</button>
  </div>
</form>

<!-- Displacement confirmation modal -->
<div id="displace-modal" style="display:none;position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,0.5);align-items:center;justify-content:center">
  <div style="background:#fff;border-radius:8px;padding:2rem;max-width:480px;width:90%;box-shadow:0 8px 32px rgba(0,0,0,0.18)">
    <h3 style="margin-top:0;color:var(--danger,#dc2626)">Replace Site Admin?</h3>
    <p id="displace-msg" style="margin-bottom:1.5rem"></p>
    <p style="font-size:0.9rem;color:var(--muted)">Their posts and media will remain attributed to them. Choose what happens to their site access:</p>
    <div style="display:flex;flex-direction:column;gap:0.75rem;margin:1.25rem 0">
      <label style="display:flex;align-items:flex-start;gap:0.6rem;cursor:pointer;padding:0.75rem;border:1.5px solid var(--border,#e5e7eb);border-radius:6px">
        <input type="radio" name="displace_choice" value="remove" style="margin-top:0.2rem;flex-shrink:0" checked>
        <span><strong>Remove from site</strong><br><span style="font-size:0.875rem;color:var(--muted)">They lose all access immediately. Recommended if you no longer trust them.</span></span>
      </label>
      <label style="display:flex;align-items:flex-start;gap:0.6rem;cursor:pointer;padding:0.75rem;border:1.5px solid var(--border,#e5e7eb);border-radius:6px">
        <input type="radio" name="displace_choice" value="demote_author" style="margin-top:0.2rem;flex-shrink:0">
        <span><strong>Demote to Author</strong><br><span style="font-size:0.875rem;color:var(--muted)">They keep read and write access to their own posts only.</span></span>
      </label>
    </div>
    <div style="display:flex;justify-content:flex-end;gap:0.75rem;margin-top:1.5rem">
      <button type="button" id="displace-cancel" class="btn btn-secondary">Cancel</button>
      <button type="button" id="displace-confirm" class="btn btn-danger">Confirm &amp; Assign</button>
    </div>
  </div>
</div>

<script>
(function() {{
  var form     = document.getElementById('site-access-form');
  var modal    = document.getElementById('displace-modal');
  var msgEl    = document.getElementById('displace-msg');
  var actionFld= document.getElementById('displaced-action-field');
  var cancelBtn= document.getElementById('displace-cancel');
  var confirmBtn=document.getElementById('displace-confirm');
  var roleSelect = document.getElementById('role-select');
  var siteSelect = document.getElementById('site-select');

  form.addEventListener('submit', function(e) {{
    if (roleSelect.value !== 'site_admin') return; // no modal needed
    var opt = siteSelect.options[siteSelect.selectedIndex];
    var existingId   = opt.dataset.existingAdminId   || '';
    var existingName = opt.dataset.existingAdminName || '';
    if (!existingId) return; // no existing site admin — proceed normally
    e.preventDefault();
    var siteName = opt.text;
    msgEl.innerHTML = '<strong>' + escHtml(existingName) + '</strong> is currently the Site Admin for <strong>' + escHtml(siteName) + '</strong>. Assigning <strong>{assignee}</strong> will replace them.';
    modal.style.display = 'flex';
  }});

  cancelBtn.addEventListener('click', function() {{
    modal.style.display = 'none';
    actionFld.value = '';
  }});

  confirmBtn.addEventListener('click', function() {{
    var choice = document.querySelector('input[name="displace_choice"]:checked');
    actionFld.value = choice ? choice.value : 'remove';
    modal.style.display = 'none';
    form.submit();
  }});

  // Close on backdrop click.
  modal.addEventListener('click', function(e) {{
    if (e.target === modal) {{
      modal.style.display = 'none';
      actionFld.value = '';
    }}
  }});

  function escHtml(s) {{
    return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }}
}})();
</script>"#,
            user_id        = crate::html_escape(&data.user_id),
            site_opts      = site_options,
            assignee       = crate::html_escape(&data.display_name),
            site_admin_opt = if ctx.is_global_admin {
                r#"<option value="site_admin">Site Admin (owner)</option>"#
            } else {
                ""
            },
        )
    };

    let content = format!(
        r#"<p><a href="/admin/users">&larr; Back to Users</a></p>
<h2 style="margin-bottom:0.25rem">{display_name}</h2>
<p style="margin-top:0;color:var(--muted)">{email}</p>
<h3>Current Site Access</h3>
<table class="data-table">
  <thead><tr><th>Site</th><th>Role</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
<h3>Add to a Site</h3>
{add_form}"#,
        display_name = crate::html_escape(&data.display_name),
        email        = crate::html_escape(&data.email),
        rows         = assignment_rows,
        add_form     = add_form,
    );

    crate::admin_page(
        &format!("Site Access — {}", data.display_name),
        "/admin/users",
        flash,
        &content,
        ctx,
    )
}

