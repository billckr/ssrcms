//! Admin post list and editor pages.

pub struct PostRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub slug: String,
    pub post_type: String,
    pub author_name: String,
    pub published_at: Option<String>,
    pub post_password_set: bool,
    pub site_hostname: String,
}

pub struct PostEdit {
    pub id: Option<String>,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: String,
    pub status: String,
    pub published_at: Option<String>,
    pub post_type: String,
    pub categories: Vec<TermOption>,
    pub tags: Vec<TermOption>,
    pub selected_categories: Vec<String>,
    pub selected_tags: Vec<String>,
    /// Current template override (e.g. "forms/contact"). None = default.
    pub template: Option<String>,
    /// Templates available in the active theme (relative paths without .html).
    pub available_templates: Vec<String>,
    /// UUID of the selected featured image, if any.
    pub featured_image_id: Option<String>,
    /// Public URL for the featured image preview (e.g. "/uploads/abc.png").
    pub featured_image_url: Option<String>,
    /// True if the post currently has a password hash stored (so UI shows checkbox pre-checked).
    pub post_password_set: bool,
    /// Whether comments are currently enabled on this post.
    pub comments_enabled: bool,
    /// Total number of comments on this post (0 for new posts).
    pub comment_count: u64,
    /// Display name of the post author (empty string for new posts).
    pub author_name: String,
    /// Hostname of the site this post belongs to (empty for new posts / global admin context).
    pub site_name: String,
    /// UUID of the parent page (pages only). None = top-level.
    pub parent_id: Option<String>,
    /// (id, title) pairs of published pages on this site, excluding self. For parent dropdown.
    pub available_parents: Vec<(String, String)>,
}

pub struct TermOption {
    pub id: String,
    pub name: String,
}

