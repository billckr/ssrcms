//! Admin login page.

/// Render the standalone login page (no sidebar).
/// `action` is the form POST target — "/admin/login" for staff, "/login" for the public page.
/// `default_theme` is the site-wide fallback appearance ("light"/"dark"/"system")
/// from Settings → General → Appearance.
pub fn render(error: Option<&str>, default_theme: &str) -> String {
    render_with_action(error, None, "/admin/login", None, default_theme)
}

/// Same form rendered for the public-facing /login page.
/// `redirect` is an optional path to send the user to after a successful login.
/// `flash` is an optional one-shot success message (e.g. after a password reset).
pub fn render_public(error: Option<&str>, flash: Option<&str>, redirect: Option<&str>, default_theme: &str) -> String {
    render_with_action(error, flash, "/login", redirect, default_theme)
}

fn render_with_action(error: Option<&str>, flash: Option<&str>, action: &str, redirect: Option<&str>, default_theme: &str) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let flash_html = match flash {
        Some(msg) => format!(r#"<div class="flash success">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let redirect_input = match redirect {
        Some(r) if !r.is_empty() => format!(
            r#"<input type="hidden" name="redirect" value="{}">"#,
            crate::html_escape(r)
        ),
        _ => String::new(),
    };
    // Only the public-facing /login page offers self-service signup/recovery —
    // staff logging in at /admin/login already have accounts created for them.
    let below_form_links = if action == "/login" {
        r#"<p style="color:var(--muted);margin-top:1rem;text-align:center">
      Not a member? <a href="/subscribe">Join today!</a><br>
      <a href="/recover">Recover Login</a>
    </p>"#
    } else {
        ""
    };

    // Already validated to one of these three literals when saved — safe to
    // splice into JS, but re-checked here since callers pass it through untrusted.
    let default_theme = match default_theme {
        "light" | "dark" => default_theme,
        _ => "system",
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Sign in</title>
  <script>
    // Applies the saved/system theme before first paint (same key + logic as
    // the admin shell in lib.rs) so this standalone page matches whatever
    // theme the user last chose there instead of always rendering light.
    (function() {{
      try {{
        var pref = localStorage.getItem('admin-theme') || '{default_theme}';
        var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        if (dark) {{
          document.documentElement.setAttribute('data-theme', 'dark');
        }}
      }} catch (e) {{}}
    }})();
  </script>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">Synaptic</h1>
    <h2>Sign in</h2>
    {error_html}
    {flash_html}
    <form method="POST" action="{action}">
      {redirect_input}
      <label for="email">Email</label>
      <input type="email" id="email" name="email" required autofocus>
      <label for="password">Password</label>
      <input type="password" id="password" name="password" required>
      <button type="submit">Sign in</button>
    </form>
    {below_form_links}
  </div>
</body>
</html>"#,
        css              = crate::ADMIN_CSS,
        error_html       = error_html,
        flash_html       = flash_html,
        redirect_input   = redirect_input,
        action           = action,
        below_form_links = below_form_links,
        default_theme    = default_theme,
    )
}
