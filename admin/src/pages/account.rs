//! App-controlled account area — rendered entirely in Rust, never in a theme.
//!
//! Because this area handles authenticated user data (profile, passwords), it
//! must NOT be theme-rendered. A site admin cannot modify these templates.

use serde::Deserialize;

/// Context passed to every account page shell.
pub struct AccountContext {
    pub user_email: String,
    pub user_display_name: String,
    pub site_name: String,
    /// Site-wide fallback appearance ("light" | "dark" | "system") from
    /// Settings → General → Appearance — same convention as admin_page.
    pub default_theme: String,
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
                || msg.contains("do not match")
                || msg.contains("already exists");
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

    let site_name         = crate::html_escape(&ctx.site_name);
    let user_display_name = crate::html_escape(&ctx.user_display_name);

    // Account doesn't support a per-user theme setting yet, but it shares the
    // same 'admin-theme' localStorage key, site-wide default, and
    // system/dark-only switch as the admin area (light mode is disabled
    // there for now too — see admin_page).
    let default_theme = match ctx.default_theme.as_str() {
        "light" | "dark" => ctx.default_theme.as_str(),
        _ => "system",
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} — {site_name}</title>
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
<body>
  <div class="sidebar-overlay" onclick="closeSidebar()"></div>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <a class="brand" href="/account">{site_name}</a>
      <ul>
        {dashboard_link}
        {saved_link}
        {comments_link}
      </ul>
    </nav>
    <main class="admin-main">
      <header class="admin-header">
        <button class="hamburger" onclick="toggleSidebar()" aria-label="Open navigation">
          <span></span><span></span><span></span>
        </button>
        <h1>{title}</h1>
        <span class="admin-header-user">{user_display_name}</span>
        <div class="header-menu">
          <button type="button" class="icon-btn" onclick="toggleHeaderMenu()" title="Menu" aria-label="Menu" aria-haspopup="true" aria-expanded="false" id="header-menu-btn">
            <img src="/admin/static/icons/list.svg" alt="">
          </button>
          <div class="header-menu-dropdown" id="header-menu-dropdown">
            <div class="theme-switch" role="group" aria-label="Theme" id="theme-switch">
              <!-- Light mode disabled for now, same as admin — setTheme('light') logic kept intact for re-enabling later. -->
              <button type="button" class="theme-switch-btn" data-theme-choice="system" onclick="setTheme('system')" title="Match system" aria-label="Match system">
                <img src="/admin/static/icons/monitor.svg" alt="">
              </button>
              <button type="button" class="theme-switch-btn" data-theme-choice="dark" onclick="setTheme('dark')" title="Dark mode" aria-label="Dark mode">
                <img src="/admin/static/icons/moon.svg" alt="">
              </button>
            </div>
            <a href="/account/profile" class="header-menu-item">
              <img src="/admin/static/icons/fingerprint-light.svg" alt="">
              <span>Profile</span>
            </a>
            <a href="/account/logout" class="header-menu-item">
              <img src="/admin/static/icons/log-out.svg" alt="">
              <span>Log out</span>
            </a>
          </div>
        </div>
      </header>
      <div class="admin-content">
        {flash_html}
        {content}
      </div>
    </main>
  </div>
  <script>
    function toggleSidebar() {{
      document.body.classList.toggle('sidebar-open');
    }}
    function closeSidebar() {{
      document.body.classList.remove('sidebar-open');
    }}
    function applyTheme(pref) {{
      var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
      if (dark) {{
        document.documentElement.setAttribute('data-theme', 'dark');
      }} else {{
        document.documentElement.removeAttribute('data-theme');
      }}
      var switchEl = document.getElementById('theme-switch');
      if (switchEl) {{
        var btns = switchEl.querySelectorAll('.theme-switch-btn');
        for (var i = 0; i < btns.length; i++) {{
          btns[i].classList.toggle('active', btns[i].getAttribute('data-theme-choice') === pref);
        }}
      }}
    }}
    function setTheme(pref) {{
      try {{ localStorage.setItem('admin-theme', pref); }} catch (e) {{}}
      applyTheme(pref);
      document.getElementById('header-menu-dropdown').classList.remove('open');
      document.getElementById('header-menu-btn').setAttribute('aria-expanded', 'false');
    }}
    applyTheme((function() {{
      try {{ return localStorage.getItem('admin-theme') || '{default_theme}'; }} catch (e) {{ return '{default_theme}'; }}
    }})());
    function toggleHeaderMenu() {{
      var dropdown = document.getElementById('header-menu-dropdown');
      var btn = document.getElementById('header-menu-btn');
      var open = dropdown.classList.toggle('open');
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    }}
    document.addEventListener('click', function(e) {{
      var menu = document.querySelector('.header-menu');
      if (menu && !menu.contains(e.target)) {{
        document.getElementById('header-menu-dropdown').classList.remove('open');
        document.getElementById('header-menu-btn').setAttribute('aria-expanded', 'false');
      }}
    }});
    document.addEventListener('keydown', function(e) {{
      if (e.key === 'Escape') {{
        document.getElementById('header-menu-dropdown').classList.remove('open');
        document.getElementById('header-menu-btn').setAttribute('aria-expanded', 'false');
      }}
    }});
    document.querySelectorAll('.admin-sidebar a').forEach(function(a) {{
      a.addEventListener('click', function(e) {{
        if (a.getAttribute('href') !== '#') closeSidebar();
      }});
    }});
    (function() {{
      var flash = document.querySelector('.flash');
      if (flash) {{
        setTimeout(function() {{
          flash.style.transition = 'opacity .4s ease';
          flash.style.opacity = '0';
          setTimeout(function() {{ flash.remove(); }}, 400);
        }}, 5000);
      }}
      if (window.location.search.indexOf('flash=') !== -1) {{
        var url = new URL(window.location.href);
        url.searchParams.delete('flash');
        window.history.replaceState({{}}, '', url.pathname + url.search + url.hash);
      }}
    }})();
  </script>
</body>
</html>"#,
        title       = crate::html_escape(title),
        site_name   = site_name,
        css           = crate::ADMIN_CSS,
        dashboard_link = dashboard_link,
        saved_link    = saved_link,
        comments_link = comments_link,
        user_display_name = user_display_name,
        flash_html  = flash_html,
        content     = content,
        default_theme = default_theme,
    )
}

