//! Public subscriber signup page (standalone, no admin sidebar).

/// Render the signup form. `error` may contain a validation or conflict message.
/// `default_theme` is the site-wide fallback appearance ("light"/"dark"/"system")
/// from Settings → General → Appearance.
pub fn render(error: Option<&str>, site_name: &str, default_theme: &str) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let site_name = crate::html_escape(site_name);
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
  <title>Subscribe — {site_name}</title>
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
  <div class="login-box login-box--wide">
    <h1 class="login-brand">{site_name}</h1>
    <h2>Create an account</h2>
    {error_html}
    <form method="POST" action="/subscribe">
      <div style="display:flex;gap:.75rem">
        <div style="flex:1;min-width:0">
          <label for="display_name">Display Name</label>
          <input type="text" id="display_name" name="display_name" required autofocus autocomplete="name" maxlength="60">
        </div>
        <div style="flex:1;min-width:0">
          <label for="email">Email</label>
          <input type="email" id="email" name="email" required autocomplete="email">
        </div>
      </div>

      <div style="display:flex;gap:.75rem">
        <div style="flex:1;min-width:0">
          <label for="password">Password</label>
          <input type="password" id="password" name="password" required autocomplete="new-password">
        </div>
        <div style="flex:1;min-width:0">
          <label for="confirm_password">Confirm password</label>
          <input type="password" id="confirm_password" name="confirm_password" required autocomplete="new-password">
        </div>
      </div>

      <!-- Honeypot: hidden from real users; bots fill it; handler rejects if non-empty -->
      <div style="display:none" aria-hidden="true">
        <label for="website">Website</label>
        <input type="text" id="website" name="website" tabindex="-1" autocomplete="off">
      </div>

      <div class="form-note" style="margin-top:1rem;margin-bottom:0;font-size:12px">
        <p style="margin-bottom:.35rem"><strong>Requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0;display:grid;gap:.15rem">
          <li id="dname-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Display name: 1–60 characters</li>
          <li id="email-req-valid"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Valid email address</li>
          <li id="pw-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Password: 8–12 characters</li>
          <li id="pw-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Password: at least one uppercase letter</li>
          <li id="pw-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Password: at least one number</li>
          <li id="pw-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Password: at least one symbol (! @ # $ % &amp;)</li>
          <li id="human-req"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>&#x201c;I&#x2019;m Human&#x201d; confirmed</li>
        </ul>
      </div>

      <label style="display:flex;align-items:center;gap:.5rem;font-weight:400;margin-top:1rem;cursor:pointer">
        <input type="checkbox" id="human_check" name="human_check" value="on" required style="width:auto;flex-shrink:0;margin:0">
        <span>I&#x2019;m Human</span>
      </label>

      <label style="display:flex;align-items:center;gap:.5rem;font-weight:400;margin-top:.6rem;cursor:pointer">
        <input type="checkbox" name="terms" value="on" required style="width:auto;flex-shrink:0;margin:0">
        <span>Agree to <a href="/terms" target="_blank" rel="noopener noreferrer">Terms of Service</a></span>
      </label>

      <button type="submit" style="margin-top:1rem">Subscribe</button>
    </form>
    <p style="color:var(--muted);margin-top:1rem;text-align:center">
      Already a member? <a href="/login">Sign in</a>
    </p>
  </div>
  <script>
  (function () {{
    var dnameEl = document.getElementById('display_name');
    var emailEl = document.getElementById('email');
    var pwEl    = document.getElementById('password');
    var humanEl = document.getElementById('human_check');

    function setDot(id, state) {{
      var li  = document.getElementById(id);
      if (!li) return;
      var dot = li.querySelector('.pw-dot');
      if (state === null) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (state) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }}

    function update() {{
      var dname = dnameEl ? dnameEl.value : '';
      setDot('dname-req-len', dname ? (dname.length >= 1 && dname.length <= 60) : null);

      var email = emailEl ? emailEl.value.trim() : '';
      setDot('email-req-valid', email ? /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) : null);

      var pw = pwEl ? pwEl.value : '';
      var pwReqs = [
        ['pw-req-len',   function(p) {{ return p.length >= 8 && p.length <= 12; }}],
        ['pw-req-upper', function(p) {{ return /[A-Z]/.test(p); }}],
        ['pw-req-num',   function(p) {{ return /[0-9]/.test(p); }}],
        ['pw-req-sym',   function(p) {{ return /[!@#$%&]/.test(p); }}],
      ];
      pwReqs.forEach(function(req) {{
        setDot(req[0], pw ? req[1](pw) : null);
      }});

      setDot('human-req', humanEl ? (humanEl.checked ? true : null) : null);
    }}

    [dnameEl, emailEl, pwEl].forEach(function(el) {{
      if (el) el.addEventListener('input', update);
    }});
    if (humanEl) humanEl.addEventListener('change', update);
    update();
  }})();
  </script>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        site_name = site_name,
        error_html = error_html,
    )
}

/// Render the post-signup success page.
pub fn render_success(site_name: &str, default_theme: &str) -> String {
    let site_name = crate::html_escape(site_name);
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
  <title>Subscribed — {site_name}</title>
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
    <h1 class="login-brand">{site_name}</h1>
    <h2>You&rsquo;re subscribed!</h2>
    <p style="color:var(--muted);margin-top:.5rem">
      Your account has been created. You can now
      <a href="/login">sign in</a>.
    </p>
  </div>
</body>
</html>"#,
        css = crate::ADMIN_CSS,
        site_name = site_name,
    )
}
