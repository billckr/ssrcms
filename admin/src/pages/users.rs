//! Admin user management page.

/// Map a stored role value to a human-readable display label.
fn role_display(role: &str) -> &str {
    match role {
        "super_admin" => "Super Admin",
        "admin"       => "Admin",
        "editor"      => "Editor",
        "author"      => "Author",
        "subscriber"  => "Subscriber",
        other         => other,
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

pub fn render_list(users: &[UserRow], flash: Option<&str>, current_site: &str, current_user_id: &str, is_global_admin: bool, user_email: &str) -> String {
    let rows = users.iter().map(|u| {
        let delete_btn = if u.id != current_user_id && !u.is_protected {
            let warn_msg = format!(
                "Delete user \\u2018{}\\u2019? This will permanently delete all their posts and pages. This cannot be undone.",
                u.display_name.replace('\'', "\\'"),
            );
            format!(
                r#"<form method="POST" action="/admin/users/{id}/delete" style="display:inline" onsubmit="return confirm('{warn_msg}')">
                  <button class="icon-btn icon-danger" title="Delete user" type="submit">
                    <img src="/admin/static/icons/delete.svg" alt="Delete">
                  </button>
                </form>"#,
                id = crate::html_escape(&u.id),
                warn_msg = warn_msg,
            )
        } else {
            String::new()
        };
        format!(
            r#"<tr>
              <td><a href="/admin/users/{id}/edit">{display_name}</a></td>
              <td>{username}</td>
              <td>{email}</td>
              <td><span class="badge">{role}</span></td>
              <td class="actions">
                <a href="/admin/users/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
                {delete_btn}
              </td>
            </tr>"#,
            id = crate::html_escape(&u.id),
            display_name = crate::html_escape(&u.display_name),
            username = crate::html_escape(&u.username),
            email = crate::html_escape(&u.email),
            role = crate::html_escape(role_display(&u.role)),
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

    crate::admin_page("Users", "/admin/users", flash, &content, current_site, is_global_admin, user_email)
}

pub fn render_editor(user: &UserEdit, flash: Option<&str>, current_site: &str, is_global_admin: bool, user_email: &str) -> String {
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
        let roles: &[(&str, &str)] = if is_global_admin {
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
    let site_section = if user.id.is_none() && is_global_admin {
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

    crate::admin_page(title, "/admin/users", flash, &content, current_site, is_global_admin, user_email)
}
