//! Login-time role picker — shown when a user holds more than one role on
//! the current site. Standalone page (no sidebar), same pattern as login.rs.

/// `roles` are the role strings the user holds on this site (e.g. "admin",
/// "editor"), already validated by the caller. `default_theme` is the
/// site-wide fallback appearance, same convention as login::render.
pub fn render(roles: &[&str], site_hostname: &str, default_theme: &str) -> String {
    fn label(r: &str) -> &str {
        match r {
            "admin" => "Admin",
            "editor" => "Editor",
            "author" => "Author",
            "subscriber" => "Subscriber",
            other => other,
        }
    }

    let options: String = roles
        .iter()
        .map(|r| {
            format!(
                r#"<option value="{value}">{label}</option>"#,
                value = crate::html_escape(r),
                label = crate::html_escape(label(r)),
            )
        })
        .collect::<Vec<_>>()
        .join("\n      ");

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
  <title>Select your role</title>
  <script>
    // Applies the saved/system theme before first paint (same key + logic as
    // the admin shell in lib.rs and the login page) so this standalone page
    // matches whatever theme the user last chose there.
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
  <style>{css}
    .role-pick-select {{ width: 100%; padding: .5rem .75rem; border: 1px solid var(--border); border-radius: var(--radius); font-size: 14px; margin-bottom: 1rem; background: var(--field-bg); color: var(--field-text); }}
    .role-pick-submit-row {{ display: flex; justify-content: center; }}
    /* .login-box button applies a blue gradient submit-button look to every
       button in the box — override it back to a plain blend-in icon button
       here so the hexagon just sits in its pill, not inside a blue chip. */
    .login-box .role-pick-submit-row button {{ width: 32px; height: 32px; padding: 0; background: none; background-image: none; border: 1px solid transparent; box-shadow: none; }}
    .login-box .role-pick-submit-row button:hover {{ box-shadow: none; transform: none; }}
    .login-box .role-pick-submit-row button:focus {{ box-shadow: none; }}
  </style>
</head>
<body class="login-body">
  <div class="login-box">
    <h1 class="login-brand">{site}</h1>
    <h2>Select your role</h2>
    <form method="post" action="/admin/pick-role">
      <select name="role" class="role-pick-select" required>
        {options}
      </select>
      <div class="role-pick-submit-row">
        <div class="icon-pill">
          <button type="submit" class="icon-btn" title="Continue" aria-label="Continue">
            <img src="/admin/static/icons/hexagon.svg" alt="">
          </button>
        </div>
      </div>
    </form>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        site = crate::html_escape(site_hostname),
        options = options,
        default_theme = default_theme,
    )
}
