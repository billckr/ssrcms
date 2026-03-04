//! Public subscriber signup page (standalone, no admin sidebar).

/// Render the signup form. `error` may contain a validation or conflict message.
pub fn render(error: Option<&str>, site_name: &str) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let site_name = crate::html_escape(site_name);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Subscribe — {site_name}</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">{site_name}</h1>
    <h2>Create an account</h2>
    {error_html}
    <form method="POST" action="/subscribe">
      <label for="display_name">Name</label>
      <input type="text" id="display_name" name="display_name" required autofocus autocomplete="name">

      <label for="email">Email</label>
      <input type="email" id="email" name="email" required autocomplete="email">

      <label for="password">Password</label>
      <input type="password" id="password" name="password" required autocomplete="new-password">
      <small style="color:var(--muted);display:block;margin-top:.25rem">
        8–12 characters &middot; uppercase &middot; number &middot; symbol (! @ # $ % &amp;)
      </small>

      <label for="confirm_password" style="margin-top:.75rem">Confirm password</label>
      <input type="password" id="confirm_password" name="confirm_password" required autocomplete="new-password">

      <button type="submit" style="margin-top:1rem">Subscribe</button>
    </form>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        site_name = site_name,
        error_html = error_html,
    )
}

/// Render the post-signup success page.
pub fn render_success(site_name: &str) -> String {
    let site_name = crate::html_escape(site_name);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Subscribed — {site_name}</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">{site_name}</h1>
    <h2>You&rsquo;re subscribed!</h2>
    <p style="color:var(--muted);margin-top:.5rem">
      Your account has been created. You can now
      <a href="/admin/login">sign in</a>.
    </p>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        site_name = site_name,
    )
}
