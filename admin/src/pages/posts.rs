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
}

pub struct TermOption {
    pub id: String,
    pub name: String,
}

pub fn render_list(posts: &[PostRow], post_type: &str, page: i64, total_pages: i64, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let title = if post_type == "page" { "Pages" } else { "Posts" };
    let new_label = if post_type == "page" { "New Page" } else { "New Post" };
    let new_href = if post_type == "page" { "/admin/pages/new" } else { "/admin/posts/new" };
    let edit_prefix = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    let base_path = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };

    let bulk_action = if post_type == "page" { "/admin/pages/bulk-delete" } else { "/admin/posts/bulk-delete" };

    let rows = posts.iter().map(|p| {
        let path = if p.post_type == "page" {
            format!("/{}", p.slug)
        } else {
            format!("/blog/{}", p.slug)
        };
        let view_href = if ctx.current_site.is_empty() {
            path
        } else {
            format!("//{}{}", ctx.current_site, path)
        };
        format!(
            r#"<tr>
              <td style="width:2rem;text-align:center">
                <input type="checkbox" class="bulk-cb" value="{id}" aria-label="Select">
              </td>
              <td><a href="{prefix}/{id}/edit">{title}</a></td>
              <td><span class="badge badge-{status}">{status}</span>{protected_badge}</td>
              <td>{author}</td>
              <td>{published}</td>
              <td class="actions">
                <a href="{view_href}" class="icon-btn" title="View" target="_blank" rel="noopener noreferrer">
                  <img src="/admin/static/icons/eye.svg" alt="View">
                </a>
                <a href="{prefix}/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
                <form method="POST" action="{prefix}/{id}/delete" style="display:inline" onsubmit="return confirm('Delete this?')">
                  <button class="icon-btn icon-danger" title="Delete" type="submit">
                    <img src="/admin/static/icons/delete.svg" alt="Delete">
                  </button>
                </form>
              </td>
            </tr>"#,
            prefix = edit_prefix,
            id = crate::html_escape(&p.id),
            title = crate::html_escape(&p.title),
            status = crate::html_escape(&p.status),
            protected_badge = if p.post_password_set { r#" <span class="badge badge-protected" title="Protected">&#x1F512;</span>"# } else { "" },
            author = crate::html_escape(&p.author_name),
            published = p.published_at.as_deref().map(|d| crate::html_escape(d)).unwrap_or_default(),
            view_href = crate::html_escape(&view_href),
        )
    }).collect::<Vec<_>>().join("\n");

    let pagination = if total_pages > 1 {
        let prev = if page > 1 {
            format!(r#"<a href="{}?page={}" class="page-btn">&laquo; Prev</a>"#, base_path, page - 1)
        } else {
            r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
        };
        let next = if page < total_pages {
            format!(r#"<a href="{}?page={}" class="page-btn">Next &raquo;</a>"#, base_path, page + 1)
        } else {
            r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
        };
        // Show up to 7 page number links centred around the current page.
        let start = (page - 3).max(1);
        let end = (page + 3).min(total_pages);
        let mut nums = String::new();
        for p in start..=end {
            if p == page {
                nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{}</span>"#, p));
            } else {
                nums.push_str(&format!(r#"<a href="{}?page={}" class="page-btn">{}</a>"#, base_path, p, p));
            }
        }
        format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
    } else {
        String::new()
    };

    let content = format!(
        r#"<div style="display:flex;align-items:center;gap:.75rem;margin-bottom:1rem">
  <a href="{new_href}" class="btn btn-primary">{new_label}</a>
  <button id="bulk-delete-btn" type="button" class="btn btn-danger" style="display:none"
          onclick="bulkDelete()">Delete Selected (<span id="bulk-count">0</span>)</button>
</div>
<table class="data-table">
  <thead><tr>
    <th style="width:2rem"><input type="checkbox" id="select-all" title="Select all" aria-label="Select all"></th>
    <th>Title</th><th>Status</th><th>Author</th><th>Published</th><th>Actions</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}
<script>
(function() {{
  var selectAll = document.getElementById('select-all');
  var btn       = document.getElementById('bulk-delete-btn');
  var countEl   = document.getElementById('bulk-count');

  function updateBtn() {{
    var checked = document.querySelectorAll('.bulk-cb:checked');
    var n = checked.length;
    countEl.textContent = n;
    btn.style.display = n > 0 ? '' : 'none';
    selectAll.indeterminate = n > 0 && n < document.querySelectorAll('.bulk-cb').length;
    selectAll.checked = n > 0 && n === document.querySelectorAll('.bulk-cb').length;
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
        new_href = new_href,
        new_label = new_label,
        rows = rows,
        pagination = pagination,
        bulk_action = bulk_action,
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

    let status_options = ["draft", "published", "scheduled", "trashed"].iter().map(|s| {
        let selected = if *s == post.status { " selected" } else { "" };
        format!(r#"<option value="{s}"{selected}>{s}</option>"#, s = s, selected = selected)
    }).collect::<Vec<_>>().join("");

    // Default published_at to now if empty (for new posts)
    let published_at = if let Some(val) = &post.published_at {
        val.clone()
    } else if post.id.is_none() {
        // Set to current date/time for new posts
        chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string()
    } else {
        String::new()
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

    let categories_section = if post.post_type != "page" {
        format!(r#"<div class="form-section">
          <h3>Categories</h3>
          <div class="checkbox-group">{cat_options}</div>
        </div>
        <div class="form-section">
          <h3>Tags</h3>
          <div class="checkbox-group">{tag_options}</div>
        </div>"#, cat_options = cat_options, tag_options = tag_options)
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

    let mut content = format!(
        r#"<link rel="stylesheet" href="/admin/static/quill/quill.snow.css">
<form method="POST" action="{action}">
  <div class="editor-layout">
    <div class="editor-main">
      <div class="form-group">
        <label for="title">Title</label>
        <input type="text" id="title" name="title" value="{title_val}" required class="title-input"{autofocus}>
      </div>
      <div class="form-group">
        <label for="slug">Slug</label>
        <input type="text" id="slug" name="slug" value="{slug}"
          onkeydown="if(event.key===' '){{ event.preventDefault(); var i=this.selectionStart; this.value=this.value.slice(0,i)+'-'+this.value.slice(this.selectionEnd); this.selectionStart=this.selectionEnd=i+1; }}"
          onblur="this.value=this.value.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');">
        <small>Lowercase, hyphens only. Spaces auto-convert to hyphens.</small>
      </div>
      <div class="form-group">
        <label>Content</label>
        <div id="quill-editor" style="height:480px;background:#fff;font-size:1rem"></div>
        <input type="hidden" id="content" name="content">
      </div>
      <div class="form-group">
        <label for="excerpt">Excerpt <span style="color:var(--danger);font-weight:700">*</span> <small style="font-weight:400;color:var(--muted)">Used as meta description — required for SEO</small></label>
        <textarea id="excerpt" name="excerpt" rows="3" required>{excerpt}</textarea>
      </div>
    </div>
    <div class="editor-sidebar">
      <div class="form-section">
        <h3>Publish</h3>
        <div class="form-group">
          <label for="status">Status</label>
          <select id="status" name="status">{status_options}</select>
        </div>
        <div class="form-group">
          <label for="published_at">Published At</label>
          <input type="datetime-local" id="published_at" name="published_at" value="{published_at}">
        </div>
        <div class="form-group" style="margin-bottom:.5rem">
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
        </div>
        <input type="hidden" name="post_type" value="{post_type}">
        <button type="submit" class="btn btn-primary">Save</button>
      </div>
      {template_section}
      {categories_section}
      {featured_image_section}
    </div>
  </div>
</form>
<script src="/admin/static/quill/quill.min.js"></script>
<script>
(function() {{
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

  // Load existing content
  var existing = document.getElementById('content').value;
  if (!existing) {{
    existing = {content_js};
  }}
  if (existing) {{
    quill.clipboard.dangerouslyPasteHTML(existing);
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

  // Override Quill's image button to open the media library instead of file picker
  window._quillInstance = quill;
  window._quillRange = null;
  var toolbar = quill.getModule('toolbar');
  toolbar.addHandler('image', function() {{
    window._quillRange = quill.getSelection(true);
    openMediaPicker('inline');
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
        published_at = crate::html_escape(&published_at),
        post_type = crate::html_escape(&post.post_type),
        template_section = template_section,
        categories_section = categories_section,
        featured_image_section = featured_image_section,
        protected_checked = protected_checked,
        pw_group_display = pw_group_display,
        pw_placeholder = pw_placeholder,
        pw_hint = pw_hint,
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
            unread_forms_count: 0,
            app_name: "Synaptic".to_string(),
        }
    }

    #[test]
    fn post_view_link_uses_blog_prefix() {
        let html = render_list(&[make_row("post", "my-post")], "post", 1, 1, None, &make_ctx());
        assert!(html.contains("href=\"/blog/my-post\""), "post view href should be /blog/{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn page_view_link_uses_root_prefix() {
        let html = render_list(&[make_row("page", "about")], "page", 1, 1, None, &make_ctx());
        assert!(html.contains("href=\"/about\""), "page view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn view_icon_present_in_both_post_and_page_lists() {
        let post_html = render_list(&[make_row("post", "hello")], "post", 1, 1, None, &make_ctx());
        let page_html = render_list(&[make_row("page", "hello")], "page", 1, 1, None, &make_ctx());
        assert!(post_html.contains("eye.svg"), "post list should include eye icon");
        assert!(page_html.contains("eye.svg"), "page list should include eye icon");
    }
}