/// Build pagination controls for the posts/pages list.
/// Preserves `status_qs` (e.g. `"&status=published"`) and `search_qs` across page nav.
fn posts_pagination(base_path: &str, page: i64, total_pages: i64, status_qs: &str, search_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let qs = format!("{status_qs}{search_qs}");
    let prev = if page > 1 {
        format!(r#"<a href="{base_path}?page={}{qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="{base_path}?page={}{qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
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
            nums.push_str(&format!(r#"<a href="{base_path}?page={p}{qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Renders only the table and bottom pagination — the content of `div#posts-list`.
/// Called by `render_list` on full page loads and returned directly for `?partial=1`
/// JS live-search requests so the browser can swap just the table div without a full reload.
pub fn posts_list_fragment(
    posts: &[PostRow],
    post_type: &str,
    page: i64,
    total_pages: i64,
    ctx: &crate::PageContext,
    status_filter: Option<&str>,
    search: &str,
) -> String {
    let edit_prefix = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    let base_path   = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };

    // Only published/scheduled (and the mixed "all") views show a date column.
    let show_date_col = matches!(status_filter, None | Some("") | Some("published") | Some("scheduled"));
    let date_col_label = match status_filter {
        Some("scheduled") => "Scheduled (UTC)",
        Some("published") => "Published (UTC)",
        _ => "Date (UTC)",
    };

    let status_qs = match status_filter {
        Some(s) if !s.is_empty() => format!("&status={}", s),
        _ => String::new(),
    };
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };

    if posts.is_empty() {
        let msg = if search.is_empty() {
            format!("No {}s found.", post_type)
        } else {
            format!("No {}s matched &ldquo;{}&rdquo;.", post_type, crate::html_escape(search))
        };
        return format!(r#"<p class="muted">{msg}</p>"#);
    }

    let rows = posts.iter().map(|p| {
        let path = if p.post_type == "page" {
            format!("/{}", p.slug)
        } else {
            format!("/{}", p.slug)
        };
        let view_href = if ctx.current_site.is_empty() {
            path
        } else {
            format!("//{}{}", ctx.current_site, path)
        };
        // Authors cannot edit scheduled or published posts — show view only.
        let author_read_only = ctx.user_role.eq_ignore_ascii_case("author")
            && (p.status == "scheduled" || p.status == "published");
        let title_cell = if author_read_only {
            format!(r#"<span>{}</span>"#, crate::html_escape(&p.title))
        } else {
            format!(r#"<a href="{prefix}/{id}/edit">{title}</a>"#,
                prefix = edit_prefix, id = crate::html_escape(&p.id), title = crate::html_escape(&p.title))
        };
        let edit_btn = if author_read_only {
            String::new()
        } else {
            format!(r#"<a href="{prefix}/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>"#,
                prefix = edit_prefix, id = crate::html_escape(&p.id))
        };
        // Date cell: only for tabs where it's meaningful.
        let date_td = if show_date_col {
            let val = if p.status == "published" || p.status == "scheduled" {
                p.published_at.as_deref()
                    .map(|d| crate::html_escape(d))
                    .unwrap_or_else(|| "\u{2014}".to_string())
            } else {
                "\u{2014}".to_string()
            };
            format!("<td>{}</td>", val)
        } else {
            String::new()
        };
        // Domain badge — gray pill style.
        let domain_td = {
            let h = crate::html_escape(&p.site_hostname);
            if h.is_empty() {
                r#"<td><span style="color:var(--muted);font-size:0.8rem">—</span></td>"#.to_string()
            } else {
                format!(r#"<td><span style="display:inline-block;background:#e2e8f0;color:#64748b;border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap">{h}</span></td>"#)
            }
        };
        // Column order varies by tab:
        //   Drafts / Pending: Author → Domain
        //   All / Published / Scheduled / Trashed: Author → Domain → Date
        let author_td = format!("<td>{}</td>", crate::html_escape(&p.author_name));
        let middle_tds = match status_filter {
            Some("draft") | Some("pending") => format!("{author_td}{domain_td}"),
            _ => format!("{author_td}{domain_td}{date_td}"),
        };
        let delete_btn = if ctx.user_role.eq_ignore_ascii_case("author") {
            String::new()
        } else {
            format!(
                r#"<form method="POST" action="{prefix}/{id}/delete" style="display:inline" onsubmit="return confirm('Delete this?')">
              <button class="icon-btn icon-danger" title="Delete" type="submit">
                <img src="/admin/static/icons/delete.svg" alt="Delete">
              </button>
            </form>"#,
                prefix = edit_prefix,
                id = crate::html_escape(&p.id),
            )
        };
        format!(
            r#"<tr>
              <td style="width:2rem;text-align:center">
                <input type="checkbox" class="bulk-cb" value="{id}" aria-label="Select">
              </td>
              <td>{title_cell}</td>
              <td><span class="badge badge-{status_cls}">{status_label}</span>{protected_badge}</td>
              {middle_tds}
              <td class="actions">
                <a href="{view_href}" class="icon-btn" title="View" target="_blank" rel="noopener noreferrer">
                  <img src="/admin/static/icons/eye.svg" alt="View">
                </a>
                {edit_btn}
                {delete_btn}
              </td>
            </tr>"#,
            id            = crate::html_escape(&p.id),
            title_cell    = title_cell,
            status_cls    = crate::html_escape(&p.status),
            status_label  = crate::html_escape(if p.status == "pending" { "Pending Review" } else { &p.status }),
            protected_badge = if p.post_password_set { r#" <span class="badge badge-protected" title="Protected">&#x1F512;</span>"# } else { "" },
            middle_tds    = middle_tds,
            view_href     = crate::html_escape(&view_href),
            edit_btn      = edit_btn,
            delete_btn    = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    // Thead middle columns mirror the tbody column ordering.
    let middle_ths = match status_filter {
        Some("draft") | Some("pending") => "<th>Author</th><th>Domain</th>".to_string(),
        _ => {
            let date_th = if show_date_col { format!("<th>{}</th>", date_col_label) } else { String::new() };
            format!("<th>Author</th><th>Domain</th>{date_th}")
        },
    };

    let pagination = posts_pagination(base_path, page, total_pages, &status_qs, &search_qs);

    format!(
        r#"<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all" title="Select all" aria-label="Select all"></th>
    <th>Title</th><th>Status</th>{middle_ths}<th>Actions</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}"#,
        middle_ths = middle_ths,
        rows       = rows,
        pagination = pagination,
    )
}

pub fn render_list(posts: &[PostRow], post_type: &str, page: i64, total_pages: i64, flash: Option<&str>, ctx: &crate::PageContext, status_filter: Option<&str>, pending_count: i64, author_scheduled_count: i64, search: &str) -> String {
    let title     = if post_type == "page" { "Pages" } else { "Posts" };
    let new_label = if post_type == "page" { "New Page" } else { "New Post" };
    let new_href  = if post_type == "page" { "/admin/pages/new" } else { "/admin/posts/new" };
    let base_path = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    let bulk_action = if post_type == "page" { "/admin/pages/bulk-delete" } else { "/admin/posts/bulk-delete" };

    let status_qs = match status_filter {
        Some(s) if !s.is_empty() => format!("&status={}", s),
        _ => String::new(),
    };
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };

    // Top pagination lives outside div#posts-list so the search input (also outside)
    // is never wiped by the JS live-search innerHTML swap.
    let top_pagination = posts_pagination(base_path, page, total_pages, &status_qs, &search_qs);

    // Filter tabs — pages have fewer statuses; authors don't see Trash and only see
    // Scheduled when they actually have scheduled posts.
    let tab_specs: &[(&str, &str)] = if post_type == "page" {
        &[("all", "All"), ("published", "Published"), ("draft", "Draft"), ("trashed", "Trashed")]
    } else if ctx.user_role.eq_ignore_ascii_case("author") {
        if author_scheduled_count > 0 {
            &[("all", "All"), ("published", "Published"), ("draft", "Draft"), ("pending", "Pending Review"), ("scheduled", "Scheduled")]
        } else {
            &[("all", "All"), ("published", "Published"), ("draft", "Draft"), ("pending", "Pending Review")]
        }
    } else {
        &[("all", "All"), ("published", "Published"), ("draft", "Draft"), ("pending", "Pending Review"), ("scheduled", "Scheduled"), ("trashed", "Trashed")]
    };
    let tabs: String = tab_specs.iter().map(|(val, label)| {
        let is_active = match status_filter {
            None | Some("") => *val == "all",
            Some(sf) => *val == sf,
        };
        let active_class = if is_active { " active" } else { "" };
        let href = if *val == "all" {
            base_path.to_string()
        } else {
            format!("{}?status={}", base_path, val)
        };
        let extra = if *val == "pending" && pending_count > 0 {
            format!(
                r#" <span class="badge badge-pending" style="font-size:10px;padding:.05rem .35rem;vertical-align:middle">{}</span>"#,
                pending_count
            )
        } else {
            String::new()
        };
        format!(r#"<a href="{}" class="page-tab{}">{}{}</a>"#, href, active_class, label, extra)
    }).collect();
    let tabs_html = format!(r#"<div class="page-tabs" style="margin-bottom:1.25rem">{}</div>"#, tabs);

    // Fragment: table + bottom pagination — swapped by the live-search JS.
    let fragment = posts_list_fragment(posts, post_type, page, total_pages, ctx, status_filter, search);

    // The live-search fetch URL includes status= so results stay scoped to the current tab.
    let fetch_prefix = format!("{}?partial=1{}", base_path, status_qs);

    let content = format!(
        r#"{tabs_html}
<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:.75rem">
  <a href="{new_href}" class="btn btn-primary">{new_label}</a>
  <button id="bulk-delete-btn" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDelete()">Delete Selected (<span id="bulk-count">0</span>)</button>
</div>
<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:.75rem">
  <div>{top_pagination}</div>
  <input id="post-search"
         type="search"
         placeholder="Search {post_type}s&hellip;"
         value="{search_val}"
         style="width:100%;max-width:320px;padding:.4rem .75rem;border:1px solid var(--border);border-radius:4px;font-size:14px;background:var(--card-bg);color:inherit">
</div>
<div id="posts-list">{fragment}</div>
{live_search}
<script>
(function() {{
  var btn     = document.getElementById('bulk-delete-btn');
  var countEl = document.getElementById('bulk-count');

  function updateBtn() {{
    var checked = document.querySelectorAll('.bulk-cb:checked');
    var n = checked.length;
    var total = document.querySelectorAll('.bulk-cb').length;
    countEl.textContent = n;
    btn.style.display = n > 0 ? '' : 'none';
    // Re-query select-all each call: after a live-search swap the element inside
    // div#posts-list is replaced, so the cached reference would be stale.
    var sa = document.getElementById('select-all');
    if (sa) {{
      sa.indeterminate = n > 0 && n < total;
      sa.checked = n > 0 && n === total;
    }}
  }}

  document.addEventListener('change', function(e) {{
    if (e.target.classList.contains('bulk-cb')) updateBtn();
    if (e.target.id === 'select-all') {{
      document.querySelectorAll('.bulk-cb').forEach(function(cb) {{ cb.checked = e.target.checked; }});
      updateBtn();
    }}
  }});

  window.bulkDelete = function() {{
    var checked = Array.from(document.querySelectorAll('.bulk-cb:checked'));
    if (checked.length === 0) return;
    var noun = checked.length === 1 ? '1 item' : checked.length + ' items';
    if (!confirm('Permanently delete ' + noun + '? This cannot be undone.')) return;
    var form = document.createElement('form');
    form.method = 'POST';
    form.action = '{bulk_action}';
    var input = document.createElement('input');
    input.type = 'hidden';
    input.name = 'ids';
    input.value = checked.map(function(cb) {{ return cb.value; }}).join(',');
    form.appendChild(input);
    document.body.appendChild(form);
    form.submit();
  }};
}})();
</script>"#,
        tabs_html      = tabs_html,
        new_href       = new_href,
        new_label      = new_label,
        top_pagination = top_pagination,
        post_type      = post_type,
        search_val     = crate::html_escape(search),
        fragment       = fragment,
        live_search    = crate::live_search_script("post-search", "posts-list", &fetch_prefix),
        bulk_action    = bulk_action,
    );

    let path = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    crate::admin_page(title, path, flash, &content, ctx)
}

pub fn render_editor(post: &PostEdit, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let is_new = post.id.is_none();
    let title = if is_new {
        if post.post_type == "page" { "New Page".to_string() } else { "New Post".to_string() }
    } else {
        if post.post_type == "page" { "Edit Page".to_string() } else { "Edit Post".to_string() }
    };

    let action = match &post.id {
        Some(id) => {
            if post.post_type == "page" {
                format!("/admin/pages/{}/edit", id)
            } else {
                format!("/admin/posts/{}/edit", id)
            }
        },
        None => {
            if post.post_type == "page" {
                "/admin/pages/new".to_string()
            } else {
                "/admin/posts/new".to_string()
            }
        },
    };

    let cat_options = post.categories.iter().map(|t| {
        let checked = if post.selected_categories.contains(&t.id) { " checked" } else { "" };
        format!(
            r#"<label><input type="checkbox" name="categories" value="{id}"{checked}> {name}</label>"#,
            id = crate::html_escape(&t.id),
            name = crate::html_escape(&t.name),
            checked = checked,
        )
    }).collect::<Vec<_>>().join("\n");

    let tag_options = post.tags.iter().map(|t| {
        let checked = if post.selected_tags.contains(&t.id) { " checked" } else { "" };
        format!(
            r#"<label><input type="checkbox" name="tags" value="{id}"{checked}> {name}</label>"#,
            id = crate::html_escape(&t.id),
            name = crate::html_escape(&t.name),
            checked = checked,
        )
    }).collect::<Vec<_>>().join("\n");

    let status_options = if ctx.user_role.eq_ignore_ascii_case("author") {
        [("draft", "Draft"), ("pending", "Submit for Review")].iter().map(|(val, label)| {
            let selected = if *val == post.status { " selected" } else { "" };
            format!(r#"<option value="{val}"{selected}>{label}</option>"#, val = val, label = label, selected = selected)
        }).collect::<Vec<_>>().join("")
    } else {
        // Editors/admins: include pending so they can see/change it too
        [("draft", "Draft"), ("pending", "Pending Review"), ("published", "Published"), ("scheduled", "Scheduled"), ("trashed", "Trashed")].iter().map(|(val, label)| {
            let selected = if *val == post.status { " selected" } else { "" };
            format!(r#"<option value="{val}"{selected}>{label}</option>"#, val = val, label = label, selected = selected)
        }).collect::<Vec<_>>().join("")
    };

    // Hint displayed below the status dropdown for authors
    let status_hint = if ctx.user_role.eq_ignore_ascii_case("author") {
        r#"<small id="status-hint" style="color:var(--muted);display:block;margin-top:.3rem"></small>
<script>
(function(){
  var sel = document.getElementById('status');
  var hint = document.getElementById('status-hint');
  function update(){hint.textContent=sel.value==='pending'?'An editor will review this post before it goes live.':'';}
  sel.addEventListener('change',update); update();
})();
</script>"#
    } else {
        ""
    };

    // Default published_at:
    // - Authors: always empty (field is hidden, value not user-controlled)
    // - Editors/admins opening a pending post: default to now so they can publish immediately
    // - New posts (non-author): prefill with now
    // - Existing non-pending posts: use stored value
    let published_at = if ctx.user_role.eq_ignore_ascii_case("author") {
        // Authors don't control publish time; send an empty hidden value
        String::new()
    } else if let Some(val) = &post.published_at {
        if post.status == "pending" {
            // Override stale author-set time with current UTC for reviewer convenience
            chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string()
        } else {
            val.clone()
        }
    } else {
        // New post or no stored time — prefill with now
        chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string()
    };

    let template_section = if post.post_type == "page" && !post.available_templates.is_empty() {
        let opts = std::iter::once(("".to_string(), "Default (page.html)".to_string()))
            .chain(post.available_templates.iter().map(|t| (t.clone(), t.clone())))
            .map(|(val, label)| {
                let selected = if post.template.as_deref().unwrap_or("") == val { " selected" } else { "" };
                format!(r#"<option value="{val}"{selected}>{label}</option>"#,
                    val = crate::html_escape(&val),
                    label = crate::html_escape(&label),
                    selected = selected)
            })
            .collect::<Vec<_>>().join("");
        format!(r#"<div class="form-group">
          <label for="template">Template</label>
          <select id="template" name="template">{opts}</select>
          <small>Templates in the active theme's templates/ directory.</small>
        </div>"#, opts = opts)
    } else {
        String::new()
    };

    // Parent page selector — only shown for pages with at least one candidate parent.
    let parent_section = if post.post_type == "page" && !post.available_parents.is_empty() {
        let current_parent = post.parent_id.as_deref().unwrap_or("");
        let opts = std::iter::once(("".to_string(), "— None (top-level) —".to_string()))
            .chain(post.available_parents.iter().map(|(id, title)| (id.clone(), title.clone())))
            .map(|(val, label)| {
                let selected = if val == current_parent { " selected" } else { "" };
                format!(
                    r#"<option value="{val}"{selected}>{label}</option>"#,
                    val = crate::html_escape(&val),
                    label = crate::html_escape(&label),
                    selected = selected,
                )
            })
            .collect::<Vec<_>>().join("");
        format!(
            r#"<div class="form-group">
          <label for="parent_id">Parent Page</label>
          <select id="parent_id" name="parent_id">{opts}</select>
          <small>Set a parent to create a nested page URL.</small>
        </div>"#,
            opts = opts,
        )
    } else {
        // Hidden field to always submit an empty parent_id for pages with no candidates
        if post.post_type == "page" {
            r#"<input type="hidden" name="parent_id" value="">"#.to_string()
        } else {
            String::new()
        }
    };

    let categories_section = if post.post_type != "page" {
        let cat_count = post.selected_categories.len();
        let tag_count = post.selected_tags.len();
        let cat_badge = if cat_count > 0 {
            format!(r#"<span class="inline-media-count">{}</span>"#, cat_count)
        } else { String::new() };
        let tag_badge = if tag_count > 0 {
            format!(r#"<span class="inline-media-count">{}</span>"#, tag_count)
        } else { String::new() };
        format!(r#"<details class="form-section">
          <summary>
            <span>Categories</span>
            {cat_badge}
            <svg class="section-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
          </summary>
          <div class="form-section-body">
            <div class="checkbox-group">{cat_options}</div>
          </div>
        </details>
        <details class="form-section">
          <summary>
            <span>Tags</span>
            {tag_badge}
            <svg class="section-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
          </summary>
          <div class="form-section-body">
            <div class="checkbox-group">{tag_options}</div>
          </div>
        </details>"#,
            cat_badge = cat_badge,
            tag_badge = tag_badge,
            cat_options = cat_options,
            tag_options = tag_options,
        )
    } else {
        String::new()
    };

    let featured_image_id_val = post.featured_image_id.as_deref().unwrap_or("");
    let featured_image_url_val = post.featured_image_url.as_deref().unwrap_or("");
    let fi_box_inner = if let Some(url) = &post.featured_image_url {
        format!(
            r#"<img src="{}" alt="Featured image" style="width:100%;height:100%;object-fit:cover;display:block">"#,
            crate::html_escape(url)
        )
    } else {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" style="opacity:.35"><rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10.5" r="1.5"/><path d="M3 16l4.5-4.5 3 3 2.5-2.5 5 5"/></svg><span style="color:var(--muted);font-size:12px">No image selected</span>"#.to_string()
    };
    let has_image_class = if post.featured_image_url.is_some() { " has-image" } else { "" };
    let remove_display = if post.featured_image_url.is_some() { "" } else { "display:none" };
    let featured_image_section = format!(
        r#"<div class="form-section">
      <h3>Featured Image</h3>
      <input type="hidden" id="featured_image_id" name="featured_image_id" value="{id_val}">
      <input type="hidden" id="featured_image_url_field" name="featured_image_url" value="{url_val}">
      <div class="featured-image-box{has_image_class}" id="featured-image-box">{fi_box_inner}</div>
      <button type="button" id="fi-remove-btn" class="featured-image-remove" onclick="removeFeaturedImage()" style="{remove_display}">&#x2715; Remove featured image</button>
      <button type="button" class="btn btn-primary" style="width:100%;font-size:12px;margin-top:.5rem" onclick="openMediaPicker()">Set Featured Image</button>
    </div>"#,
        id_val = crate::html_escape(featured_image_id_val),
        url_val = crate::html_escape(featured_image_url_val),
        has_image_class = has_image_class,
        fi_box_inner = fi_box_inner,
        remove_display = remove_display,
    );

    let protected_checked = if post.post_password_set { "checked" } else { "" };
    let pw_group_display  = if post.post_password_set { "" } else { "display:none" };
    let pw_placeholder = if post.post_password_set { "Leave blank to keep current password" } else { "Enter password" };
    let pw_hint = if post.post_password_set {
        r#"<small style="color:var(--muted)">Leave blank to keep existing password.</small>"#
    } else { "" };

    let password_section = if ctx.user_role.eq_ignore_ascii_case("author") {
        String::new()
    } else {
        format!(
            r#"<div class="form-group" style="margin-bottom:.5rem">
          <label style="display:flex;align-items:center;gap:.5rem;cursor:pointer;font-weight:400">
            <input type="checkbox" id="post-protected-cb" name="post_protected" value="on" {protected_checked}
              onchange="document.getElementById('post-pw-group').style.display=this.checked?'':'none'">
            Password Protected
          </label>
        </div>
        <div class="form-group" id="post-pw-group" style="{pw_group_display}">
          <label for="post-password" style="font-size:12px">Password</label>
          <input type="password" id="post-password" name="post_password" autocomplete="new-password" placeholder="{pw_placeholder}" style="font-size:13px">
          {pw_hint}
        </div>"#,
            protected_checked = protected_checked,
            pw_group_display = pw_group_display,
            pw_placeholder = pw_placeholder,
            pw_hint = pw_hint,
        )
    };

    // Comments control: only editors/admins can toggle; authors see nothing here.
    let comments_section = if ctx.user_role.eq_ignore_ascii_case("author") {
        String::new()
    } else {
        let enabled_sel  = if post.comments_enabled  { " selected" } else { "" };
        let disabled_sel = if !post.comments_enabled { " selected" } else { "" };
        let count_badge = if post.comment_count > 0 {
            format!(
                r#" <span class="badge badge-pending" title="{n} comment{s}">{n}</span>"#,
                n = post.comment_count,
                s = if post.comment_count == 1 { "" } else { "s" },
            )
        } else {
            String::new()
        };
        format!(
            r#"<div class="form-group" style="margin-top:.75rem">
          <label for="comments-enabled" style="font-size:12px">Comments{count_badge}</label>
          <select id="comments-enabled" name="comments_enabled" style="font-size:13px">
            <option value="false"{disabled_sel}>Disabled</option>
            <option value="true"{enabled_sel}>Allowed</option>
          </select>
        </div>"#,
            count_badge  = count_badge,
            disabled_sel = disabled_sel,
            enabled_sel  = enabled_sel,
        )
    };

    // Author card: shown to editors/admins when viewing an existing post written by someone else.
    let author_card = if !ctx.user_role.eq_ignore_ascii_case("author") && !post.author_name.is_empty() {
        let site_line = if !post.site_name.is_empty() {
            format!(
                r#"<div class="author-card-site">{}</div>"#,
                crate::html_escape(&post.site_name)
            )
        } else {
            String::new()
        };
        format!(
            r#"<div class="form-section author-card">
      <h3>Author</h3>
      <div class="author-card-name">{name}</div>
      {site_line}
    </div>"#,
            name = crate::html_escape(&post.author_name),
            site_line = site_line,
        )
    } else {
        String::new()
    };

    let inline_media_section = r#"<details class="form-section">
      <summary>
        <span>Inline Media</span>
        <span id="inline-media-count" class="inline-media-count" style="display:none"></span>
        <svg class="section-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
      </summary>
      <div class="form-section-body">
        <div id="inline-media-list"><p style="color:var(--muted);font-size:12px;margin:0">No media embedded yet.</p></div>
      </div>
    </details>"#;

    let mut content = format!(
        r#"<link rel="stylesheet" href="/admin/static/quill/quill.snow.css">
<form method="POST" action="{action}">
  <div class="editor-layout">
    <div class="editor-main">
      <div style="display:grid;grid-template-columns:1fr auto;gap:.75rem;align-items:start;margin-bottom:1.25rem">
        <div class="form-group" style="margin:0">
          <label for="title">Title <span style="color:var(--danger);font-weight:700">*</span></label>
          <input type="text" id="title" name="title" value="{title_val}" required class="title-input" maxlength="255"{autofocus}>
          <small id="title-count" style="color:var(--muted)">255 remaining</small>
        </div>
        <div class="form-group" style="margin:0;min-width:200px;max-width:280px">
          <label for="slug">Slug</label>
          <input type="text" id="slug" name="slug" value="{slug}" maxlength="200"
            onkeydown="if(event.key===' '){{ event.preventDefault(); var i=this.selectionStart; this.value=this.value.slice(0,i)+'-'+this.value.slice(this.selectionEnd); this.selectionStart=this.selectionEnd=i+1; }}"
            onblur="this.value=this.value.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');">
        </div>
      </div>
      <div class="form-group" style="margin-bottom:1.25rem">
        <label for="excerpt">Excerpt <span style="color:var(--danger);font-weight:700">*</span> <small style="font-weight:400;color:var(--muted)">Used as meta description — required for SEO</small></label>
        <textarea id="excerpt" name="excerpt" rows="3" required maxlength="500" style="resize:none">{excerpt}</textarea>
        <small id="excerpt-count" style="color:var(--muted)">500 remaining</small>
      </div>
      <div class="form-group">
        <label>Content <span style="color:var(--danger);font-weight:700">*</span></label>
        <div id="quill-editor" style="height:620px;background:#fff;font-size:1rem"></div>
        <input type="hidden" id="content" name="content">
      </div>
    </div>
    <div class="editor-sidebar">
      {author_card}
      <div class="form-section">
        <h3>Publish</h3>
        <div class="form-group">
          <label for="status">Status</label>
          <select id="status" name="status">{status_options}</select>
          {status_hint}
        </div>
        <div class="form-group">
          {datetime_field}
        </div>
        {password_section}
        {comments_section}
        <input type="hidden" name="post_type" value="{post_type}">
        <button type="submit" class="btn btn-primary">Save</button>
      </div>
      {featured_image_section}
      {inline_media_section}
      {template_section}
      {parent_section}
      {categories_section}
    </div>
  </div>
</form>
<script src="/admin/static/quill/quill.min.js"></script>
<script>
(function() {{
  // Register a custom Quill format for <audio controls> embeds.
  // BlockEmbed is an ES6 class; must use 'class extends' — calling it via
  // .apply() (ES5 pattern) throws "cannot invoke without 'new'" at instantiation.
  var BlockEmbed = Quill.import('blots/block/embed');
  class AudioBlot extends BlockEmbed {{}}
  AudioBlot.blotName = 'audio';
  AudioBlot.tagName  = 'audio';
  AudioBlot.create   = function(src) {{
    var node = document.createElement('audio');
    node.setAttribute('src', src);
    node.setAttribute('controls', '');
    return node;
  }};
  AudioBlot.value = function(node) {{ return node.getAttribute('src'); }};
  Quill.register('formats/audio', AudioBlot);

  var quill = new Quill('#quill-editor', {{
    theme: 'snow',
    modules: {{
      toolbar: [
        [{{ header: [1, 2, 3, false] }}],
        ['bold', 'italic', 'underline', 'strike'],
        ['blockquote', 'code-block'],
        [{{ list: 'ordered' }}, {{ list: 'bullet' }}],
        ['link', 'image'],
        ['clean']
      ]
    }}
  }});

  // Load existing content.
  // Use clipboard.convert → setContents so that registered blots (e.g. AudioBlot)
  // are reconstructed from their tag names rather than stripped by the HTML sanitiser
  // that dangerouslyPasteHTML applies before inserting.
  var existing = document.getElementById('content').value;
  if (!existing) {{
    existing = {content_js};
  }}
  if (existing) {{
    quill.setContents(quill.clipboard.convert(existing), 'silent');
  }}

  // On submit, copy Quill HTML into the hidden input and validate excerpt
  document.querySelector('form').addEventListener('submit', function(e) {{
    document.getElementById('content').value = quill.root.innerHTML;
    var excerpt = document.getElementById('excerpt').value.trim();
    if (!excerpt) {{
      e.preventDefault();
      var el = document.getElementById('excerpt');
      el.focus();
      el.style.borderColor = 'var(--danger)';
      el.setAttribute('placeholder', 'Excerpt is required — describe this post in 1–2 sentences.');
      el.addEventListener('input', function() {{ el.style.borderColor = ''; }}, {{ once: true }});
    }}
  }});

  // ── Inline Media panel ───────────────────────────────────────────────
  function escHtmlEditor(s) {{
    return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }}
  window.refreshInlineMediaList = function() {{
    var list = document.getElementById('inline-media-list');
    if (!list) return;
    var items = [];
    quill.root.querySelectorAll('audio').forEach(function(el) {{
      var src = el.getAttribute('src') || '';
      items.push({{ kind: 'audio', filename: src.split('/').pop() || src }});
    }});
    quill.root.querySelectorAll('img[src^="/uploads/"]').forEach(function(el) {{
      var src = el.getAttribute('src') || '';
      items.push({{ kind: 'image', filename: src.split('/').pop() || src }});
    }});
    var badge = document.getElementById('inline-media-count');
    if (badge) {{
      if (items.length > 0) {{ badge.textContent = items.length; badge.style.display = ''; }}
      else {{ badge.style.display = 'none'; }}
    }}
    if (items.length === 0) {{
      list.innerHTML = '<p style="color:var(--muted);font-size:12px;margin:0">No media embedded yet.</p>';
      return;
    }}
    var labels = {{ audio: 'AUD', image: 'IMG', doc: 'DOC', video: 'VID' }};
    var html = '<ul style="list-style:none;margin:0;padding:0">';
    items.forEach(function(item) {{
      var label = labels[item.kind] || 'DOC';
      html += '<li style="display:flex;align-items:center;gap:.4rem;padding:.35rem 0;border-bottom:1px solid var(--border)">'
        + '<span style="flex-shrink:0;display:inline-block;background:#f3f4f6;color:#374151;border-radius:4px;padding:.1rem .35rem;font-size:.6rem;font-weight:600;letter-spacing:.04em">' + label + '</span>'
        + '<span style="font-size:.75rem;color:var(--muted);word-break:break-all">' + escHtmlEditor(item.filename) + '</span>'
        + '</li>';
    }});
    html += '</ul>';
    list.innerHTML = html;
  }};
  refreshInlineMediaList();
  quill.on('text-change', function() {{ refreshInlineMediaList(); }});

  // Override Quill's image button to open the media library instead of file picker
  window._quillInstance = quill;
  window._quillRange = null;
  var toolbar = quill.getModule('toolbar');
  toolbar.addHandler('image', function() {{
    window._quillRange = quill.getSelection(true);
    openMediaPicker('inline');
  }});

  // Add custom audio button to the Quill toolbar
  (function() {{
    var qlToolbar = document.querySelector('.ql-toolbar');
    if (!qlToolbar) return;
    var span = document.createElement('span');
    span.className = 'ql-formats';
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.title = 'Insert audio';
    btn.style.cssText = 'width:auto;padding:0 4px';
    btn.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>';
    btn.addEventListener('click', function() {{
      window._quillRange = quill.getSelection(true);
      openMediaPicker('audio');
    }});
    span.appendChild(btn);
    qlToolbar.appendChild(span);
  }})();

  // Remaining character counters for title and excerpt
  (function() {{
    function initCount(inputId, countId, max) {{
      var el = document.getElementById(inputId);
      var counter = document.getElementById(countId);
      if (!el || !counter) return;
      function update() {{
        var remaining = max - el.value.length;
        counter.textContent = remaining + ' remaining';
        counter.style.color = remaining <= 20 ? 'var(--danger)' : 'var(--muted)';
      }}
      el.addEventListener('input', update);
      update();
    }}
    initCount('title',   'title-count',   255);
    initCount('excerpt', 'excerpt-count', 500);
  }})();

  // ── Accordion checkbox counters (Categories / Tags) ──────────────────
  document.querySelectorAll('details.form-section .checkbox-group').forEach(function(group) {{
    var details = group.closest('details.form-section');
    if (!details) return;
    var summary = details.querySelector('summary');
    if (!summary) return;
    var chevron = summary.querySelector('.section-chevron');

    function syncBadge() {{
      var total = group.querySelectorAll('input[type=checkbox]:checked').length;
      var badge = summary.querySelector('.inline-media-count');
      if (total > 0) {{
        if (!badge) {{
          badge = document.createElement('span');
          badge.className = 'inline-media-count';
          summary.insertBefore(badge, chevron);
        }}
        badge.textContent = total;
      }} else if (badge) {{
        badge.remove();
      }}
    }}

    group.addEventListener('change', syncBadge);
  }});
}})();
</script>"#,
        action = action,
        autofocus = if is_new { " autofocus" } else { "" },
        title_val = crate::html_escape(&post.title),
        slug = crate::html_escape(&post.slug),
        content_js = serde_json::to_string(&post.content).unwrap_or_else(|_| "\"\"".into()),
        excerpt = crate::html_escape(&post.excerpt),
        status_options = status_options,
        status_hint = status_hint,
        datetime_field = if ctx.user_role.eq_ignore_ascii_case("author") {
            format!(r#"<input type="hidden" name="published_at" value="{}">"#, crate::html_escape(&published_at))
        } else {
            format!(
                r#"<label for="published_at">Date and Time (UTC)</label>
          <input type="datetime-local" id="published_at" name="published_at" value="{}">"#,
                crate::html_escape(&published_at)
            )
        },
        post_type = crate::html_escape(&post.post_type),
        template_section = template_section,
        parent_section = parent_section,
        categories_section = categories_section,
        featured_image_section = featured_image_section,
        inline_media_section = inline_media_section,
        password_section = password_section,
        comments_section = comments_section,
        author_card = author_card,
    );

    let path = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    content.push_str(&crate::media_picker_modal_html());
    crate::admin_page(&title, path, flash, &content, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(post_type: &str, slug: &str) -> PostRow {
        PostRow {
            id: "abc123".to_string(),
            title: "Test".to_string(),
            status: "published".to_string(),
            slug: slug.to_string(),
            post_type: post_type.to_string(),
            author_name: "Author".to_string(),
            published_at: None,
            post_password_set: false,
            site_hostname: String::new(),
        }
    }

    fn make_ctx() -> crate::PageContext {
        crate::PageContext {
            current_site: String::new(),
            user_email: "test@example.com".to_string(),
            user_role: "admin".to_string(),
            is_global_admin: false,
            visiting_foreign_site: false,
            can_manage_users: false,
            can_manage_sites: false,
            can_manage_plugins: false,
            can_manage_settings: false,
            can_manage_content: true,
            can_manage_appearance: false,
            can_manage_taxonomies: false,
            can_manage_forms: false,
            can_manage_pages: true,
            unread_forms_count: 0,
            pending_review_count: 0,
            app_name: "Synaptic".to_string(),
        }
    }

    #[test]
    fn post_view_link_uses_blog_prefix() {
        let html = render_list(&[make_row("post", "my-post")], "post", 1, 1, None, &make_ctx(), None, 0, 0, "");
        assert!(html.contains("href=\"/my-post\""), "post view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn page_view_link_uses_root_prefix() {
        let html = render_list(&[make_row("page", "about")], "page", 1, 1, None, &make_ctx(), None, 0, 0, "");
        assert!(html.contains("href=\"/about\""), "page view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn view_icon_present_in_both_post_and_page_lists() {
        let post_html = render_list(&[make_row("post", "hello")], "post", 1, 1, None, &make_ctx(), None, 0, 0, "");
        let page_html = render_list(&[make_row("page", "hello")], "page", 1, 1, None, &make_ctx(), None, 0, 0, "");
        assert!(post_html.contains("eye.svg"), "post list should include eye icon");
        assert!(page_html.contains("eye.svg"), "page list should include eye icon");
    }
}
