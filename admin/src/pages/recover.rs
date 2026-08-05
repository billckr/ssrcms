//! Public password-recovery pages: request form, "check your email"
//! confirmation, and the token-gated set-new-password form.

/// GET /recover — request form (type in your email).
pub fn render_request(error: Option<&str>, sent: bool) -> String {
    let body = if sent {
        r#"<p style="color:var(--muted);margin-top:.5rem">
      If that email address has an account, we&rsquo;ve sent a link to reset
      the password. The link expires in 1 hour.
    </p>"#.to_string()
    } else {
        let error_html = match error {
            Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
            None => String::new(),
        };
        format!(
            r#"{error_html}
    <form method="POST" action="/recover">
      <label for="email">Email</label>
      <input type="email" id="email" name="email" required autofocus autocomplete="email">

      <!-- Honeypot: hidden from real users; bots fill it; handler ignores if non-empty -->
      <div style="display:none" aria-hidden="true">
        <label for="website">Website</label>
        <input type="text" id="website" name="website" tabindex="-1" autocomplete="off">
      </div>

      <button type="submit" style="margin-top:1rem">Send Recovery Link</button>
    </form>"#,
            error_html = error_html,
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Recover Login</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">Synaptic</h1>
    <h2>Recover Login</h2>
    {body}
    <p style="color:var(--muted);margin-top:1rem;text-align:center">
      <a href="/login">Back to sign in</a>
    </p>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        body = body,
    )
}

/// GET /recover/{token} — set a new password. `valid` is false when the
/// token is missing, expired, or already used.
pub fn render_reset(token: &str, valid: bool, error: Option<&str>) -> String {
    let body = if !valid {
        r#"<p style="color:var(--muted);margin-top:.5rem">
      This recovery link is invalid or has expired.
    </p>
    <p style="margin-top:1rem"><a href="/recover">Request a new one</a></p>"#.to_string()
    } else {
        let error_html = match error {
            Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
            None => String::new(),
        };
        format!(
            r#"{error_html}
    <form method="POST" action="/recover/{token}">
      <label for="password">New password</label>
      <input type="password" id="password" name="password" required autofocus autocomplete="new-password">
      <small style="color:var(--muted);display:block;margin-top:.25rem">
        8&ndash;12 characters &middot; uppercase &middot; number &middot; symbol (! @ # $ % &amp;)
      </small>

      <label for="confirm_password" style="margin-top:.75rem">Confirm new password</label>
      <input type="password" id="confirm_password" name="confirm_password" required autocomplete="new-password">

      <button type="submit" style="margin-top:1rem">Reset Password</button>
    </form>"#,
            error_html = error_html,
            token = crate::html_escape(token),
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Reset Password</title>
  <style>{css}</style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">Synaptic</h1>
    <h2>Set a new password</h2>
    {body}
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        body = body,
    )
}
