//! Admin user management page.

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
}

pub fn render_list(users: &[UserRow], flash: Option<&str>, current_site: &str, current_user_id: &str, is_global_admin: bool) -> String {
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
            role = crate::html_escape(&u.role),
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

    crate::admin_page("Users", "/admin/users", flash, &content, current_site, is_global_admin)
}

pub fn render_editor(user: &UserEdit, flash: Option<&str>, current_site: &str, is_global_admin: bool) -> String {
    let title = if user.id.is_none() { "New User" } else { "Edit User" };
    let action = match &user.id {
        Some(id) => format!("/admin/users/{}/edit", id),
        None => "/admin/users/new".to_string(),
    };

    let roles: &[&str] = if is_global_admin {
        &["admin", "editor", "author", "subscriber"]
    } else {
        &["editor", "author"]
    };
    let role_options = roles.iter().map(|r| {
        let selected = if *r == user.role { " selected" } else { "" };
        format!(r#"<option value="{r}"{selected}>{r}</option>"#, r = r, selected = selected)
    }).collect::<Vec<_>>().join("");

    let password_hint = if user.id.is_some() {
        r#"<small>Leave blank to keep current password.</small>"#
    } else {
        ""
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
  <div class="form-group">
    <label for="role">Role</label>
    <select id="role" name="role">{role_options}</select>
  </div>
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
        role_options = role_options,
        bio = crate::html_escape(&user.bio),
        password_hint = password_hint,
    );

    crate::admin_page(title, "/admin/users", flash, &content, current_site, is_global_admin)
}
