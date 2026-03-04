//! App-controlled account area — rendered entirely in Rust, never in a theme.
//!
//! Because this area handles authenticated user data (profile, passwords), it
//! must NOT be theme-rendered. A site admin cannot modify these templates.

use serde::Deserialize;

/// Context passed to every account page shell.
pub struct AccountContext {
    pub user_email: String,
    pub user_role: String,
    pub user_display_name: String,
    pub site_name: String,
    pub site_base_url: String,
}

/// Wrap page content in the full account page shell (sidebar + nav + footer).
pub fn account_page(
    title: &str,
    current_path: &str,
    flash: Option<&str>,
    content: &str,
    ctx: &AccountContext,
) -> String {
    let flash_html = match flash {
        Some(msg) => {
            let is_error = msg.starts_with("Error")
                || msg.contains("error")
                || msg.contains("does not")
                || msg.contains("incorrect")
                || msg.contains("must")
                || msg.contains("cannot")
                || msg.contains("invalid")
                || msg.contains("failed")
                || msg.contains("do not match");
            let class = if is_error { "error" } else { "success" };
            format!(r#"<div class="flash {}">{}</div>"#, class, crate::html_escape(msg))
        }
        None => String::new(),
    };

    let nav_link = |href: &str, label: &str| -> String {
        let active = if current_path == href { " class=\"active\"" } else { "" };
        format!(r#"<li><a href="{}"{}>{}</a></li>"#, href, active, label)
    };

    let dashboard_link  = nav_link("/account",              "Dashboard");
    let saved_link      = nav_link("/account/saved-posts",  "Saved Posts");
    let comments_link   = nav_link("/account/my-comments",  "My Comments");
    let profile_link    = nav_link("/account/profile",      "Profile");

    let site_name   = crate::html_escape(&ctx.site_name);
    let user_email  = crate::html_escape(&ctx.user_email);
    let user_role   = crate::html_escape(&ctx.user_role);
    let back_url    = crate::html_escape(&ctx.site_base_url);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — {site_name}</title>
  <style>{css}</style>
</head>
<body>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <div class="brand">{site_name}</div>
      <ul>
        {dashboard_link}
        {saved_link}
        {comments_link}
        {profile_link}
      </ul>
      <div class="sidebar-footer">
        <span>{user_email}</span>
        <span class="sidebar-user-role">{user_role}</span>
        <a href="{back_url}">&larr; Back to site</a>
        <a href="/account/logout">Log out</a>
      </div>
    </nav>
    <main class="admin-main">
      <div class="admin-content">
        {flash_html}
        {content}
      </div>
    </main>
  </div>
</body>
</html>"#,
        title       = crate::html_escape(title),
        site_name   = site_name,
        css           = crate::ADMIN_CSS,
        dashboard_link = dashboard_link,
        saved_link    = saved_link,
        comments_link = comments_link,
        profile_link  = profile_link,
        user_email  = user_email,
        user_role   = user_role,
        back_url    = back_url,
        flash_html  = flash_html,
        content     = content,
    )
}

// ── Dashboard ──────────────────────────────────────────────────────────────

pub fn render_dashboard(ctx: &AccountContext) -> String {
    let display_name = crate::html_escape(&ctx.user_display_name);
    let content = format!(
        r#"<div class="profile-container">
  <h2>Dashboard</h2>
  <p>Welcome back, <strong>{display_name}</strong>!</p>
</div>"#,
        display_name = display_name,
    );
    account_page("Dashboard", "/account", None, &content, ctx)
}

// ── Profile ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AccountProfileForm {
    pub email: String,
    pub display_name: Option<String>,
}

pub struct ProfileData {
    pub username: String,
    pub email: String,
    pub display_name: String,
}

pub fn render_profile(data: &ProfileData, flash: Option<&str>, ctx: &AccountContext) -> String {
    let content = format!(
        r#"<div class="profile-container">
  <h2>Profile Management</h2>

  <form method="POST" action="/account/profile/update" class="profile-form">
    <fieldset>

      <div class="form-group">
        <label>Username</label>
        <p class="form-static-value">{username}</p>
        <small>Username cannot be changed.</small>
      </div>

      <div class="form-group">
        <label for="email">Email</label>
        <input type="email" id="email" name="email" value="{email}" required>
      </div>

      <div class="form-group">
        <label for="display_name">Display Name</label>
        <input type="text" id="display_name" name="display_name" value="{display_name}">
      </div>
    </fieldset>

    <button type="submit" class="btn btn-primary">Save Changes</button>
  </form>
</div>

<div class="profile-container">
  <h2>Password Management</h2>

  <form method="POST" action="/account/profile/change-password" class="password-form">
    <fieldset>

      <div class="form-group">
        <label for="current_password">Current Password</label>
        <input type="password" id="current_password" name="current_password" required>
      </div>

      <div class="form-group">
        <label for="new_password">New Password</label>
        <input type="password" id="new_password" name="new_password" required>
      </div>

      <div class="form-group">
        <label for="confirm_password">Confirm New Password</label>
        <input type="password" id="confirm_password" name="confirm_password" required>
      </div>

      <div class="form-note">
        <p><strong>Password requirements:</strong></p>
        <ul>
          <li>8&ndash;12 characters</li>
          <li>At least one uppercase letter</li>
          <li>At least one number</li>
          <li>At least one symbol: ! @ # $ % &amp;</li>
        </ul>
      </div>
    </fieldset>

    <button type="submit" class="btn btn-primary">Change Password</button>
  </form>
</div>"#,
        username     = crate::html_escape(&data.username),
        email        = crate::html_escape(&data.email),
        display_name = crate::html_escape(&data.display_name),
    );

    account_page("Profile", "/account/profile", flash, &content, ctx)
}

// ── Saved Posts (stub) ───────────────────────────────────────────────────────

pub fn render_saved_posts(ctx: &AccountContext) -> String {
    let content = r#"<h2>Saved Posts</h2>
<p class="muted">You haven&rsquo;t saved any posts yet.</p>"#;
    account_page("Saved Posts", "/account/saved-posts", None, content, ctx)
}

// ── My Comments (stub) ───────────────────────────────────────────────────────

pub fn render_my_comments(ctx: &AccountContext) -> String {
    let content = r#"<h2>My Comments</h2>
<p class="muted">You haven&rsquo;t made any comments yet.</p>"#;
    account_page("My Comments", "/account/my-comments", None, content, ctx)
}
