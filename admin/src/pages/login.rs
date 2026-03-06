//! Admin login page.

/// Render the standalone login page (no sidebar).
/// `action` is the form POST target — "/admin/login" for staff, "/login" for the public page.
pub fn render(error: Option<&str>) -> String {
    render_with_action(error, "/admin/login", None)
}

/// Same form rendered for the public-facing /login page.
/// `redirect` is an optional path to send the user to after a successful login.
pub fn render_public(error: Option<&str>, redirect: Option<&str>) -> String {
    render_with_action(error, "/login", redirect)
}

fn render_with_action(error: Option<&str>, action: &str, redirect: Option<&str>) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let redirect_input = match redirect {
        Some(r) if !r.is_empty() => format!(
            r#"<input type="hidden" name="redirect" value="{}">"#,
            crate::html_escape(r)
        ),
        _ => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Sign in</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">Synaptic</h1>
    <h2>Sign in</h2>
    {error_html}
    <form method="POST" action="{action}">
      {redirect_input}
      <label for="email">Email</label>
      <input type="email" id="email" name="email" required autofocus>
      <label for="password">Password</label>
      <input type="password" id="password" name="password" required>
      <button type="submit">Sign in</button>
    </form>
  </div>
</body>
</html>"#,
        css            = crate::ADMIN_CSS,
        error_html     = error_html,
        redirect_input = redirect_input,
        action         = action,
    )
}
