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
    /// Path this row's View link should point to — for posts, built from the
    /// site's configured `permalink_structure` (see `models::post::build_permalink`);
    /// for pages, the flat `/{slug}` path.
    pub view_path: String,
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
    /// Whether the author's account is currently active (false = suspended).
    pub author_is_active: bool,
    /// UUID of the post author (empty string for new posts), used to link the
    /// Author card to their user edit page.
    pub author_id: String,
    /// Hostname of the site this post belongs to (empty for new posts / global admin context).
    pub site_name: String,
    /// UUID of the site this post belongs to (empty for new posts / global admin
    /// context), used to link the Author card's site name to that site's settings.
    pub site_id: String,
    /// UUID of the parent page (pages only). None = top-level.
    pub parent_id: Option<String>,
    /// (id, title) pairs of published pages on this site, excluding self. For parent dropdown.
    pub available_parents: Vec<(String, String)>,
    /// Source URLs attached to this post.
    pub sources: Vec<String>,
    /// Whether the sources list is shown on the live page.
    pub sources_public: bool,
    /// Relative path to the live post/page (e.g. "/my-post"), only set when
    /// currently published. None for drafts/new posts — there's nothing
    /// public to view yet.
    pub live_url: Option<String>,
    /// Relative path to preview a draft/pending/scheduled post/page, only
    /// viewable by a logged-in staff session (see can_preview_site). None
    /// when published (use live_url instead) or trashed.
    pub preview_url: Option<String>,
    /// (slug, name) pairs for every form defined in Form Designer — powers
    /// the editor's "Insert Form" picker.
    pub saved_forms: Vec<(String, String)>,
    /// (slug, name) pairs for every poll defined in Poll Designer — powers
    /// the editor's "Insert Poll" picker.
    pub saved_polls: Vec<(String, String)>,
    /// (form slug, form name, submission count) for each distinct form
    /// embedded in this post's content. Empty means "show nothing" — the
    /// sidebar section and the Publish Options pill's results link are only
    /// rendered when this is non-empty (see render_editor).
    pub form_analytics: Vec<(String, String, i64)>,
    /// When this post/page was first created. None for a new, unsaved post.
    pub created_at: Option<String>,
    /// When it was last saved. None for a new, unsaved post.
    pub updated_at: Option<String>,
}

pub struct TermOption {
    pub id: String,
    pub name: String,
}

