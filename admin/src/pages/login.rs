//! Admin login page.

/// Render the standalone login page (no sidebar).
pub fn render(error: Option<&str>) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Admin Login — Synaptic</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">Synaptic</h1>
    <h2>Sign in</h2>
    {error_html}
    <form method="POST" action="/admin/login">
      <label for="email">Email</label>
      <input type="email" id="email" name="email" required autofocus>
      <label for="password">Password</label>
      <input type="password" id="password" name="password" required>
      <button type="submit">Sign in</button>
    </form>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        error_html = error_html,
    )
}
