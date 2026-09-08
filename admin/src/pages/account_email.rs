//! Public email-change confirmation page. Standalone (not wrapped in
//! `account_page`) because the visitor confirming a change may not be
//! logged into this browser at all — the link is often opened from a mail
//! client or a different device than the one that requested the change.

pub enum ConfirmState<'a> {
    Pending {
        old_email: &'a str,
        new_email: &'a str,
    },
    Invalid,
    Error(&'a str),
}

/// GET/POST /account/email/confirm/{token}.
pub fn render_confirm(token: &str, state: ConfirmState, default_theme: &str) -> String {
    let body = match state {
        ConfirmState::Pending {
            old_email,
            new_email,
        } => format!(
            r#"<p style="color:var(--muted);margin-top:.5rem">
      Change the sign-in email on your account from
      <strong>{old_email}</strong> to <strong>{new_email}</strong>?
    </p>
    <form method="POST" action="/account/email/confirm/{token}">
      <button type="submit" style="margin-top:1rem">Confirm Change</button>
    </form>
    <p style="color:var(--muted);margin-top:1rem;text-align:center">
      <a href="/account/profile">Cancel</a>
    </p>"#,
            old_email = crate::html_escape(old_email),
            new_email = crate::html_escape(new_email),
            token = crate::html_escape(token),
        ),
        ConfirmState::Invalid => r#"<p style="color:var(--muted);margin-top:.5rem">
      This confirmation link is invalid or has expired.
    </p>
    <p style="margin-top:1rem"><a href="/account/profile">Back to profile</a></p>"#
            .to_string(),
        ConfirmState::Error(msg) => format!(
            r#"<div class="error">{}</div>
    <p style="margin-top:1rem"><a href="/account/profile">Back to profile</a></p>"#,
            crate::html_escape(msg)
        ),
    };

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
  <title>Confirm Email Change</title>
  <script>
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
    <h2>Confirm Email Change</h2>
    {body}
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        body = body,
        default_theme = default_theme,
    )
}
