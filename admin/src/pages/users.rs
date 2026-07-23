//! Admin user management page.

/// Map a stored role value to a human-readable display label.
fn role_display(role: &str) -> &str {
    match role {
        "super_admin" => "Super Admin",
        "site_admin"  => "Site Admin",
        "admin"       => "Site Admin",
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
        "admin"       => "badge-site-admin",
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
    /// The user's default/primary site UUID. Used to highlight the primary domain badge.
    pub default_site_id: Option<String>,
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
    /// Current (hostname, role) site assignments for this user — display-only,
    /// shown in the Role section of the edit form. Empty on the new-user form.
    pub site_roles: Vec<(String, String)>,
}

/// Render the `<tr>` rows for the Site Users (staff) table.
fn build_staff_rows(staff: &[UserRow], current_user_id: &str, can_manage_access: bool) -> String {
    staff.iter().map(|u| {
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
                let is_primary = u.default_site_id.as_deref() == Some(sid.as_str());
                let (bg, fg) = if is_primary { ("#dbeafe", "#1e40af") } else { ("#e2e8f0", "#64748b") };
                format!(
                    r#"<form method="POST" action="/admin/sites/switch" style="display:inline;margin:.1rem .15rem .1rem 0">
                      <input type="hidden" name="site_id" value="{sid}">
                      <button type="submit" title="Switch to {h}" style="display:inline-block;background:{bg};color:{fg};border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap;border:none;cursor:pointer;font-family:inherit;line-height:1.4">
                        {h}
                      </button>
                    </form>"#,
                    sid = crate::html_escape(sid),
                    h = crate::html_escape(h),
                    bg = bg,
                    fg = fg,
                )
            }).collect::<Vec<_>>().join("")
        } else {
            u.site_hostnames.iter().zip(u.site_ids.iter()).map(|(h, sid)| {
                let is_primary = u.default_site_id.as_deref() == Some(sid.as_str());
                let (bg, fg) = if is_primary { ("#dbeafe", "#1e40af") } else { ("#e2e8f0", "#64748b") };
                format!(
                    r#"<span style="display:inline-block;background:{bg};color:{fg};border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;margin:.1rem .15rem .1rem 0;white-space:nowrap">{h}</span>"#,
                    bg = bg, fg = fg, h = crate::html_escape(h),
                )
            }).collect::<Vec<_>>().join("")
        };
        format!(
            r#"<tr>
              <td style="width:2rem;text-align:center">{cb}</td>
              <td><a href="/admin/users/{id}/edit">{display_name}</a></td>
              <td>{username}</td>
              <td><button type="button" class="copy-email-btn" data-email="{email_raw}" title="Click to copy email">{email}</button></td>
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
            email_raw = crate::html_escape(&u.email),
            domain_badges = domain_badges,
            role = crate::html_escape(role_display(&u.role)),
            badge_class = role_badge_class(&u.role),
            site_access_btn = site_access_btn,
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n")
}

/// Render the `<tr>` rows for the Subscribers table.
fn build_sub_rows(subscribers: &[UserRow], current_user_id: &str) -> String {
    subscribers.iter().map(|u| {
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
              <td><button type="button" class="copy-email-btn" data-email="{email_raw}" title="Click to copy email">{email}</button></td>
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
            email_raw = crate::html_escape(&u.email),
            domain_badges = domain_badges,
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n")
}

/// Build pagination controls for the users list.
/// Preserves `search_qs`/`site_qs` (each already prefixed with `&`) across page nav.
fn users_pagination(active_tab: &str, page: i64, total_pages: i64, search_qs: &str, site_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let base = format!("/admin/users?tab={}", active_tab);
    let qs = format!("{search_qs}{site_qs}");
    let prev = if page > 1 {
        format!(r#"<a href="{base}&page={}{qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="{base}&page={}{qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end   = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(r#"<a href="{base}&page={p}{qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Renders the full table (+ bottom pagination) for the active tab — the content of
/// `div#users-list`. Called by `render_list` on full page loads and returned directly
/// for `?partial=1` JS live-search requests so the browser can swap the whole table
/// (rows + pagination) without a full reload. `staff`/`subscribers` are expected to
/// already be sliced to the current page.
pub fn users_list_fragment(
    staff: &[UserRow],
    subscribers: &[UserRow],
    current_user_id: &str,
    can_manage_access: bool,
    active_tab: &str,
    search: &str,
    page: i64,
    total_pages: i64,
) -> String {
    let search_qs = if search.is_empty() { String::new() } else { format!("&search={}", crate::html_escape(search)) };
    let pagination = users_pagination(active_tab, page, total_pages, &search_qs, "");

    if active_tab != "subscribers" {
        let rows = build_staff_rows(staff, current_user_id, can_manage_access);
        let empty_msg = if staff.is_empty() {
            let msg = if search.is_empty() { "No users yet." } else { "No users matched your search." };
            format!(r#"<tr><td colspan="7" style="text-align:center;color:var(--muted);padding:2rem">{}</td></tr>"#, msg)
        } else {
            String::new()
        };
        format!(
            r#"<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all-staff" title="Select all" aria-label="Select all"></th>
    <th>Name</th><th>Username</th><th>Email</th><th>Domain</th><th>Role</th><th>Actions</th>
  </tr></thead>
  <tbody id="users-tbody">{rows}{empty_msg}</tbody>
</table>
{pagination}"#
        )
    } else {
        let rows = build_sub_rows(subscribers, current_user_id);
        let empty_msg = if subscribers.is_empty() {
            let msg = if search.is_empty() { "No subscribers yet." } else { "No subscribers matched your search." };
            format!(r#"<tr><td colspan="6" style="text-align:center;color:var(--muted);padding:2rem">{}</td></tr>"#, msg)
        } else {
            String::new()
        };
        format!(
            r#"<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all-subs" title="Select all" aria-label="Select all"></th>
    <th>Name</th><th>Username</th><th>Email</th><th>Domain</th><th>Actions</th>
  </tr></thead>
  <tbody id="users-tbody">{rows}{empty_msg}</tbody>
</table>
{pagination}"#
        )
    }
}

pub fn render_list(
    staff: &[UserRow],
    subscribers: &[UserRow],
    staff_total: i64,
    sub_total: i64,
    page: i64,
    total_pages: i64,
    flash: Option<&str>,
    current_user_id: &str,
    can_manage_access: bool,
    active_tab: &str,
    available_sites: &[SiteOption],
    selected_site_id: &str,
    search: &str,
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
        staff_count  = staff_total,
        sub_count    = sub_total,
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
            r#"<form method="GET" action="/admin/users" style="display:inline-flex;align-items:center;gap:.5rem;margin:0">
  <input type="hidden" name="tab" value="site-users">
  <select id="site-filter-staff" name="site" onchange="this.form.submit()" aria-label="Filter users by site" style="height:2.25rem;padding:0 .5rem;border:1px solid var(--border,#e5e7eb);border-radius:6px;font-size:.875rem;background:#fff;cursor:pointer">
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
            r#"<form method="GET" action="/admin/users" style="display:inline-flex;align-items:center;gap:.5rem;margin:0">
  <input type="hidden" name="tab" value="subscribers">
  <select id="site-filter-subs" name="site" onchange="this.form.submit()" aria-label="Filter users by site" style="height:2.25rem;padding:0 .5rem;border:1px solid var(--border,#e5e7eb);border-radius:6px;font-size:.875rem;background:#fff;cursor:pointer">
    <option value="">All Sites</option>
    {opts}
  </select>
</form>"#,
            opts = opts,
        )
    } else {
        String::new()
    };

    // Shared bulk-delete + select-all script (handles both tabs).
    // Uses event delegation throughout — rows in tbody#users-tbody are replaced
    // wholesale by the live-search fetch, so listeners can't be bound once at load.
    let bulk_script = r#"<script>
(function() {
  function syncGroup(cbClass, btnId, cntId, selAllId) {
    var checked = document.querySelectorAll('.' + cbClass + ':checked');
    var total = document.querySelectorAll('.' + cbClass).length;
    var btn = document.getElementById(btnId);
    var cnt = document.getElementById(cntId);
    if (cnt) cnt.textContent = checked.length;
    if (btn) btn.style.display = checked.length > 0 ? '' : 'none';
    var sa = document.getElementById(selAllId);
    if (sa) {
      sa.indeterminate = checked.length > 0 && checked.length < total;
      sa.checked = total > 0 && checked.length === total;
    }
  }

  document.addEventListener('change', function(e) {
    if (e.target.classList.contains('bulk-cb-staff')) {
      syncGroup('bulk-cb-staff', 'bulk-delete-btn-staff', 'bulk-count-staff', 'select-all-staff');
    } else if (e.target.classList.contains('bulk-cb-subs')) {
      syncGroup('bulk-cb-subs', 'bulk-delete-btn-subs', 'bulk-count-subs', 'select-all-subs');
    } else if (e.target.id === 'select-all-staff') {
      document.querySelectorAll('.bulk-cb-staff').forEach(function(c) { c.checked = e.target.checked; });
      syncGroup('bulk-cb-staff', 'bulk-delete-btn-staff', 'bulk-count-staff', 'select-all-staff');
    } else if (e.target.id === 'select-all-subs') {
      document.querySelectorAll('.bulk-cb-subs').forEach(function(c) { c.checked = e.target.checked; });
      syncGroup('bulk-cb-subs', 'bulk-delete-btn-subs', 'bulk-count-subs', 'select-all-subs');
    }
  });

  // Copy email to clipboard (delegated — buttons are recreated on live-search swaps)
  document.addEventListener('click', function(e) {
    var btn = e.target.closest('.copy-email-btn');
    if (!btn) return;
    e.preventDefault();
    var email = btn.getAttribute('data-email');

    // Try modern clipboard API first
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(email).then(function() {
        showCopyTooltip(btn);
      }).catch(function(err) {
        console.error('Clipboard failed:', err);
        fallbackCopy(email, btn);
      });
    } else {
      // Fallback for older browsers
      fallbackCopy(email, btn);
    }
  });

  function fallbackCopy(text, btn) {
    var textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand('copy');
      showCopyTooltip(btn);
    } catch (err) {
      console.error('Copy failed:', err);
    }
    document.body.removeChild(textarea);
  }

  function showCopyTooltip(btn) {
    var tooltip = document.createElement('div');
    tooltip.textContent = 'Copied!';
    tooltip.style.cssText = 'position:absolute;background:#16a34a;color:#fff;padding:.4rem .6rem;border-radius:4px;font-size:12px;white-space:nowrap;pointer-events:none;z-index:1000;box-shadow:0 2px 8px rgba(0,0,0,0.15)';

    document.body.appendChild(tooltip);

    var rect = btn.getBoundingClientRect();
    tooltip.style.left = (rect.left + rect.width / 2 - tooltip.offsetWidth / 2) + 'px';
    tooltip.style.top = (rect.top - 35) + 'px';

    setTimeout(function() {
      tooltip.style.opacity = '0';
      tooltip.style.transition = 'opacity 0.3s ease';
      setTimeout(function() {
        document.body.removeChild(tooltip);
      }, 300);
    }, 1500);
  }
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

    // ── Live search ──────────────────────────────────────────────────────────
    let search_input = format!(
        r#"<input id="user-search"
               type="search"
               placeholder="Search users&hellip;"
               value="{search_val}"
               style="margin-left:auto;width:100%;max-width:320px;padding:.4rem .75rem;border:1px solid var(--border);border-radius:4px;font-size:14px;background:var(--card-bg);color:inherit">"#,
        search_val = crate::html_escape(search),
    );
    let site_qs = if selected_site_id.is_empty() {
        String::new()
    } else {
        format!("&site={}", crate::html_escape(selected_site_id))
    };
    let fetch_prefix = format!("/admin/users?partial=1&tab={}{}", active_tab, site_qs);
    let live_search = crate::live_search_script("user-search", "users-list", &fetch_prefix);

    let fragment = users_list_fragment(staff, subscribers, current_user_id, can_manage_access, active_tab, search, page, total_pages);

    let content = if !is_subscribers {
        format!(
            r#"{tabs}
<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <a href="/admin/users/new" class="btn btn-primary">New User</a>
  {site_filter_staff}
  <button id="bulk-delete-btn-staff" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDeleteUsers('site-users')">Delete Selected (<span id="bulk-count-staff">0</span>)</button>
  {search_input}
</div>
<div id="users-list">{fragment}</div>
{bulk_script}
{live_search}"#,
            tabs = tabs,
            site_filter_staff = site_filter_staff,
            fragment = fragment,
            bulk_script = bulk_script,
            search_input = search_input,
            live_search = live_search,
        )
    } else {
        format!(
            r#"{tabs}
<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  {site_filter_subs}
  <button id="bulk-delete-btn-subs" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDeleteUsers('subscribers')">Delete Selected (<span id="bulk-count-subs">0</span>)</button>
  {search_input}
</div>
<div id="users-list">{fragment}</div>
{bulk_script}
{live_search}"#,
            tabs = tabs,
            site_filter_subs = site_filter_subs,
            fragment = fragment,
            bulk_script = bulk_script,
            search_input = search_input,
            live_search = live_search,
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

    // List of the user's current (hostname, role) site assignments — display-only,
    // rendered as a small table in the separate Role card on the edit form.
    let site_roles_list = if user.site_roles.is_empty() {
        String::new()
    } else {
        let rows = user.site_roles.iter().map(|(hostname, role)| {
            format!(
                r#"<tr><td>{hostname}</td><td><span class="badge {badge_class}">{role_label}</span></td></tr>"#,
                hostname = crate::html_escape(hostname),
                badge_class = role_badge_class(role),
                role_label = crate::html_escape(role_display(role)),
            )
        }).collect::<Vec<_>>().join("");
        format!(
            r#"<table class="data-table" style="margin-top:1rem;max-width:480px">
  <thead><tr><th>Site</th><th>Role</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#
        )
    };

    // Role field: read-only display for super_admin targets; dropdown for everyone else.
    // Global admin creates/edits site-scoped users using site role values (admin/editor/author/subscriber).
    // "admin" here means site_users.role = 'admin' (site admin), NOT users.role = 'super_admin'.
    let is_new = user.id.is_none();
    let role_field = if user.is_super_admin_target {
        if is_new {
            r#"<div class="form-group">
  <label>Role</label>
  <p style="margin:0;padding:0.4rem 0">Super Admin</p>
  <input type="hidden" name="role" value="super_admin">
</div>"#.to_string()
        } else {
            r#"<input type="hidden" name="role" value="super_admin">"#.to_string()
        }
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
            // Edit: role is read-only here. Site-scoped roles can only be changed
            // from /site-access, which shows exactly which site is affected and
            // warns before demoting a site's current admin/owner — this page has
            // no site picker, so an editable dropdown here was ambiguous about
            // which site's role it was actually changing.
            format!(r#"<input type="hidden" name="role" value="{current_role}">"#,
                current_role = crate::html_escape(&user.role),
            )
        }
    };

    // Separate "Role" card shown only on the edit form — current role (read-only
    // here; changed via /site-access) plus the user's existing site assignments.
    let role_section = if is_new {
        String::new()
    } else {
        format!(
            r#"<div class="profile-container">
  <h2>Role</h2>
  <div class="form-group">
    <label>Current Role</label>
    <p style="margin:0 0 0.5rem">{current_label}</p>
    <a href="/admin/users/{user_id}/site-access" class="btn btn-secondary">Change Role</a>
  </div>
  {site_roles_list}
</div>"#,
            current_label = crate::html_escape(role_display(&user.role)),
            user_id = crate::html_escape(user.id.as_deref().unwrap_or("")),
            site_roles_list = site_roles_list,
        )
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
        r#"<div class="profile-container">
  <h2>{form_title}</h2>
  <form method="POST" action="{action}" style="max-width:580px">
    <div class="user-form-grid">
      <div class="form-group">
        <label for="username">Username <span class="field-hint">(letters, numbers, hyphens only)</span></label>
        <input type="text" id="username" name="username" value="{username}" required autocomplete="off"
               pattern="[a-z0-9][a-z0-9\-]*[a-z0-9]|[a-z0-9]" title="Lowercase letters, numbers and hyphens only"{autofocus}>
        <span id="username-hint" class="field-error" style="display:none">Only lowercase letters, numbers and hyphens allowed.</span>
      </div>
      <div class="form-group">
        <label for="display_name">Display Name</label>
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
      <p><strong>New user requirements:</strong></p>
      <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
        <li id="pw-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>8–12 characters</li>
        <li id="pw-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one uppercase letter</li>
        <li id="pw-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one number</li>
        <li id="pw-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one symbol: ! @ # $ % &amp;</li>
        <li id="role-req"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Role selected</li>
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
      // Update role requirement
      var roleEl = document.getElementById('role');
      var roleHasValue = roleEl && roleEl.value && roleEl.value !== '';
      var roleLi = document.getElementById('role-req');
      var roleDot = roleLi ? roleLi.querySelector('.pw-dot') : null;
      if (roleLi) {{
        if (roleHasValue) {{
          roleLi.style.color = '#16a34a'; if (roleDot) roleDot.textContent = '✓';
        }} else {{
          roleLi.style.color = '#dc2626'; if (roleDot) roleDot.textContent = '✗';
        }}
      }}

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

    // Slugify a string to lowercase letters, numbers and hyphens.
    function toSlug(s) {{
      return s.toLowerCase()
        .replace(/[^a-z0-9\s-]/g, '')
        .trim()
        .replace(/[\s]+/g, '-')
        .replace(/-{{2,}}/g, '-')
        .replace(/^-|-$/g, '');
    }}
    var slugPattern = /^[a-z0-9][a-z0-9\-]*[a-z0-9]$|^[a-z0-9]$/;
    var unameEl = document.getElementById('username');
    var dnameEl = document.getElementById('display_name');
    var unameHint = document.getElementById('username-hint');
    var usernameTouched = false;
    if (unameEl) {{
      unameEl.addEventListener('input', function() {{
        usernameTouched = true;
        var slug = toSlug(unameEl.value);
        if (unameHint) unameHint.style.display = (unameEl.value && !slugPattern.test(unameEl.value)) ? '' : 'none';
        syncSaveBtn();
      }});
    }}
    // Auto-populate username from display name on new user form (until admin edits it manually).
    if (dnameEl && unameEl && {is_new_js}) {{
      dnameEl.addEventListener('input', function() {{
        if (!usernameTouched) {{
          unameEl.value = toSlug(dnameEl.value);
          if (unameHint) unameHint.style.display = 'none';
          syncSaveBtn();
        }}
      }});
    }}

    var checkComplete = function() {{
      var uname = unameEl ? unameEl.value.trim() : '';
      var dname = dnameEl ? dnameEl.value.trim() : '';
      var emailEl = document.getElementById('email');
      var email = emailEl ? emailEl.value.trim() : '';
      var pw    = pwInput ? pwInput.value : '';
      if (!uname || !dname || !email || !pw) return false;
      if (!slugPattern.test(uname)) return false;
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
</div>
{role_section}"#,
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
        role_section      = role_section,
    );

    crate::admin_page(title, "/admin/users", flash, &content, ctx)
}

// ── Site access management ──────────────────────────────────────────────────

pub struct SiteAssignmentRow {
    pub site_id: String,
    pub hostname: String,
    pub role: String,
    /// True when this row is the only 'admin'-role user on the site — removing
    /// or demoting them would leave the site with no site-scoped admin.
    pub is_last_admin: bool,
}

pub struct SiteAccessData {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    /// Current site assignments for this user.
    pub assignments: Vec<SiteAssignmentRow>,
    /// Sites the acting admin can assign this user to (their owned/managed sites).
    pub available_sites: Vec<SiteOption>,
    /// The user's global role at the time this page was created — used to
    /// pre-select a matching, non-privileged role in the assignment form.
    pub default_role: String,
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
            let confirm_msg = if a.is_last_admin {
                format!(
                    "{hostname} has no other Site Admin. Removing this access will leave the site \
                     with no one able to manage it (other than a super admin). Continue?",
                    hostname = a.hostname,
                )
            } else {
                format!("Remove {hostname} from site access?", hostname = a.hostname)
            };
            format!(
                r#"<tr>
                  <td>{hostname}</td>
                  <td><span class="badge">{role}</span></td>
                  <td class="actions">
                    <form method="post" action="/admin/users/{user_id}/site-access/remove" style="display:inline"
                          data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
                      <input type="hidden" name="site_id" value="{site_id}">
                      <button type="submit" class="icon-btn icon-danger" title="Remove from site">
                        <img src="/admin/static/icons/trash-2.svg" alt="Remove">
                      </button>
                    </form>
                  </td>
                </tr>"#,
                user_id     = crate::html_escape(&data.user_id),
                site_id     = crate::html_escape(&a.site_id),
                hostname    = crate::html_escape(&a.hostname),
                role        = crate::html_escape(role_display(&a.role)),
                confirm_msg = crate::html_escape(&confirm_msg),
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
        // Pre-select the role matching the user's existing global role when it maps
        // cleanly onto a site role. Never pre-select Site Admin — that must always
        // be a deliberate choice, since it grants ownership of the site.
        let sel = |role: &str| if data.default_role == role { " selected" } else { "" };
        let placeholder_selected = if matches!(data.default_role.as_str(), "editor" | "author" | "subscriber") {
            ""
        } else {
            " selected"
        };
        format!(
            r#"<form id="site-access-form" method="post" action="/admin/users/{user_id}/site-access/add">
  <input type="hidden" name="displaced_action" id="displaced-action-field" value="">
  <div class="form-group">
    <label for="site-select">Site</label>
    <select name="site_id" id="site-select" style="width:100%">
      <option value="" disabled selected>Select Site</option>
      {site_opts}
    </select>
  </div>
  <div class="form-group">
    <label for="role-select">Role</label>
    <select name="role" id="role-select" style="width:100%" disabled required>
      <option value="" disabled{placeholder_selected}>Select role&hellip;</option>
      {site_admin_opt}
      <option value="editor"{editor_selected}>Editor</option>
      <option value="author"{author_selected}>Author</option>
      <option value="subscriber"{subscriber_selected}>Subscriber</option>
    </select>
  </div>
  <button type="submit" class="btn btn-primary" id="assign-btn" disabled>Assign</button>
</form>

<!-- Existing Site Admin modal -->
<div id="displace-modal" style="display:none;position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,0.5);align-items:center;justify-content:center">
  <div style="background:#fff;border-radius:8px;padding:2rem;max-width:480px;width:90%;box-shadow:0 8px 32px rgba(0,0,0,0.18)">
    <h3 style="margin-top:0;color:var(--danger,#dc2626)">This site already has a Site Admin</h3>
    <p id="displace-msg" style="margin-bottom:1.5rem"></p>
    <p style="font-size:0.9rem;color:var(--muted)">A site can have more than one Site Admin. Choose what should happen:</p>
    <div style="display:flex;flex-direction:column;gap:0.75rem;margin:1.25rem 0">
      <label style="display:flex;align-items:flex-start;gap:0.6rem;cursor:pointer;padding:0.75rem;border:1.5px solid var(--border,#e5e7eb);border-radius:6px">
        <input type="radio" name="displace_choice" value="add_additional" style="margin-top:0.2rem;flex-shrink:0" checked>
        <span><strong>Add as an additional Site Admin</strong><br><span style="font-size:0.875rem;color:var(--muted)">The existing Site Admin keeps their access and ownership of the site unchanged.</span></span>
      </label>
      <label style="display:flex;align-items:flex-start;gap:0.6rem;cursor:pointer;padding:0.75rem;border:1.5px solid var(--border,#e5e7eb);border-radius:6px">
        <input type="radio" name="displace_choice" value="remove" style="margin-top:0.2rem;flex-shrink:0">
        <span><strong>Remove from site</strong><br><span style="font-size:0.875rem;color:var(--muted)">They lose all access immediately, and ownership transfers to the new assignee. Recommended if you no longer trust them.</span></span>
      </label>
      <label style="display:flex;align-items:flex-start;gap:0.6rem;cursor:pointer;padding:0.75rem;border:1.5px solid var(--border,#e5e7eb);border-radius:6px">
        <input type="radio" name="displace_choice" value="demote_author" style="margin-top:0.2rem;flex-shrink:0">
        <span><strong>Demote to Author, transfer ownership</strong><br><span style="font-size:0.875rem;color:var(--muted)">They keep read and write access to their own posts only, and ownership transfers to the new assignee.</span></span>
      </label>
    </div>
    <div style="display:flex;justify-content:flex-end;gap:0.75rem;margin-top:1.5rem">
      <button type="button" id="displace-cancel" class="btn btn-secondary">Cancel</button>
      <button type="button" id="displace-confirm" class="btn btn-primary">Confirm &amp; Assign</button>
    </div>
  </div>
</div>

<script>
(function() {{
  var form       = document.getElementById('site-access-form');
  var modal      = document.getElementById('displace-modal');
  var msgEl      = document.getElementById('displace-msg');
  var actionFld  = document.getElementById('displaced-action-field');
  var cancelBtn  = document.getElementById('displace-cancel');
  var confirmBtn = document.getElementById('displace-confirm');
  var roleSelect = document.getElementById('role-select');
  var siteSelect = document.getElementById('site-select');
  var assignBtn  = document.getElementById('assign-btn');

  function syncAssignBtn() {{
    assignBtn.disabled = !siteSelect.value || !roleSelect.value;
  }}

  // Enable role only once a real site is chosen.
  siteSelect.addEventListener('change', function() {{
    roleSelect.disabled = !siteSelect.value;
    syncAssignBtn();
  }});
  roleSelect.addEventListener('change', syncAssignBtn);

  var targetUserId = '{user_id}';

  form.addEventListener('submit', function(e) {{
    if (!siteSelect.value || !roleSelect.value) {{ e.preventDefault(); return; }}
    var opt = siteSelect.options[siteSelect.selectedIndex];
    var existingId   = opt.dataset.existingAdminId   || '';
    var existingName = opt.dataset.existingAdminName || '';

    if (roleSelect.value !== 'site_admin') {{
      // Demoting this same person away from Site Admin on a site they
      // currently own — warn, since it also clears their site ownership.
      if (existingId && existingId === targetUserId) {{
        var ok = confirm(escHtml(existingName) + ' is currently the Site Admin and owner of ' + opt.text +
          '. Changing their role will remove that access and site ownership. Continue?');
        if (!ok) {{ e.preventDefault(); }}
      }}
      return;
    }}

    if (!existingId) return; // no existing site admin — proceed normally
    e.preventDefault();
    var siteName = opt.text;
    msgEl.innerHTML = '<strong>' + escHtml(existingName) + '</strong> is currently the Site Admin for <strong>' + escHtml(siteName) + '</strong>.';
    modal.style.display = 'flex';
  }});

  cancelBtn.addEventListener('click', function() {{
    modal.style.display = 'none';
    actionFld.value = '';
  }});

  confirmBtn.addEventListener('click', function() {{
    var choice = document.querySelector('input[name="displace_choice"]:checked');
    actionFld.value = choice ? choice.value : 'add_additional';
    modal.style.display = 'none';
    form.submit();
  }});

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
            user_id              = crate::html_escape(&data.user_id),
            site_opts            = site_options,
            placeholder_selected = placeholder_selected,
            editor_selected      = sel("editor"),
            author_selected      = sel("author"),
            subscriber_selected  = sel("subscriber"),
            site_admin_opt = if ctx.is_global_admin {
                r#"<option value="site_admin">Site Admin</option>"#
            } else {
                ""
            },
        )
    };

    let content = format!(
        r#"<div class="two-col">
  <div>
    <h2>Current Site Access</h2>
    <table class="data-table">
      <thead><tr><th>Site</th><th>Role</th><th>Actions</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>
  </div>
  <div>
    <h2>Add to a Site</h2>
    {add_form}
  </div>
</div>"#,
        rows     = assignment_rows,
        add_form = add_form,
    );

    crate::admin_page(
        &format!("Site Access — {}", data.display_name),
        "/admin/users",
        flash,
        &content,
        ctx,
    )
}