/// Build pagination controls for the posts/pages list.
/// Preserves `status_qs` (e.g. `"&status=published"`), `search_qs`, and `sort_qs` across page nav.
fn posts_pagination(base_path: &str, page: i64, total_pages: i64, status_qs: &str, search_qs: &str, sort_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let qs = format!("{status_qs}{search_qs}{sort_qs}");
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
    sort: Option<&str>,
    dir: Option<&str>,
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
    let sort_qs = match sort {
        Some(s) if !s.is_empty() => format!("&sort={}&dir={}", s, dir.unwrap_or("desc")),
        _ => String::new(),
    };

    if posts.is_empty() {
        let noun = match status_filter {
            Some("draft")     => format!("draft {}s", post_type),
            Some("pending")   => format!("{}s pending review", post_type),
            Some("scheduled") => format!("scheduled {}s", post_type),
            Some("published") => format!("published {}s", post_type),
            Some("trashed")   => format!("trashed {}s", post_type),
            _                 => format!("{}s", post_type),
        };
        let msg = if search.is_empty() {
            format!("No {} found.", noun)
        } else {
            format!("No {} matched &ldquo;{}&rdquo;.", noun, crate::html_escape(search))
        };
        return format!(r#"<p class="muted">{msg}</p>"#);
    }

    let rows = posts.iter().map(|p| {
        let view_href = if ctx.current_site.is_empty() {
            p.view_path.clone()
        } else {
            format!("//{}{}", ctx.current_site, p.view_path)
        };
        // Authors cannot edit scheduled or published posts — show view only.
        let author_read_only = ctx.user_role.eq_ignore_ascii_case("author")
            && (p.status == "scheduled" || p.status == "published");
        let display_title = if p.title.chars().count() > 100 {
            format!("{}...", p.title.chars().take(100).collect::<String>())
        } else {
            p.title.clone()
        };
        let title_cell = if author_read_only {
            format!(r#"<span title="{full}">{title}</span>"#,
                full = crate::html_escape(&p.title), title = crate::html_escape(&display_title))
        } else {
            format!(r#"<a href="{prefix}/{id}/edit" title="{full}">{title}</a>"#,
                prefix = edit_prefix, id = crate::html_escape(&p.id),
                full = crate::html_escape(&p.title), title = crate::html_escape(&display_title))
        };
        let edit_btn = if author_read_only {
            String::new()
        } else {
            format!(r#"<a href="{prefix}/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>"#,
                prefix = edit_prefix, id = crate::html_escape(&p.id))
        };
        // View button: only for published/scheduled posts
        let view_btn = if p.status == "published" || p.status == "scheduled" {
            format!(r#"<a href="{view_href}" class="icon-btn" title="View" target="_blank" rel="noopener noreferrer">
                  <img src="/admin/static/icons/eye.svg" alt="View">
                </a>"#,
                view_href = crate::html_escape(&view_href))
        } else {
            String::new()
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
                format!(r#"<td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap">{h}</span></td>"#)
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
                <img src="/admin/static/icons/trash.svg" alt="Delete">
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
                <div class="icon-pill-actionbuttons">
                  {view_btn}
                  {edit_btn}
                  {delete_btn}
                </div>
              </td>
            </tr>"#,
            id            = crate::html_escape(&p.id),
            title_cell    = title_cell,
            status_cls    = crate::html_escape(&p.status),
            status_label  = crate::html_escape(if p.status == "pending" { "Pending Review" } else { &p.status }),
            protected_badge = if p.post_password_set {
                r#" <span class="badge badge-protected" title="This post is password protected"><svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align:-1px"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect><path d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg></span>"#
            } else { "" },
            middle_tds    = middle_tds,
            view_btn      = view_btn,
            edit_btn      = edit_btn,
            delete_btn    = delete_btn,
        )
    }).collect::<Vec<_>>().join("\n");

    // Sortable column header: link toggles asc/desc for that column, preserving the
    // current status/search filters and resetting to page 1 (a new sort is a new view).
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == Some(key);
        let next_dir = if is_active && dir == Some("asc") { "desc" } else { "asc" };
        let arrow = if is_active {
            if dir == Some("asc") { " \u{25B2}" } else { " \u{25BC}" }
        } else {
            ""
        };
        format!(
            r#"<th><a href="{base_path}?sort={key}&dir={next_dir}{status_qs}{search_qs}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    // Thead middle columns mirror the tbody column ordering.
    let middle_ths = match status_filter {
        Some("draft") | Some("pending") => format!("{}{}", sort_th("Author", "author"), sort_th("Domain", "domain")),
        _ => {
            let date_th = if show_date_col { sort_th(date_col_label, "date") } else { String::new() };
            format!("{}{}{}", sort_th("Author", "author"), sort_th("Domain", "domain"), date_th)
        },
    };

    let pagination = posts_pagination(base_path, page, total_pages, &status_qs, &search_qs, &sort_qs);

    format!(
        r#"<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all" title="Select all" aria-label="Select all"></th>
    {title_th}{status_th}{middle_ths}<th>Actions</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}"#,
        title_th   = sort_th("Title", "title"),
        status_th  = sort_th("Status", "status"),
        middle_ths = middle_ths,
        rows       = rows,
        pagination = pagination,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_list(posts: &[PostRow], post_type: &str, page: i64, total_pages: i64, flash: Option<&str>, ctx: &crate::PageContext, status_filter: Option<&str>, pending_count: i64, author_scheduled_count: i64, search: &str, sort: Option<&str>, dir: Option<&str>) -> String {
    let title     = if post_type == "page" { "Pages" } else { "Posts" };
    let new_label = if post_type == "page" { "New Page" } else { "New Post" };
    let new_href  = if post_type == "page" { "/admin/pages/new" } else { "/admin/posts/new" };
    let base_path = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    let bulk_action = if post_type == "page" { "/admin/pages/bulk-delete" } else { "/admin/posts/bulk-delete" };

    let status_qs = match status_filter {
        Some(s) if !s.is_empty() => format!("&status={}", s),
        _ => String::new(),
    };
    let sort_qs = match sort {
        Some(s) if !s.is_empty() => format!("&sort={}&dir={}", s, dir.unwrap_or("desc")),
        _ => String::new(),
    };

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
                r#" <span class="badge" style="margin-left:.35rem;font-size:.75rem;padding:.1rem .45rem">{}</span>"#,
                pending_count
            )
        } else {
            String::new()
        };
        format!(r#"<a href="{}" class="page-tab{}">{}{}</a>"#, href, active_class, label, extra)
    }).collect();
    let tabs_html = format!(r#"<div class="page-tabs" style="margin-bottom:0">{}</div>"#, tabs);

    // Fragment: table + bottom pagination — swapped by the live-search JS.
    let fragment = posts_list_fragment(posts, post_type, page, total_pages, ctx, status_filter, search, sort, dir);

    // The live-search fetch URL includes status=/sort= so results stay scoped to the
    // current tab and column sort.
    let fetch_prefix = format!("{}?partial=1{}{}", base_path, status_qs, sort_qs);
    let search_placeholder = format!("Search {}s&hellip;", post_type);
    let search_toggle = crate::pill_search_toggle("post-search", &search_placeholder, search);

    let content = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:space-between;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  {tabs_html}
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">
    <button id="bulk-delete-btn" type="button" class="icon-btn icon-danger icon-danger-armed" style="display:none" title="Delete Selected" aria-label="Delete Selected" onclick="bulkDelete()">
      <img src="/admin/static/icons/trash.svg" alt="">
    </button>
    {search_toggle}
    <a href="{new_href}" class="icon-btn" title="{new_label}" aria-label="{new_label}"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
</div>
<div id="posts-list">{fragment}</div>
{live_search}
{pill_search_init}
<script>
(function() {{
  var btn     = document.getElementById('bulk-delete-btn');

  function updateBtn() {{
    var checked = document.querySelectorAll('.bulk-cb:checked');
    var n = checked.length;
    var total = document.querySelectorAll('.bulk-cb').length;
    btn.title = 'Delete Selected (' + n + ')';
    btn.setAttribute('aria-label', btn.title);
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
        search_toggle  = search_toggle,
        fragment       = fragment,
        live_search    = crate::live_search_script("post-search", "posts-list", &fetch_prefix),
        pill_search_init = crate::pill_search_init_script(),
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
        let display_title = if post.title.chars().count() > 150 {
            format!("{}...", post.title.chars().take(150).collect::<String>())
        } else {
            post.title.clone()
        };
        // Not escaped here — admin_page() escapes the whole title once
        // already (for both <title> and <h1>); escaping it again here would
        // double-escape entities like &#x27; into literal text.
        format!("Editing - {}", display_title)
    };
    let publish_options_label = if post.post_type == "page" { "Page Options" } else { "Post Options" };

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
        // Editors/admins: include pending so they can see/change it too.
        // Trashed only makes sense once a post exists to trash — a brand-new,
        // never-saved post has nothing for it to do, and Delete already
        // covers removing real content.
        let mut opts: Vec<(&str, &str)> = vec![("draft", "Draft"), ("pending", "Pending Review"), ("published", "Published"), ("scheduled", "Scheduled")];
        if !is_new {
            opts.push(("trashed", "Trashed"));
        }
        opts.iter().map(|(val, label)| {
            let selected = if *val == post.status { " selected" } else { "" };
            format!(r#"<option value="{val}"{selected}>{label}</option>"#, val = val, label = label, selected = selected)
        }).collect::<Vec<_>>().join("")
    };

    let live_url_link = match &post.live_url {
        Some(url) => format!(
            r#"<a href="{url}" class="icon-btn" title="View live" aria-label="View live" target="_blank" rel="noopener"><img src="/admin/static/icons/eye.svg" alt=""></a>"#,
            url = crate::html_escape(url),
        ),
        None => match &post.preview_url {
            Some(url) => format!(
                r#"<a href="{url}" class="icon-btn" title="Preview" aria-label="Preview" target="_blank" rel="noopener"><img src="/admin/static/icons/eye.svg" alt=""></a>"#,
                url = crate::html_escape(url),
            ),
            None => String::new(),
        },
    };

    // On an existing post, the status is usually left as-is — show it as
    // plain text (matching the Originally posted/Last updated styling)
    // instead of a dropdown that's always "armed" to be changed. Change
    // Status/View, down by Save, reveal the real <select> when clicked. A
    // brand-new post never had a prior status to fall back to display, and
    // has nothing to view yet either, so it just shows the dropdown
    // outright with no separate actions needed.
    let status_select_display = if is_new { "" } else { "display:none" };
    let status_readonly = if is_new {
        String::new()
    } else {
        let label = match post.status.as_str() {
            "draft" => "Draft",
            "pending" => "Pending Review",
            "published" => "Published",
            "scheduled" => "Scheduled",
            "trashed" => "Trashed",
            other => other,
        };
        format!(
            r#"<div id="status-readonly">
            <label style="display:block;font-weight:500;margin-bottom:.35rem;font-size:13px">Status</label>
            <div style="font-size:13px;color:var(--muted)">{label}</div>
          </div>"#,
            label = crate::html_escape(label),
        )
    };
    // Links to the first embedded form's results page (see fetch_form_analytics)
    // — only shown when this post actually has a form embedded. Multiple
    // distinct forms in one post is rare enough not to warrant a picker here;
    // the "Form Analytics" sidebar section below lists all of them by name.
    let form_metrics_link = match post.form_analytics.first() {
        Some((slug, _name, _count)) => format!(
            r#"<a href="/admin/form-data-analytics/{slug}" class="icon-btn" title="View form results" aria-label="View form results" target="_blank" rel="noopener"><img src="/admin/static/icons/send.svg" alt=""></a>"#,
            slug = crate::html_escape(slug),
        ),
        None => String::new(),
    };
    // Shares the Save pill, only for an existing post (nothing to change
    // status on or view/preview for a post that hasn't been saved yet).
    let status_actions_pill = if is_new {
        String::new()
    } else {
        format!(
            r#"<button type="button" class="icon-btn" id="status-edit-btn" title="Change status" aria-label="Change status">
              <img src="/admin/static/icons/edit.svg" alt="">
            </button>
            {form_metrics_link}
            {live_url_link}"#,
            form_metrics_link = form_metrics_link,
            live_url_link = live_url_link,
        )
    };

    // Date/Time is only meaningful once Scheduled is picked — hidden the
    // rest of the time to keep an infrequently-needed field from cluttering
    // Publish Options on every post/page. When it's hidden, show the
    // original-post/last-updated dates in its place instead of leaving the
    // spot empty (nothing to show yet on a new, unsaved post).
    let is_scheduled = post.status == "scheduled";
    let datetime_picker_display = if is_scheduled { "" } else { "display:none" };
    // Nothing to show at all yet on a brand-new, unsaved, non-scheduled
    // post — hide the whole section rather than leaving an empty box.
    let datetime_section_display = if !is_scheduled && post.created_at.is_none() { "display:none" } else { "" };
    // One label/value block per embedded form (see fetch_form_analytics) —
    // named per-form ("Form Analytics — {name}") when there's more than one,
    // since summing submission counts across different forms wouldn't mean
    // anything. Nothing rendered at all when the post has no form embedded.
    let form_analytics_html: String = post.form_analytics.iter().map(|(_slug, name, count)| {
        let label = if post.form_analytics.len() > 1 {
            format!("Form Analytics — {}", crate::html_escape(name))
        } else {
            "Form Analytics".to_string()
        };
        format!(
            r#"<div style="margin-top:.6rem">
              <label style="display:block;font-weight:500;margin-bottom:.35rem;font-size:13px">{label}:</label>
              <div style="font-size:13px;color:var(--muted)">{count} result{plural}</div>
            </div>"#,
            label = label,
            count = count,
            plural = if *count == 1 { "" } else { "s" },
        )
    }).collect();

    let post_dates_info = match (&post.created_at, &post.updated_at) {
        (Some(created), Some(updated)) => format!(
            r#"<div id="post-dates-info" style="{display}">
            <div style="margin-bottom:.6rem">
              <label style="display:block;font-weight:500;margin-bottom:.35rem;font-size:13px">Originally posted:</label>
              <div style="font-size:13px;color:var(--muted)">{created}</div>
            </div>
            <div>
              <label style="display:block;font-weight:500;margin-bottom:.35rem;font-size:13px">Last updated:</label>
              <div style="font-size:13px;color:var(--muted)">{updated}</div>
            </div>
            {form_analytics_html}
          </div>"#,
            display = if is_scheduled { "display:none" } else { "" },
            created = crate::html_escape(created),
            updated = crate::html_escape(updated),
            form_analytics_html = form_analytics_html,
        ),
        _ => String::new(),
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
        format!(r#"<div class="card-boxed-section card-boxed-section-hidden">
          <div class="form-group">
            <label for="template">Template</label>
            <select id="template" name="template">{opts}</select>
          </div>
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

    let delete_btn_inline = match &post.id {
        Some(id) => {
            let (label, path) = if post.post_type == "page" {
                ("Page", format!("/admin/pages/{}/delete", id))
            } else {
                ("Post", format!("/admin/posts/{}/delete", id))
            };
            format!(
                r##"<a href="#" style="font-size:12px;font-weight:600;color:var(--danger)" onclick="event.preventDefault();deletePostConfirm('{path}', '{label_lower}')">Delete {label}</a>"##,
                label = label,
                label_lower = label.to_lowercase(),
                path = crate::html_escape(&path),
            )
        }
        None => String::new(),
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
      <input type="hidden" id="featured_image_cleared" name="featured_image_cleared" value="">
      <div class="featured-image-box{has_image_class}" id="featured-image-box">{fi_box_inner}</div>
      <div class="icon-pill" style="margin-top:.5rem">
        <button type="button" class="icon-btn" title="Set Featured Image" aria-label="Set Featured Image" onclick="openMediaPicker()">
          <img src="/admin/static/icons/image.svg" alt="">
        </button>
        <button type="button" id="fi-remove-btn" class="icon-btn icon-danger" title="Remove featured image" aria-label="Remove featured image" onclick="removeFeaturedImage()" style="{remove_display}">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
      </div>
    </div>"#,
        id_val = crate::html_escape(featured_image_id_val),
        url_val = crate::html_escape(featured_image_url_val),
        has_image_class = has_image_class,
        fi_box_inner = fi_box_inner,
        remove_display = remove_display,
    );

    let protected_checked = if post.post_password_set { "checked" } else { "" };
    let pw_group_display  = if post.post_password_set { "" } else { "display:none" };

    // A post that already has a password shows a compact "Password set ·
    // Change" row instead of an empty field with "leave blank to keep it"
    // placeholder text — clicking Change swaps in the real input so a new
    // password can be typed. Leaving it alone (not saving) intentionally
    // keeps the existing password: no change link click means no new
    // value gets submitted, and the backend already treats a blank
    // post_password as "keep existing" (see save_edit in posts.rs).
    let pw_input_row = format!(
        r#"<div class="form-group" id="post-pw-input-row" style="{display}">
          <label for="post-password" class="sr-only">Password</label>
          <input type="password" id="post-password" name="post_password" autocomplete="new-password" placeholder="{placeholder}" style="font-size:13px">
        </div>"#,
        display = if post.post_password_set { "display:none" } else { "" },
        placeholder = if post.post_password_set { "Enter new password" } else { "Enter password" },
    );
    let pw_set_row = if post.post_password_set {
        r#"<div class="form-group" id="post-pw-set-row" style="display:flex;align-items:center;gap:.4rem">
          <span style="font-size:13px;color:var(--muted)">Password set</span>
          <button type="button" class="icon-btn" title="Change password" aria-label="Change password"
            onclick="document.getElementById('post-pw-set-row').style.display='none';document.getElementById('post-pw-input-row').style.display='';document.getElementById('post-password').focus()">
            <img src="/admin/static/icons/edit.svg" alt="">
          </button>
        </div>"#.to_string()
    } else {
        String::new()
    };

    let password_section = if ctx.user_role.eq_ignore_ascii_case("author") {
        String::new()
    } else {
        format!(
            r#"<div class="form-group" style="margin-bottom:.5rem">
          <label style="display:flex;align-items:center;gap:.5rem;cursor:pointer;font-weight:400">
            <input type="checkbox" id="post-protected-cb" name="post_protected" value="on" {protected_checked}
              onchange="document.getElementById('post-pw-group').style.display=this.checked?'':'none'">
            Password Protect
          </label>
        </div>
        <div id="post-pw-group" style="{pw_group_display}">
          {pw_set_row}
          {pw_input_row}
        </div>"#,
            protected_checked = protected_checked,
            pw_group_display = pw_group_display,
            pw_set_row = pw_set_row,
            pw_input_row = pw_input_row,
        )
    };

    // Comments control: only editors/admins can toggle; authors see nothing here.
    let comments_section = if ctx.user_role.eq_ignore_ascii_case("author") {
        String::new()
    } else {
        let checked = if post.comments_enabled { "checked" } else { "" };
        let label_text = if post.comments_enabled { "Disable Comments" } else { "Allow Comments" };
        let count_badge = if post.comment_count > 0 {
            format!(
                r#" <span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500" title="{n} comment{s}">{n}</span>"#,
                n = post.comment_count,
                s = if post.comment_count == 1 { "" } else { "s" },
            )
        } else {
            String::new()
        };
        format!(
            r#"<div class="form-group" style="margin-bottom:.5rem">
          <label style="display:flex;align-items:center;gap:.5rem;cursor:pointer;font-weight:400">
            <input type="checkbox" id="comments-enabled-cb" name="comments_enabled" value="on" {checked}
              onchange="if(!this.checked && this.defaultChecked){{if(!confirm('Disable comments on this post? Comments already posted are kept, but they\'ll stop showing on the page and no new comments will be allowed.')){{this.checked=true;return;}}}} document.getElementById('comments-enabled-label').textContent=this.checked?'Disable Comments':'Allow Comments';">
            <span id="comments-enabled-label">{label_text}</span>{count_badge}
          </label>
        </div>"#,
            checked = checked,
            label_text = label_text,
            count_badge = count_badge,
        )
    };

    // Comments + Password share one compact box with the Save pill below —
    // both are simple on/off checkboxes for the same audience
    // (editors/admins), so splitting them added visual weight for no gain,
    // and putting Save in the same .card-boxed-section as the last field
    // (rather than its own row below the card) is what lets the section's
    // fill go transparent via .card-boxed-section:has(.icon-pill) in
    // admin.css instead of the pill needing its own recolor.
    let comments_and_password_box = format!(
        "{comments_section}{password_section}",
        comments_section = comments_section,
        password_section = password_section,
    );

    // Author card: shown to editors/admins when viewing an existing post written by someone else.
    let author_card = if !ctx.user_role.eq_ignore_ascii_case("author") && !post.author_name.is_empty() {
        let site_line = if post.site_name.is_empty() {
            String::new()
        } else if post.site_id.is_empty() {
            format!(
                r#"<div class="author-card-site">{}</div>"#,
                crate::html_escape(&post.site_name)
            )
        } else {
            format!(
                r#"<a class="author-card-site" href="/admin/sites/{id}/settings" target="_blank" rel="noopener">{name}</a>"#,
                id = crate::html_escape(&post.site_id),
                name = crate::html_escape(&post.site_name),
            )
        };
        let suspended_badge = if !post.author_is_active {
            r#" <span class="badge" style="background:#fee2e2;color:#991b1b" title="Login blocked until reactivated">Suspended</span>"#
        } else {
            ""
        };
        let name_html = if post.author_id.is_empty() {
            format!(r#"<div class="author-card-name">{}{}</div>"#, crate::html_escape(&post.author_name), suspended_badge)
        } else {
            format!(
                r#"<a class="author-card-name" href="/admin/users/{id}/edit">{name}</a>{suspended_badge}"#,
                id = crate::html_escape(&post.author_id),
                name = crate::html_escape(&post.author_name),
                suspended_badge = suspended_badge,
            )
        };
        format!(
            r#"<div class="form-section author-card">
      <h3>Author</h3>
      {name_html}
      {site_line}
    </div>"#,
            name_html = name_html,
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

    let sources_public_checked = if post.sources_public { " checked" } else { "" };
    let source_rows: String = post.sources.iter().map(|url| {
        format!(
            r#"<div class="source-row" style="display:flex;gap:.5rem;margin-bottom:.5rem;align-items:center">
        <span class="drag-handle" title="Drag to reorder" draggable="true">
          <img src="/admin/static/icons/move.svg" alt="">
        </span>
        <input type="url" class="source-url-input" value="{url}" placeholder="https://example.com/article" style="flex:1">
        <div class="icon-pill" style="align-self:center;margin-top:0">
          <button type="button" class="icon-btn icon-danger" title="Remove" onclick="this.closest('.source-row').remove(); markSourcesDirty();">
            <img src="/admin/static/icons/trash.svg" alt="Remove">
          </button>
        </div>
      </div>"#,
            url = crate::html_escape(url),
        )
    }).collect();
    let sources_json = serde_json::to_string(&post.sources).unwrap_or_else(|_| "[]".to_string());
    // Sources is an editorial/citation feature — fits blog posts, not
    // structural content like an About or Contact page, so it's hidden for
    // pages rather than shown empty.
    let sources_section = if post.post_type == "page" {
        String::new()
    } else {
        format!(
            r#"<div class="card-boxed">
      <h2 class="card-boxed-header">Sources</h2>
      <div class="card-boxed-body">
        <div class="form-group" style="margin-bottom:.75rem">
          <label style="display:flex;align-items:center;gap:.5rem;cursor:pointer;font-weight:400">
            <input type="checkbox" id="sources-public-cb" name="sources_public" value="on"{sources_public_checked}>
            Show sources on the live page
          </label>
          <span id="sources-public-saved" style="display:none;color:var(--muted);font-size:12px;margin-left:1.6rem">Saved</span>
        </div>
        <div id="sources-list">{source_rows}</div>
        <div class="icon-pill">
          <button type="button" class="icon-btn" id="add-source-btn" title="Add Source URL" aria-label="Add Source URL" onclick="addSourceRow()">
            <img src="/admin/static/icons/file-plus.svg" alt="">
          </button>
          {save_sources_btn}
        </div>
        <span id="sources-saved" style="display:none;color:var(--muted);font-size:12px;margin-left:.5rem">Saved</span>
        <input type="hidden" id="sources_json" name="sources_json" value='{sources_json_attr}'>
      </div>
    </div>"#,
            sources_public_checked = sources_public_checked,
            source_rows = source_rows,
            sources_json_attr = crate::html_escape(&sources_json),
            // Only meaningful once the post exists (the save endpoint needs
            // an id) — for a brand-new, not-yet-saved post, sources are
            // covered by the main Save like everything else.
            save_sources_btn = if post.id.is_some() {
                r#"<button type="button" class="icon-btn" id="save-sources-btn" title="Save Sources" aria-label="Save Sources" onclick="saveSources()" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>"#
            } else {
                ""
            },
        )
    };

    let mut content = format!(
        r#"<link rel="stylesheet" href="/admin/static/quill/quill.snow.css">
<style>
  /* Quill's vendored Snow theme hardcodes its own colors (icon strokes/
     fills to #444, dropdown backgrounds to #fff, etc.) with no dark-mode
     awareness of its own — rather than patch the vendored file, every one
     of those is overridden here to the same var(--field-bg)/var(--field-text)
     every other input/select/textarea uses (see admin.css), so the toolbar
     and editor follow theme changes exactly like the rest of the form. */
  .ql-toolbar.ql-snow {{ background: var(--field-bg); color: var(--field-text); border-color: var(--border); }}
  .ql-container.ql-snow {{ border-color: var(--border); }}
  .ql-editor {{ color: var(--field-text); }}
  .ql-snow .ql-stroke {{ stroke: var(--field-text); }}
  .ql-snow .ql-fill, .ql-snow .ql-stroke.ql-fill {{ fill: var(--field-text); }}
  .ql-snow .ql-picker-label {{ color: var(--field-text); }}
  .ql-snow .ql-picker-options {{ background: var(--field-bg); color: var(--field-text); border-color: var(--border); }}
  .ql-snow .ql-picker.ql-expanded .ql-picker-label {{ color: var(--field-text); }}
</style>
<form method="POST" action="{action}" id="post-editor-form">
  <div class="editor-layout">
    <div class="editor-main">
      <div class="card-boxed">
        <h2 class="card-boxed-header">Content</h2>
        <div class="card-boxed-body">
          <div class="card-boxed-section card-boxed-section-hidden">
            <div style="display:grid;grid-template-columns:1fr auto;gap:.75rem;align-items:start">
              <div class="form-group" style="margin:0">
                <label for="title"><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:999px;padding:.15rem .65rem;font-size:.78rem;font-weight:600">Title <span style="color:var(--danger)">*</span></span></label>
                <input type="text" id="title" name="title" value="{title_val}" required class="title-input" maxlength="255"{autofocus}>
                <small id="title-count" style="color:var(--muted)">255/255</small>
              </div>
              <div class="form-group" style="margin:0;min-width:200px;max-width:280px">
                <label for="slug"><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:999px;padding:.15rem .65rem;font-size:.78rem;font-weight:600">Slug</span></label>
                <input type="text" id="slug" name="slug" value="{slug}" maxlength="200"
                  onkeydown="if(event.key===' '){{ event.preventDefault(); var i=this.selectionStart; this.value=this.value.slice(0,i)+'-'+this.value.slice(this.selectionEnd); this.selectionStart=this.selectionEnd=i+1; }}"
                  onblur="this.value=this.value.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');">
                <small id="slug-mode" style="color:var(--muted)">Auto</small>
              </div>
            </div>
          </div>
          <div class="card-boxed-section card-boxed-section-hidden">
            <div class="form-group">
              <label for="excerpt"><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:999px;padding:.15rem .65rem;font-size:.78rem;font-weight:600">Excerpt <span style="color:var(--danger)">*</span></span> <small style="font-weight:400;color:var(--muted)">Used as meta description — required for SEO</small></label>
              <textarea id="excerpt" name="excerpt" rows="3" required maxlength="500" style="resize:none">{excerpt}</textarea>
              <small id="excerpt-count" style="color:var(--muted)">500/500</small>
            </div>
          </div>
          <div class="card-boxed-section card-boxed-section-hidden">
            <div class="form-group">
              <label><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:999px;padding:.15rem .65rem;font-size:.78rem;font-weight:600">Content <span style="color:var(--danger)">*</span></span></label>
              <div id="quill-editor" style="height:620px;background:var(--field-bg);font-size:1rem"></div>
              <input type="hidden" id="content" name="content">
            </div>
          </div>
        </div>
      </div>
      {sources_section}
    </div>
    <div class="editor-sidebar">
      {author_card}
      <div class="form-section">
        <h3 style="display:flex;align-items:center;justify-content:space-between;gap:.5rem">
          <span>{publish_options_label}</span>
          {delete_btn_inline}
        </h3>
        <div class="card-boxed-section card-boxed-section-hidden">
          <div class="form-group">
            <label for="status" id="status-label" style="{status_select_display}">Status</label>
            {status_readonly}
            <select id="status" name="status" style="{status_select_display}">{status_options}</select>
            {status_hint}
          </div>
        </div>
        {template_section}
        <div class="card-boxed-section card-boxed-section-hidden" id="datetime-section" style="{datetime_section_display}">
          <div class="form-group" id="datetime-picker-wrap" style="{datetime_picker_display}">
            {datetime_field}
          </div>
          {post_dates_info}
        </div>
        <input type="hidden" name="post_type" value="{post_type}">
        <div class="card-boxed-section">
          {comments_and_password_box}
          <div style="display:flex;align-items:center;gap:.6rem">
            <div class="icon-pill">
              <button type="submit" class="icon-btn" id="save-btn" title="Save" aria-label="Save" disabled>
                <img src="/admin/static/icons/save.svg" alt="">
              </button>
              <span class="unsaved-indicator" style="display:none;color:var(--success);font-weight:600;font-size:12px;padding:0 .3rem;white-space:nowrap">Save Changes</span>
              {status_actions_pill}
            </div>
          </div>
        </div>
      </div>
      {featured_image_section}
      {inline_media_section}
      {parent_section}
      {categories_section}
    </div>
  </div>
</form>
<script src="/admin/static/quill/quill.min.js"></script>
<script>
(function() {{
  // Enter in any single-line field (title, slug, password, etc.) would
  // otherwise submit the whole form via the browser's default behavior —
  // surprising here since Save is a deliberate icon-button action, and
  // (if title/excerpt are still empty) it just triggers their native
  // required-field validation instead of doing anything useful. Textareas
  // keep normal Enter-for-newline behavior.
  var editorForm = document.getElementById('post-editor-form');
  if (editorForm) {{
    editorForm.addEventListener('keydown', function(e) {{
      if (e.key === 'Enter' && e.target.tagName === 'INPUT') e.preventDefault();
    }});
  }}
  var statusSel = document.getElementById('status');
  var statusLabel = document.getElementById('status-label');
  var statusReadonly = document.getElementById('status-readonly');
  var statusEditBtn = document.getElementById('status-edit-btn');
  if (statusEditBtn && statusReadonly && statusSel) {{
    statusEditBtn.addEventListener('click', function() {{
      statusReadonly.style.display = 'none';
      statusSel.style.display = '';
      if (statusLabel) statusLabel.style.display = '';
      statusSel.focus();
    }});
  }}
  var dateSection = document.getElementById('datetime-section');
  var datePicker = document.getElementById('datetime-picker-wrap');
  var dateInfo = document.getElementById('post-dates-info');
  if (statusSel && dateSection && datePicker) {{
    statusSel.addEventListener('change', function() {{
      var scheduled = statusSel.value === 'scheduled';
      datePicker.style.display = scheduled ? '' : 'none';
      if (dateInfo) dateInfo.style.display = scheduled ? 'none' : '';
      dateSection.style.display = (scheduled || dateInfo) ? '' : 'none';
    }});
  }}
}})();
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

  // Register a custom Quill format for saved-form embeds. A form built in
  // Form Designer gets dropped into post/page content as an inert
  // <ss-form data-slug="..." data-label="..."> placeholder (visible-in-editor
  // only via CSS ::before, see below) — the real <form> HTML is swapped in
  // server-side at render time by expanding this tag, the same way the
  // theme never sees a raw Puck block.
  class FormEmbedBlot extends BlockEmbed {{}}
  FormEmbedBlot.blotName = 'form-embed';
  FormEmbedBlot.tagName  = 'ss-form';
  FormEmbedBlot.create   = function(value) {{
    var node = document.createElement('ss-form');
    node.setAttribute('data-slug', value.slug);
    node.setAttribute('data-label', value.label || value.slug);
    node.setAttribute('contenteditable', 'false');
    return node;
  }};
  FormEmbedBlot.value = function(node) {{
    return {{ slug: node.getAttribute('data-slug'), label: node.getAttribute('data-label') }};
  }};
  Quill.register('formats/form-embed', FormEmbedBlot);

  // Same pattern as FormEmbedBlot, for polls built in Poll Designer.
  class PollEmbedBlot extends BlockEmbed {{}}
  PollEmbedBlot.blotName = 'poll-embed';
  PollEmbedBlot.tagName  = 'ss-poll';
  PollEmbedBlot.create   = function(value) {{
    var node = document.createElement('ss-poll');
    node.setAttribute('data-slug', value.slug);
    node.setAttribute('data-label', value.label || value.slug);
    node.setAttribute('contenteditable', 'false');
    return node;
  }};
  PollEmbedBlot.value = function(node) {{
    return {{ slug: node.getAttribute('data-slug'), label: node.getAttribute('data-label') }};
  }};
  Quill.register('formats/poll-embed', PollEmbedBlot);

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

  // ── Unsaved changes indicator ────────────────────────────────────────
  // formDirty is a real diff against the form's state as loaded, not a
  // sticky "something changed at some point" flag — so checking a box and
  // then unchecking it back to its original state clears the indicator
  // again instead of leaving Save armed for no real change.
  var formDirty = false;
  var initialFormState = null;
  function serializeFormState() {{
    var contentEl = document.getElementById('content');
    if (contentEl) contentEl.value = quill.root.innerHTML;
    var pairs = [];
    new FormData(postForm).forEach(function(value, key) {{ pairs.push(key + '=' + value); }});
    pairs.sort();
    return pairs.join('&');
  }}
  function markDirty() {{
    var dirty = initialFormState !== null && serializeFormState() !== initialFormState;
    formDirty = dirty;
    document.querySelectorAll('.unsaved-indicator').forEach(function(el) {{ el.style.display = dirty ? '' : 'none'; }});
    var saveBtn = document.getElementById('save-btn');
    if (saveBtn) {{
      saveBtn.disabled = !dirty;
      saveBtn.classList.toggle('icon-btn-save-dirty', dirty);
    }}
  }}
  window.markDirty = markDirty;
  var postForm = document.querySelector('form');
  ['input', 'change'].forEach(function(evt) {{
    postForm.addEventListener(evt, function(e) {{
      // The sources-public checkbox and source URL rows are covered by the
      // dedicated Save Sources button (see saveSources/markSourcesDirty) —
      // they shouldn't also flag the main post form dirty.
      if (e.target && (e.target.id === 'sources-public-cb' || e.target.classList.contains('source-url-input'))) return;
      // Just checking "Password Protected" reveals the password field but
      // hasn't actually changed anything worth saving yet — only count it
      // once a password is typed. Unchecking it (removing protection) IS a
      // real change on its own, so that still marks dirty immediately.
      if (e.target && e.target.id === 'post-protected-cb' && e.target.checked) return;
      markDirty();
    }});
  }});
  window.addEventListener('beforeunload', function(e) {{
    if (!formDirty) return;
    e.preventDefault();
    e.returnValue = '';
    // Cancelling this dialog leaves the nav-loading overlay (lib.rs) stuck
    // on, since nothing else clears it. Queuing the clear only lets it run
    // once the (thread-blocking) dialog closes, and only on this page if
    // the user chose Cancel — a confirmed "leave" unloads first instead.
    setTimeout(function() {{
      if (window.cancelNavSpinner) window.cancelNavSpinner();
    }}, 0);
  }});

  // ── AJAX save ────────────────────────────────────────────────────────
  // A native submit reloads the whole editor, including tearing down and
  // re-initializing Quill — this is the single most frequent action on
  // the single most-used admin page, so that reload cost is paid a lot.
  // Reuses the same fetch + follow-redirect technique as
  // deletePostConfirm below: save_edit/save_new are completely untouched
  // server-side, we just choose whether to act on the redirect they
  // already send back.
  postForm.addEventListener('submit', function(e) {{
    e.preventDefault();
    fetch(postForm.action, {{ method: 'POST', body: new FormData(postForm) }}).then(function(r) {{
      if (r.redirected) {{
        var landedPath = new URL(r.url).pathname;
        if (landedPath === location.pathname) {{
          // Common case: editing an existing post lands back on the same
          // page (just gains a ?success=saved query string) — stay put.
          formDirty = false;
          initialFormState = serializeFormState();
          var saveBtn = document.getElementById('save-btn');
          if (saveBtn) {{
            saveBtn.disabled = true;
            saveBtn.classList.remove('icon-btn-save-dirty');
          }}
          document.querySelectorAll('.unsaved-indicator').forEach(function(el) {{
            el.textContent = 'Saved ✓';
            el.style.display = '';
          }});
          setTimeout(function() {{
            document.querySelectorAll('.unsaved-indicator').forEach(function(el) {{
              el.style.display = 'none';
              el.textContent = 'Save Changes';
            }});
          }}, 2000);
        }} else {{
          // New-post-created, or any other server-decided redirect target —
          // the same one-time navigation that already happens today.
          formDirty = false; // intentional navigation away — don't warn
          window.location.href = r.url;
        }}
      }} else {{
        // Validation/DB error: the server rendered the edit page directly
        // with a flash message, no redirect. Reuse that exact rendering —
        // a real native submit — instead of duplicating error display here.
        postForm.submit();
      }}
    }});
  }});

  // Delete button uses fetch instead of a nested <form> — the whole editor
  // is already one big <form>, and nested forms are invalid HTML.
  window.deletePostConfirm = function(path, label) {{
    if (!confirm('Delete this ' + label + '? This cannot be undone.')) return;
    formDirty = false; // intentional navigation away — don't warn
    fetch(path, {{ method: 'POST' }}).then(function(r) {{
      window.location.href = r.url || path;
    }});
  }};

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
  // Baseline snapshot for real dirty-diffing — must be taken after the
  // editor's starting content is loaded above, not at form-init time.
  initialFormState = serializeFormState();
  quill.on('text-change', function(delta, oldDelta, source) {{
    if (source === 'user') markDirty();
  }});

  // ── Sources ───────────────────────────────────────────────────────────
  // Save Sources starts disabled and only enables once a URL is actually
  // typed/removed AND every non-empty source field looks like a real
  // http(s):// URL — matches the post editor's own Save-until-dirty
  // pattern, plus a validity gate so a half-typed/garbage entry can't be
  // saved by accident.
  var sourcesDirty = false;
  function isValidSourceUrl(v) {{
    return /^https?:\/\/[^\s]+\.[^\s]+$/i.test(v.trim());
  }}
  function syncSourcesSaveState() {{
    var btn = document.getElementById('save-sources-btn');
    if (!btn) return;
    var allValid = Array.prototype.every.call(document.querySelectorAll('.source-url-input'), function(el) {{
      var v = el.value.trim();
      var valid = v === '' || isValidSourceUrl(v);
      el.classList.toggle('field-invalid', !valid);
      return valid;
    }});
    btn.disabled = !sourcesDirty || !allValid;
  }}
  window.markSourcesDirty = function() {{
    sourcesDirty = true;
    syncSourcesSaveState();
  }};
  var sourcesListEl = document.getElementById('sources-list');
  if (sourcesListEl) {{
    sourcesListEl.addEventListener('input', function(e) {{
      if (e.target && e.target.classList.contains('source-url-input')) markSourcesDirty();
    }});
  }}

  // Only one not-yet-saved new row is allowed at a time — Add Source URL
  // stays disabled until that row is either saved (via Save Sources) or
  // removed, so unsaved blank rows can't pile up.
  window.addSourceRow = function() {{
    var addBtn = document.getElementById('add-source-btn');
    var list = document.getElementById('sources-list');
    var row = document.createElement('div');
    row.className = 'source-row';
    row.style.cssText = 'display:flex;gap:.5rem;margin-bottom:.5rem;align-items:center';
    row.innerHTML = '<span class="drag-handle" title="Drag to reorder" draggable="true"><img src="/admin/static/icons/move.svg" alt=""></span>'
      + '<input type="url" class="source-url-input" placeholder="https://example.com/article" style="flex:1">'
      + '<div class="icon-pill" style="align-self:center;margin-top:0"><button type="button" class="icon-btn icon-danger" title="Remove" onclick="this.closest(\'.source-row\').remove(); markSourcesDirty(); var b=document.getElementById(\'add-source-btn\'); if (b) b.disabled = false;">'
      + '<img src="/admin/static/icons/trash.svg" alt="Remove"></button></div>';
    list.appendChild(row);
    row.querySelector('input').focus();
    if (addBtn) addBtn.disabled = true;
  }};

  // Drag-to-reorder, same pattern as the form designer's field list and
  // menu editor's item list. Reordering counts as a change to save.
  if (sourcesListEl) {{
    var sourceDragEl = null;
    sourcesListEl.addEventListener('dragstart', function(e) {{
      if (!e.target.classList.contains('drag-handle') && !e.target.closest('.drag-handle')) return;
      sourceDragEl = e.target.closest('.source-row');
      e.dataTransfer.effectAllowed = 'move';
    }});
    sourcesListEl.addEventListener('dragover', function(e) {{
      e.preventDefault();
      if (!sourceDragEl) return;
      var target = e.target.closest('.source-row');
      if (!target || target === sourceDragEl) return;
      var rect = target.getBoundingClientRect();
      var before = (e.clientY - rect.top) < rect.height / 2;
      sourcesListEl.insertBefore(sourceDragEl, before ? target : target.nextSibling);
    }});
    sourcesListEl.addEventListener('dragend', function() {{
      if (sourceDragEl) markSourcesDirty();
      sourceDragEl = null;
    }});
  }}

  // Auto-save the "Show sources on the live page" toggle immediately on
  // change, rather than waiting for the full post form to be saved — the
  // rest of the form may be mid-edit and not ready to submit yet.
  (function() {{
    var cb = document.getElementById('sources-public-cb');
    var postId = {post_id_js};
    if (!cb || !postId) return; // new post — nothing to save until first Save
    cb.addEventListener('change', function() {{
      var saved = document.getElementById('sources-public-saved');
      fetch('/admin/api/posts/' + postId + '/sources-public', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ public: cb.checked }}),
      }}).then(function(r) {{
        if (r.ok && saved) {{
          saved.style.display = '';
          clearTimeout(saved._hideTimer);
          saved._hideTimer = setTimeout(function() {{ saved.style.display = 'none'; }}, 2000);
        }}
      }});
    }});
  }})();

  // Save just the Sources list (+ the public toggle) without submitting the
  // whole post form — the Sources card sits below the main editor, far from
  // the primary Save button, so a dedicated save action here is more
  // discoverable than expecting the reader to scroll back up.
  window.saveSources = function() {{
    var postId = {post_id_js};
    if (!postId) return;
    var cb = document.getElementById('sources-public-cb');
    var saved = document.getElementById('sources-saved');
    var sourceUrls = Array.prototype.slice.call(document.querySelectorAll('.source-url-input'))
      .map(function(el) {{ return el.value.trim(); }})
      .filter(function(v) {{ return v.length > 0; }});
    fetch('/admin/api/posts/' + postId + '/sources', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ sources: sourceUrls, public: cb ? cb.checked : false }}),
    }}).then(function(r) {{
      if (r.ok) {{
        sourcesDirty = false;
        syncSourcesSaveState();
        var addBtn = document.getElementById('add-source-btn');
        if (addBtn) addBtn.disabled = false;
        if (saved) {{
          saved.style.display = '';
          clearTimeout(saved._hideTimer);
          saved._hideTimer = setTimeout(function() {{ saved.style.display = 'none'; }}, 2000);
        }}
      }}
    }});
  }};

  // On submit, copy Quill HTML into the hidden input and validate excerpt
  document.querySelector('form').addEventListener('submit', function(e) {{
    document.getElementById('content').value = quill.root.innerHTML;
    var sourceUrls = Array.prototype.slice.call(document.querySelectorAll('.source-url-input'))
      .map(function(el) {{ return el.value.trim(); }})
      .filter(function(v) {{ return v.length > 0; }});
    var sourcesJsonInput = document.getElementById('sources_json');
    if (sourcesJsonInput) sourcesJsonInput.value = JSON.stringify(sourceUrls);
    var excerpt = document.getElementById('excerpt').value.trim();
    if (!excerpt) {{
      e.preventDefault();
      var el = document.getElementById('excerpt');
      el.focus();
      el.style.borderColor = 'var(--danger)';
      el.setAttribute('placeholder', 'Excerpt is required — describe this post in 1–2 sentences.');
      el.addEventListener('input', function() {{ el.style.borderColor = ''; }}, {{ once: true }});
      return;
    }}
    formDirty = false; // actually saving now — don't warn on the navigation this submit triggers
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
    btn.style.cssText = 'width:auto;padding:0 4px;color:var(--field-text)';
    btn.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>';
    btn.addEventListener('mouseenter', function() {{ btn.style.color = '#06c'; }});
    btn.addEventListener('mouseleave', function() {{ btn.style.color = 'var(--field-text)'; }});
    btn.addEventListener('click', function() {{
      window._quillRange = quill.getSelection(true);
      openMediaPicker('audio');
    }});
    span.appendChild(btn);
    qlToolbar.appendChild(span);
  }})();

  // Add "Insert Form" button to the Quill toolbar — drops a saved form
  // (built in Form Designer) into the content as an embed placeholder.
  (function() {{
    var savedForms = {saved_forms_js};
    var qlToolbar = document.querySelector('.ql-toolbar');
    if (!qlToolbar) return;
    var span = document.createElement('span');
    span.className = 'ql-formats';
    span.style.position = 'relative';
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.title = 'Insert form';
    btn.style.cssText = 'width:auto;padding:0 4px;color:var(--field-text)';
    btn.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="3" width="16" height="18" rx="2"/><line x1="8" y1="8" x2="16" y2="8"/><line x1="8" y1="12" x2="16" y2="12"/><line x1="8" y1="16" x2="12" y2="16"/></svg>';
    btn.addEventListener('mouseenter', function() {{ btn.style.color = '#06c'; }});
    btn.addEventListener('mouseleave', function() {{ btn.style.color = 'var(--field-text)'; }});

    var menu = document.createElement('div');
    menu.style.cssText = 'display:none;position:absolute;top:100%;left:0;z-index:20;min-width:180px;background:var(--field-bg);border:1px solid var(--border);border-radius:var(--radius);box-shadow:var(--shadow);padding:.3rem;margin-top:2px';
    if (savedForms.length === 0) {{
      menu.innerHTML = '<div style="padding:.4rem .6rem;font-size:12px;color:var(--muted)">No forms yet — build one in <a href="/admin/form-designer" target="_blank">Form Designer</a>.</div>';
    }} else {{
      savedForms.forEach(function(f) {{
        var item = document.createElement('button');
        item.type = 'button';
        item.textContent = f[1];
        item.style.cssText = 'display:block;width:100%;text-align:left;padding:.4rem .6rem;font-size:13px;border:none;background:none;border-radius:4px;cursor:pointer;color:var(--field-text)';
        item.addEventListener('mouseenter', function() {{ item.style.background = 'var(--tint)'; }});
        item.addEventListener('mouseleave', function() {{ item.style.background = 'none'; }});
        item.addEventListener('click', function() {{
          var range = window._quillRange || quill.getSelection(true) || {{ index: quill.getLength() }};
          quill.insertEmbed(range.index, 'form-embed', {{ slug: f[0], label: f[1] }}, 'user');
          quill.setSelection(range.index + 1, 0, 'user');
          menu.style.display = 'none';
        }});
        menu.appendChild(item);
      }});
    }}
    span.appendChild(btn);
    span.appendChild(menu);
    qlToolbar.appendChild(span);

    btn.addEventListener('click', function(e) {{
      e.stopPropagation();
      window._quillRange = quill.getSelection(true);
      menu.style.display = menu.style.display === 'none' ? '' : 'none';
    }});
    document.addEventListener('click', function(e) {{
      if (!span.contains(e.target)) menu.style.display = 'none';
    }});
  }})();

  // Add "Insert Poll" button to the Quill toolbar — same pattern as
  // "Insert Form" above, dropping a saved poll (built in Poll Designer)
  // into the content as an embed placeholder.
  (function() {{
    var savedPolls = {saved_polls_js};
    var qlToolbar = document.querySelector('.ql-toolbar');
    if (!qlToolbar) return;
    var span = document.createElement('span');
    span.className = 'ql-formats';
    span.style.position = 'relative';
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.title = 'Insert poll';
    btn.style.cssText = 'width:auto;padding:0 4px;color:var(--field-text)';
    btn.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>';
    btn.addEventListener('mouseenter', function() {{ btn.style.color = '#06c'; }});
    btn.addEventListener('mouseleave', function() {{ btn.style.color = 'var(--field-text)'; }});

    var menu = document.createElement('div');
    menu.style.cssText = 'display:none;position:absolute;top:100%;left:0;z-index:20;min-width:180px;background:var(--field-bg);border:1px solid var(--border);border-radius:var(--radius);box-shadow:var(--shadow);padding:.3rem;margin-top:2px';
    if (savedPolls.length === 0) {{
      menu.innerHTML = '<div style="padding:.4rem .6rem;font-size:12px;color:var(--muted)">No polls yet — build one in <a href="/admin/designer?tab=polls" target="_blank">Poll Designer</a>.</div>';
    }} else {{
      savedPolls.forEach(function(p) {{
        var item = document.createElement('button');
        item.type = 'button';
        item.textContent = p[1];
        item.style.cssText = 'display:block;width:100%;text-align:left;padding:.4rem .6rem;font-size:13px;border:none;background:none;border-radius:4px;cursor:pointer;color:var(--field-text)';
        item.addEventListener('mouseenter', function() {{ item.style.background = 'var(--tint)'; }});
        item.addEventListener('mouseleave', function() {{ item.style.background = 'none'; }});
        item.addEventListener('click', function() {{
          var range = window._quillRange || quill.getSelection(true) || {{ index: quill.getLength() }};
          quill.insertEmbed(range.index, 'poll-embed', {{ slug: p[0], label: p[1] }}, 'user');
          quill.setSelection(range.index + 1, 0, 'user');
          menu.style.display = 'none';
        }});
        menu.appendChild(item);
      }});
    }}
    span.appendChild(btn);
    span.appendChild(menu);
    qlToolbar.appendChild(span);

    btn.addEventListener('click', function(e) {{
      e.stopPropagation();
      window._quillRange = quill.getSelection(true);
      menu.style.display = menu.style.display === 'none' ? '' : 'none';
    }});
    document.addEventListener('click', function(e) {{
      if (!span.contains(e.target)) menu.style.display = 'none';
    }});
  }})();

  // Remaining character counters for title and excerpt
  (function() {{
    function initCount(inputId, countId, max) {{
      var el = document.getElementById(inputId);
      var counter = document.getElementById(countId);
      if (!el || !counter) return;
      function update() {{
        var remaining = max - el.value.length;
        counter.textContent = remaining + '/' + max;
        counter.style.color = remaining <= 20 ? 'var(--danger)' : 'var(--muted)';
      }}
      el.addEventListener('input', update);
      update();
    }}
    initCount('title',   'title-count',   255);
    initCount('excerpt', 'excerpt-count', 500);
  }})();

  // Auto-populate slug from title
  (function() {{
    var titleEl = document.getElementById('title');
    var slugEl  = document.getElementById('slug');
    var modeEl  = document.getElementById('slug-mode');
    if (!titleEl || !slugEl) return;

    function slugify(s) {{
      return s.toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '');
    }}

    function setMode(isLocked) {{
      if (modeEl) modeEl.textContent = isLocked ? 'Custom' : 'Auto';
    }}

    // Lock if slug already has a value on load (existing post with a slug set)
    var locked = slugEl.value.trim() !== '';
    setMode(locked);

    // User manually editing the slug locks it; clearing it unlocks
    slugEl.addEventListener('input', function() {{
      locked = slugEl.value.trim() !== '';
      setMode(locked);
    }});

    titleEl.addEventListener('input', function() {{
      if (!locked) {{
        slugEl.value = slugify(titleEl.value);
      }}
    }});
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
        saved_forms_js = serde_json::to_string(&post.saved_forms).unwrap_or_else(|_| "[]".into()),
        saved_polls_js = serde_json::to_string(&post.saved_polls).unwrap_or_else(|_| "[]".into()),
        post_id_js = serde_json::to_string(&post.id).unwrap_or_else(|_| "null".into()),
        excerpt = crate::html_escape(&post.excerpt),
        status_options = status_options,
        status_hint = status_hint,
        publish_options_label = publish_options_label,
        status_readonly = status_readonly,
        status_actions_pill = status_actions_pill,
        status_select_display = status_select_display,
        datetime_section_display = datetime_section_display,
        datetime_picker_display = datetime_picker_display,
        post_dates_info = post_dates_info,
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
        comments_and_password_box = comments_and_password_box,
        author_card = author_card,
        sources_section = sources_section,
        delete_btn_inline = delete_btn_inline,
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
            view_path: format!("/{}", slug),
        }
    }

    fn make_ctx() -> crate::PageContext {
        crate::PageContext {
            current_site: String::new(),
            user_email: "test@example.com".to_string(),
            user_role: "admin".to_string(),
            is_global_admin: false,
            is_impersonating: false,
            is_on_home_site: true,
            can_manage_users: false,
            can_manage_sites: false,
            can_manage_plugins: false,
            can_manage_settings: false,
            can_manage_content: true,
            can_manage_themes: false,
            can_manage_taxonomies: false,
            can_manage_forms: false,
            can_manage_pages: true,
            can_manage_site_settings: false,
            unread_forms_count: 0,
            app_name: "Synaptic".to_string(),
            logo_url: None,
            default_theme: "system".to_string(),
        }
    }

    #[test]
    fn post_view_link_uses_blog_prefix() {
        let html = render_list(&[make_row("post", "my-post")], "post", 1, 1, None, &make_ctx(), None, 0, 0, "", None, None);
        assert!(html.contains("href=\"/my-post\""), "post view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn page_view_link_uses_root_prefix() {
        let html = render_list(&[make_row("page", "about")], "page", 1, 1, None, &make_ctx(), None, 0, 0, "", None, None);
        assert!(html.contains("href=\"/about\""), "page view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn view_icon_present_in_both_post_and_page_lists() {
        let post_html = render_list(&[make_row("post", "hello")], "post", 1, 1, None, &make_ctx(), None, 0, 0, "", None, None);
        let page_html = render_list(&[make_row("page", "hello")], "page", 1, 1, None, &make_ctx(), None, 0, 0, "", None, None);
        assert!(post_html.contains("eye.svg"), "post list should include eye icon");
        assert!(page_html.contains("eye.svg"), "page list should include eye icon");
    }
}