// ── Dashboard ──────────────────────────────────────────────────────────────

pub fn render_dashboard(ctx: &AccountContext) -> String {
    account_page("Dashboard", "/account", None, "", ctx)
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
    pub bio: String,
}

/// Up to two uppercase initials, preferring the display name over the username.
fn initials(display_name: &str, username: &str) -> String {
    let source = if display_name.trim().is_empty() { username } else { display_name };
    let letters: String = source
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect();
    if letters.is_empty() { "?".to_string() } else { letters }
}

/// Escaped bio, or a muted placeholder line when the user hasn't written one.
fn display_or_placeholder(value: &str) -> String {
    if value.trim().is_empty() {
        r#"<span class="profile-summary-empty">&quot;The future has yet to be written...&quot;</span>"#.to_string()
    } else {
        format!("&quot;{}&quot;", crate::html_escape(value.trim()))
    }
}

/// Same avatar-card / bio-card / modal layout as /admin/profile
/// (admin/src/pages/profile.rs) — kept as a separate copy rather than a
/// shared function since this one posts to /account/* routes and wraps in
/// account_page instead of admin_page.
pub fn render_profile(data: &ProfileData, flash: Option<&str>, ctx: &AccountContext) -> String {
    let content = format!(
        r#"<div class="profile-layout">
  <div class="profile-main">
  </div>

  <div class="profile-side">
    <div class="profile-avatar-card">
      <div class="profile-avatar" aria-hidden="true">{initials}</div>
      <div class="profile-avatar-name">{display_name_or_username}</div>
      <div class="profile-avatar-email">{email}</div>
      <div class="icon-pill profile-avatar-btn">
        <button type="button" class="icon-btn" disabled title="Change photo (coming soon)" aria-label="Change photo">
          <img src="/admin/static/icons/camera.svg" alt="">
        </button>
        <button type="button" class="icon-btn" title="Edit Profile" aria-label="Edit Profile"
                onclick="document.getElementById('edit-profile-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
          <img src="/admin/static/icons/fingerprint-light.svg" alt="">
        </button>
        <button type="button" class="icon-btn" title="Change password" aria-label="Change password"
                onclick="document.getElementById('change-password-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
          <img src="/admin/static/icons/key.svg" alt="">
        </button>
      </div>
      <p class="profile-avatar-hint">Custom avatars aren't supported yet — this is a placeholder.</p>
    </div>

    <div class="profile-bio-card">
      <p class="profile-bio">{bio_shown}</p>
    </div>
  </div>
</div>

<dialog id="edit-profile-dialog" class="modal-card">
  <form method="POST" action="/account/profile/update">
    <h3 class="modal-card-header">Edit Profile</h3>
    <div class="modal-card-body">
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

      <div class="form-group">
        <label for="bio">Bio</label>
        <textarea id="bio" name="bio" rows="4">{bio}</textarea>
      </div>

      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('edit-profile-dialog').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn" title="Update Profile" aria-label="Update Profile" id="edit-profile-save-btn" disabled>
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>

<dialog id="change-password-dialog" class="modal-card">
  <form method="POST" action="/account/profile/change-password" id="change-password-form" novalidate>
    <h3 class="modal-card-header">Change Password</h3>
    <div class="modal-card-body">
      <div class="form-group">
        <label for="current_password">Current Password</label>
        <input type="password" id="current_password" name="current_password" required>
      </div>

      <div class="form-group">
        <label for="new_password">New Password</label>
        <input type="password" id="new_password" name="new_password" required minlength="8" maxlength="12">
      </div>

      <div class="form-group">
        <label for="confirm_password">Confirm New Password</label>
        <input type="password" id="confirm_password" name="confirm_password" required minlength="8" maxlength="12">
      </div>

      <div class="form-note">
        <p><strong>Password requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="np-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>8–12 characters</li>
          <li id="np-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one uppercase letter</li>
          <li id="np-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one number</li>
          <li id="np-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one symbol: ! @ # $ % &amp;</li>
          <li id="np-req-match"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>Passwords match</li>
        </ul>
      </div>

      <p id="change-password-error" class="profile-form-error" hidden></p>

      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('change-password-dialog').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn" title="Change Password" aria-label="Change Password" id="change-password-save-btn" disabled>
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>

<script>
document.getElementById('edit-profile-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
document.getElementById('change-password-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});

(function() {{
  var emailInput = document.getElementById('email');
  var displayNameInput = document.getElementById('display_name');
  var bioInput = document.getElementById('bio');
  var saveBtn = document.getElementById('edit-profile-save-btn');

  var original = {{
    email: emailInput.value,
    display_name: displayNameInput.value,
    bio: bioInput.value,
  }};

  var syncSaveBtn = function() {{
    var changed = emailInput.value !== original.email
      || displayNameInput.value !== original.display_name
      || bioInput.value !== original.bio;
    var active = changed && emailInput.checkValidity();
    saveBtn.disabled = !active;
    saveBtn.classList.toggle('icon-btn-active-blue', active);
  }};

  [emailInput, displayNameInput, bioInput].forEach(function(el) {{
    el.addEventListener('input', syncSaveBtn);
  }});
}})();

(function() {{
  var currentPwInput = document.getElementById('current_password');
  var newPwInput = document.getElementById('new_password');
  var confirmPwInput = document.getElementById('confirm_password');
  var saveBtn = document.getElementById('change-password-save-btn');

  var npReqs = [
    {{ id: 'np-req-len',   test: function(p) {{ return p.length >= 8 && p.length <= 12; }} }},
    {{ id: 'np-req-upper', test: function(p) {{ return /[A-Z]/.test(p); }} }},
    {{ id: 'np-req-num',   test: function(p) {{ return /[0-9]/.test(p); }} }},
    {{ id: 'np-req-sym',   test: function(p) {{ return /[!@#$%&]/.test(p); }} }},
  ];

  var updateFeedback = function() {{
    var errorEl = document.getElementById('change-password-error');
    if (errorEl) errorEl.hidden = true;

    var pw = newPwInput ? newPwInput.value : '';
    npReqs.forEach(function(req) {{
      var li = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!pw) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (req.test(pw)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }});

    var matchLi = document.getElementById('np-req-match');
    var matchDot = matchLi ? matchLi.querySelector('.pw-dot') : null;
    var confirmPw = confirmPwInput ? confirmPwInput.value : '';
    var matches = !!pw && pw === confirmPw;
    if (matchLi) {{
      if (!pw && !confirmPw) {{
        matchLi.style.color = ''; if (matchDot) matchDot.textContent = '·';
      }} else if (matches) {{
        matchLi.style.color = '#16a34a'; if (matchDot) matchDot.textContent = '✓';
      }} else {{
        matchLi.style.color = '#dc2626'; if (matchDot) matchDot.textContent = '✗';
      }}
    }}

    var meetsAllReqs = npReqs.every(function(req) {{ return req.test(pw); }});
    var currentPw = currentPwInput ? currentPwInput.value : '';
    var active = !!(currentPw && meetsAllReqs && matches);
    if (saveBtn) {{
      saveBtn.disabled = !active;
      saveBtn.classList.toggle('icon-btn-active-blue', active);
    }}
  }};

  if (currentPwInput) currentPwInput.addEventListener('input', updateFeedback);
  if (newPwInput) newPwInput.addEventListener('input', updateFeedback);
  if (confirmPwInput) confirmPwInput.addEventListener('input', updateFeedback);

  document.getElementById('change-password-form').addEventListener('submit', function(e) {{
    var newPw = newPwInput.value;
    var confirmPw = confirmPwInput.value;
    var errorEl = document.getElementById('change-password-error');
    var errors = [];

    if (newPw.length < 8 || newPw.length > 12) {{
      errors.push('Password must be 8-12 characters.');
    }}
    if (!/[A-Z]/.test(newPw)) {{
      errors.push('Password must contain at least one uppercase letter.');
    }}
    if (!/[0-9]/.test(newPw)) {{
      errors.push('Password must contain at least one number.');
    }}
    if (!/[!@#$%&]/.test(newPw)) {{
      errors.push('Password must contain at least one symbol: ! @ # $ % &');
    }}
    if (newPw !== confirmPw) {{
      errors.push('New passwords do not match.');
    }}

    if (errors.length > 0) {{
      e.preventDefault();
      errorEl.textContent = errors[0];
      errorEl.hidden = false;
    }} else {{
      errorEl.hidden = true;
    }}
  }});
}})();
</script>"#,
        username = crate::html_escape(&data.username),
        email = crate::html_escape(&data.email),
        display_name = crate::html_escape(&data.display_name),
        bio = crate::html_escape(&data.bio),
        bio_shown = display_or_placeholder(&data.bio),
        initials = crate::html_escape(&initials(&data.display_name, &data.username)),
        display_name_or_username = crate::html_escape(
            if data.display_name.trim().is_empty() { &data.username } else { &data.display_name }
        ),
    );

    account_page("Profile", "/account/profile", flash, &content, ctx)
}

// ── Saved Posts ───────────────────────────────────────────────────────────────

pub struct SavedPostRow {
    pub title:    String,
    pub slug:     String,
    pub post_url: String,
    pub saved_at: String,
}

fn saved_posts_pagination(page: i64, total_pages: i64, search: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };
    let prev = if page > 1 {
        format!(r#"<a href="/account/saved-posts?page={}{}" class="page-btn">&laquo; Prev</a>"#, page - 1, search_qs)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="/account/saved-posts?page={}{}" class="page-btn">Next &raquo;</a>"#, page + 1, search_qs)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end   = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(
                r#"<a href="/account/saved-posts?page={p}{search_qs}" class="page-btn">{p}</a>"#,
                search_qs = search_qs
            ));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Returns just the inner list HTML (pagination + table).
/// Used by `render_saved_posts` and the live-search fetch (`?partial=1`).
pub fn saved_posts_list_fragment(rows: &[SavedPostRow], page: i64, total_pages: i64, search: &str) -> String {
    if rows.is_empty() {
        let msg = if search.is_empty() {
            "You haven&rsquo;t saved any posts yet.".to_string()
        } else {
            format!("No saved posts matched &ldquo;{}&rdquo;.", crate::html_escape(search))
        };
        return format!(r#"<p class="muted">{msg}</p>"#);
    }

    let pagination = saved_posts_pagination(page, total_pages, search);

    let row_html: String = rows.iter().map(|r| {
        let unsave_url = derive_unsave_url(&r.post_url);
        format!(
            r#"<tr>
              <td><a href="{url}" target="_blank" rel="noopener noreferrer">{title}</a></td>
              <td style="white-space:nowrap">{saved_at}</td>
              <td class="actions">
                <a href="{url}" class="icon-btn" title="View post" target="_blank" rel="noopener noreferrer">
                  <img src="/admin/static/icons/eye.svg" alt="View">
                </a>
                <form method="post" action="{unsave_url}" style="display:inline"
                      onsubmit="return confirm('Remove this post from your saved list?')">
                  <input type="hidden" name="return_to" value="/account/saved-posts">
                  <button class="icon-btn icon-danger" title="Remove" type="submit">
                    <img src="/admin/static/icons/trash.svg" alt="Remove">
                  </button>
                </form>
              </td>
            </tr>"#,
            url       = crate::html_escape(&r.post_url),
            title     = crate::html_escape(&r.title),
            saved_at  = crate::html_escape(&r.saved_at),
            unsave_url = crate::html_escape(&unsave_url),
        )
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<table class="data-table">
  <thead><tr>
    <th>Post</th>
    <th>Saved</th>
    <th>Actions</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}"#,
        rows       = row_html,
        pagination = pagination,
    )
}

pub fn render_saved_posts(rows: &[SavedPostRow], page: i64, total_pages: i64, search: &str, ctx: &AccountContext) -> String {
    let fragment = saved_posts_list_fragment(rows, page, total_pages, search);

    let script = crate::live_search_script(
        "saved-posts-search",
        "saved-posts-list",
        "/account/saved-posts?partial=1",
    );

    let top_pagination = saved_posts_pagination(page, total_pages, search);
    let search_toggle = crate::pill_search_toggle("saved-posts-search", "Search saved posts&hellip;", search);

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:.75rem">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">{search_toggle}</div>
  <div>{top_pagination}</div>
</div>
<div id="saved-posts-list">{fragment}</div>
{script}
{pill_search_init}"#,
        top_pagination = top_pagination,
        search_toggle  = search_toggle,
        fragment       = fragment,
        script         = script,
        pill_search_init = crate::pill_search_init_script(),
    );

    account_page("Saved Posts", "/account/saved-posts", None, &content, ctx)
}

fn derive_unsave_url(post_url: &str) -> String {
    // post_url is like "http://host/slug" — extract the path and append /unsave
    // Strip the scheme and find the start of the path (first / after host).
    if let Some(after_scheme) = post_url.find("://").map(|i| i + 3) {
        if let Some(path_offset) = post_url[after_scheme..].find('/') {
            let path = &post_url[after_scheme + path_offset..];
            return format!("{}/unsave", path);
        }
    }
    "#".to_string()
}

// ── My Comments ──────────────────────────────────────────────────────────────

pub struct MyCommentRow {
    pub id:            String,
    pub body_preview:  String,
    pub post_title:    String,
    pub post_slug:     String,
    pub site_hostname: String,
    pub created_at:    String,
    pub can_delete:    bool,
}

/// Build pagination HTML for the comments list.
/// Pagination links preserve the active search query so navigating between
/// pages doesn't reset the filter.
fn comments_pagination(page: i64, total_pages: i64, search: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };
    let prev = if page > 1 {
        format!(r#"<a href="/account/my-comments?page={}{}" class="page-btn">&laquo; Prev</a>"#, page - 1, search_qs)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="/account/my-comments?page={}{}" class="page-btn">Next &raquo;</a>"#, page + 1, search_qs)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end   = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(
                r#"<a href="/account/my-comments?page={p}{search_qs}" class="page-btn">{p}</a>"#,
                search_qs = search_qs
            ));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Returns just the inner list HTML (pagination + table).
/// Used both by `render_my_comments` and directly by the live-search
/// fetch() call (`?partial=1`) so JS can swap only the table div.
pub fn comments_list_fragment(rows: &[MyCommentRow], page: i64, total_pages: i64, search: &str) -> String {
    if rows.is_empty() {
        let msg = if search.is_empty() {
            "You haven&rsquo;t made any comments yet.".to_string()
        } else {
            format!("No comments matched &ldquo;{}&rdquo;.", crate::html_escape(search))
        };
        return format!(r#"<p class="muted">{msg}</p>"#);
    }

    let pagination = comments_pagination(page, total_pages, search);

    let row_html: String = rows.iter().map(|r| {
        let delete_btn = if r.can_delete {
            format!(
                r#"<form method="POST" action="/account/comments/{id}/delete" style="display:inline"
                     onsubmit="return confirm('Delete this comment? This cannot be undone.')">
                  <button class="icon-btn icon-danger" title="Delete" type="submit">
                    <img src="/admin/static/icons/trash.svg" alt="Delete">
                  </button>
                </form>"#,
                id = crate::html_escape(&r.id),
            )
        } else {
            String::new()
        };
        format!(
            r#"<tr>
              <td><span class="badge">{hostname}</span></td>
              <td>{post_title}</td>
              <td class="muted" style="font-size:0.85rem">{preview}</td>
              <td style="white-space:nowrap">{date}</td>
              <td class="actions">
                <a href="/{slug}#comments" class="icon-btn" title="View post" target="_blank" rel="noopener noreferrer">
                  <img src="/admin/static/icons/eye.svg" alt="View">
                </a>
                {delete_btn}
              </td>
            </tr>"#,
            hostname   = crate::html_escape(&r.site_hostname),
            post_title = crate::html_escape(&r.post_title),
            preview    = crate::html_escape(&r.body_preview),
            date       = crate::html_escape(&r.created_at),
            slug       = crate::html_escape(&r.post_slug),
            delete_btn = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    // Fragment contains only the table + bottom pagination.
    // Top pagination lives outside the fragment div (alongside the search box)
    // so JS can replace the table without clobbering the search input.
    format!(
        r#"<table class="data-table">
  <thead><tr>
    <th>Site</th>
    <th>Post</th>
    <th>Comment</th>
    <th>Date</th>
    <th>Actions</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
<p class="muted" style="margin-top:0.75rem;font-size:0.8rem">
  Comments can be deleted within 15&nbsp;minutes of posting.
</p>
{pagination}"#,
        rows       = row_html,
        pagination = pagination,
    )
}

pub fn render_my_comments(rows: &[MyCommentRow], page: i64, total_pages: i64, search: &str, ctx: &AccountContext) -> String {
    let fragment = comments_list_fragment(rows, page, total_pages, search);

    // Live-search script — shared helper from crate::live_search_script.
    // Debounces input at 300 ms, fetches ?partial=1&search=... and swaps div#comments-list.
    // Pagination links in the fragment carry &search=... so page navigation preserves the filter.
    // When this page is ported to Leptos, replace with a reactive signal + server function.
    let script = crate::live_search_script(
        "comment-search",
        "comments-list",
        "/account/my-comments?partial=1",
    );

    // Top pagination rendered outside the fragment div so the search input
    // (also outside) is never wiped by the JS live-search swap.
    let top_pagination = comments_pagination(page, total_pages, search);
    let search_toggle = crate::pill_search_toggle("comment-search", "Search comments&hellip;", search);

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:.75rem">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">{search_toggle}</div>
  <div>{top_pagination}</div>
</div>
<div id="comments-list">{fragment}</div>
{script}
{pill_search_init}"#,
        top_pagination = top_pagination,
        search_toggle  = search_toggle,
        fragment       = fragment,
        pill_search_init = crate::pill_search_init_script(),
        script         = script,
    );

    account_page("My Comments", "/account/my-comments", None, &content, ctx)
}
