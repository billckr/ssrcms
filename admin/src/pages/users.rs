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

pub fn render_list(users: &[UserRow], flash: Option<&str>, current_user_id: &str, can_manage_access: bool, ctx: &crate::PageContext) -> String {
    let rows = users.iter().map(|u| {
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
        format!(
            r#"<tr>
              <td><a href="/admin/users/{id}/edit">{display_name}</a></td>
              <td>{username}</td>
              <td>{email}</td>
              <td><span class="badge {badge_class}">{role}</span></td>
              <td class="actions">
                <a href="/admin/users/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
                {site_access_btn}
                {delete_btn}
              </td>
            </tr>"#,
            id = crate::html_escape(&u.id),
            display_name = crate::html_escape(&u.display_name),
            username = crate::html_escape(&u.username),
            email = crate::html_escape(&u.email),
            role = crate::html_escape(role_display(&u.role)),
            badge_class = role_badge_class(&u.role),
            site_access_btn = site_access_btn,
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<p style="margin-bottom:1rem"><a href="/admin/users/new" class="btn btn-primary">New User</a></p>
<table class="data-table">
  <thead><tr><th>Name</th><th>Username</th><th>Email</th><th>Role</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows,
    );

    crate::admin_page("Users", "/admin/users", flash, &content, ctx)
}

pub fn render_editor(user: &UserEdit, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let title = if user.id.is_none() { "New User" } else { "Edit User" };
    let action = match &user.id {
        Some(id) => format!("/admin/users/{}/edit", id),
        None => "/admin/users/new".to_string(),
    };

    // Role field: read-only display for super_admin targets; dropdown for everyone else.
    // Global admin creates/edits site-scoped users using site role values (admin/editor/author/subscriber).
    // "admin" here means site_users.role = 'admin' (site admin), NOT users.role = 'super_admin'.
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
                ("editor", "Editor"),
                ("author", "Author"),
            ]
        };
        let role_options = roles.iter().map(|(value, label)| {
            let selected = if *value == user.role { " selected" } else { "" };
            format!(r#"<option value="{value}"{selected}>{label}</option>"#)
        }).collect::<Vec<_>>().join("");
        format!(r#"<div class="form-group">
  <label for="role">Role</label>
  <select id="role" name="role">{role_options}</select>
</div>"#)
    };

    let password_hint = if user.id.is_some() {
        r#"<small>Leave blank to keep current password.</small>"#
    } else {
        ""
    };

    // Site-assignment section — only for global admins creating a new user.
    let site_section = if user.id.is_none() && ctx.is_global_admin {
        let site_opts = user.sites.iter().map(|s| {
            format!(
                "<option value=\"{}\">{}</option>",
                crate::html_escape(&s.id),
                crate::html_escape(&s.hostname),
            )
        }).collect::<Vec<_>>().join("\n");
        format!(
            "<div class=\"form-group\">\
\n  <label>Site Assignment</label>\
\n  <div style=\"margin-bottom:0.5rem\">\
\n    <label style=\"margin-right:1.5rem\">\
\n      <input type=\"radio\" name=\"site_assignment\" value=\"existing\" checked onchange=\"toggleSiteFields()\"> Use existing site\
\n    </label>\
\n    <label>\
\n      <input type=\"radio\" name=\"site_assignment\" value=\"new\" onchange=\"toggleSiteFields()\"> Create new site\
\n    </label>\
\n  </div>\
\n  <div id=\"site-existing\">\
\n    <select name=\"existing_site_id\" style=\"width:100%\">{site_opts}</select>\
\n  </div>\
\n  <div id=\"site-new\" style=\"display:none\">\
\n    <input type=\"text\" name=\"new_hostname\" placeholder=\"example.com\" style=\"width:100%\">\
\n    <small>The domain this site will respond to (e.g. client.example.com)</small>\
\n  </div>\
\n</div>\
\n<script>\
\nfunction toggleSiteFields() {{\
\n  var val = document.querySelector('input[name=\"site_assignment\"]:checked').value;\
\n  document.getElementById('site-existing').style.display = val === 'existing' ? '' : 'none';\
\n  document.getElementById('site-new').style.display = val === 'new' ? '' : 'none';\
\n}}\
\n</script>",
            site_opts = site_opts,
        )
    } else {
        String::new()
    };
    let content = format!(
        r#"<form method="POST" action="{action}">
  <div class="form-group">
    <label for="username">Username</label>
    <input type="text" id="username" name="username" value="{username}" required>
  </div>
  <div class="form-group">
    <label for="display_name">Display Name</label>
    <input type="text" id="display_name" name="display_name" value="{display_name}">
  </div>
  <div class="form-group">
    <label for="email">Email</label>
    <input type="email" id="email" name="email" value="{email}" required>
  </div>
  <div class="form-group">
    <label for="password">Password</label>
    <input type="password" id="password" name="password">
    {password_hint}
  </div>
  {role_field}
  {site_section}
  <div class="form-group">
    <label for="bio">Bio</label>
    <textarea id="bio" name="bio" rows="3">{bio}</textarea>
  </div>
  <button type="submit" class="btn btn-primary">Save</button>
  <a href="/admin/users" class="btn">Cancel</a>
</form>"#,
        action = action,
        username = crate::html_escape(&user.username),
        display_name = crate::html_escape(&user.display_name),
        email = crate::html_escape(&user.email),
        role_field = role_field,
        site_section = site_section,
        bio = crate::html_escape(&user.bio),
        password_hint = password_hint,
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
        format!(
            r#"<option value="{}">{}</option>"#,
            crate::html_escape(&s.id),
            crate::html_escape(&s.hostname),
        )
    }).collect::<Vec<_>>().join("\n");

    let add_form = if data.available_sites.is_empty() {
        "<p><em>No sites available to assign.</em></p>".to_string()
    } else {
        format!(
            r#"<form method="post" action="/admin/users/{user_id}/site-access/add"
  data-username="{display_name}"
  onsubmit="if(this.querySelector('[name=role]').value==='site_admin'){{var s=this.querySelector('[name=site_id] option:checked').text;return confirm('WARNING: You are about to assign \'' + this.dataset.username + '\' as Site Admin (owner) of \'' + s + '\'.\n\nThis grants full ownership of that site and changes their global role to site_admin.\n\nAre you sure?');}}"
  style="display:flex;gap:0.75rem;align-items:flex-end;flex-wrap:wrap;margin-top:1.5rem">
  <div class="form-group" style="margin:0;flex:1;min-width:160px">
    <label>Site</label>
    <select name="site_id" style="width:100%">{site_opts}</select>
  </div>
  <div class="form-group" style="margin:0;flex:1;min-width:140px">
    <label>Role</label>
    <select name="role" style="width:100%">
      {site_admin_opt}
      <option value="editor">Editor</option>
      <option value="author">Author</option>
      <option value="subscriber">Subscriber</option>
    </select>
  </div>
  <div class="form-group" style="margin:0">
    <button type="submit" class="btn btn-primary">Assign</button>
  </div>
</form>"#,
            user_id        = crate::html_escape(&data.user_id),
            display_name   = crate::html_escape(&data.display_name),
            site_opts      = site_options,
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

