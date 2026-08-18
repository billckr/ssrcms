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
    let profile_link    = nav_link("/account/profile",      "Profile");

    let site_name         = crate::html_escape(&ctx.site_name);
    let user_email        = crate::html_escape(&ctx.user_email);
    let user_display_name = crate::html_escape(&ctx.user_display_name);

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
  <div class="sidebar-overlay" onclick="closeSidebar()"></div>
  <div class="admin-wrap">
    <nav class="admin-sidebar">
      <a class="brand" href="/account">{site_name}</a>
      <ul>
        {dashboard_link}
        {saved_link}
        {comments_link}
        {profile_link}
      </ul>
      <div class="sidebar-footer">
        <span class="sidebar-user-email">{user_email}</span>
      </div>
    </nav>
    <main class="admin-main">
      <header class="admin-header">
        <button class="hamburger" onclick="toggleSidebar()" aria-label="Open navigation">
          <span></span><span></span><span></span>
        </button>
        <h1>{title}</h1>
        <span class="admin-header-user">{user_display_name}</span>
        <a href="/account/logout" class="icon-btn" title="Log out">
          <img src="/admin/static/icons/log-out.svg" alt="Log out">
        </a>
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
        profile_link  = profile_link,
        user_email  = user_email,
        user_display_name = user_display_name,
        flash_html  = flash_html,
        content     = content,
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
    pub email: String,
    pub display_name: String,
}

pub fn render_profile(data: &ProfileData, flash: Option<&str>, ctx: &AccountContext) -> String {
    let content = format!(
        r#"<div style="max-width:720px">
  <div class="card-boxed">
    <h2 class="card-boxed-header">Profile Management</h2>
    <div class="card-boxed-body">
      <form method="POST" action="/account/profile/update" class="profile-form">
        <fieldset>

          <div class="form-group">
            <label for="display_name">Display Name</label>
            <input type="text" id="display_name" name="display_name" value="{display_name}">
          </div>

          <div class="form-group">
            <label for="email">Email</label>
            <input type="email" id="email" name="email" value="{email}" required>
          </div>
        </fieldset>

        <div class="icon-pill">
          <button type="submit" class="icon-btn" id="profile-save-btn" title="Save Changes" aria-label="Save Changes" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </form>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Password Management</h2>
    <div class="card-boxed-body">
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

        <div class="icon-pill">
          <button type="submit" class="icon-btn" id="password-save-btn" title="Change Password" aria-label="Change Password" disabled>
            <img src="/admin/static/icons/key.svg" alt="">
          </button>
        </div>
      </form>
    </div>
  </div>
</div>
<script>
(function() {{
  var form = document.querySelector('.profile-form');
  var btn  = document.getElementById('profile-save-btn');
  function checkChanged() {{
    var changed = false;
    form.querySelectorAll('input').forEach(function(inp) {{
      if (inp.value !== inp.defaultValue) changed = true;
    }});
    btn.disabled = !changed;
  }}
  form.addEventListener('input', checkChanged);
}})();
(function() {{
  var form = document.querySelector('.password-form');
  var btn  = document.getElementById('password-save-btn');
  function checkFilled() {{
    var filled = true;
    form.querySelectorAll('input[type=password]').forEach(function(inp) {{
      if (!inp.value) filled = false;
    }});
    btn.disabled = !filled;
  }}
  form.addEventListener('input', checkFilled);
}})();
</script>"#,
        email        = crate::html_escape(&data.email),
        display_name = crate::html_escape(&data.display_name),
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

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:.75rem">
  <input id="saved-posts-search"
         type="search"
         placeholder="Search saved posts&hellip;"
         value="{search_val}"
         style="width:100%;max-width:320px;padding:.4rem .75rem;border:1px solid var(--border);border-radius:4px;font-size:14px;background:var(--field-bg);color:var(--field-text)">
  <div>{top_pagination}</div>
</div>
<div id="saved-posts-list">{fragment}</div>
{script}"#,
        top_pagination = top_pagination,
        search_val     = crate::html_escape(search),
        fragment       = fragment,
        script         = script,
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

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:.75rem">
  <input id="comment-search"
         type="search"
         placeholder="Search comments&hellip;"
         value="{search_val}"
         style="width:100%;max-width:320px;padding:.4rem .75rem;border:1px solid var(--border);border-radius:4px;font-size:14px;background:var(--field-bg);color:var(--field-text)">
  <div>{top_pagination}</div>
</div>
<div id="comments-list">{fragment}</div>
{script}"#,
        top_pagination = top_pagination,
        search_val     = crate::html_escape(search),
        fragment       = fragment,
        script         = script,
    );

    account_page("My Comments", "/account/my-comments", None, &content, ctx)
}
