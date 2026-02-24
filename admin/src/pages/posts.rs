//! Admin post list and editor pages.

pub struct PostRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub slug: String,
    pub post_type: String,
    pub author_name: String,
    pub published_at: Option<String>,
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
}

pub struct TermOption {
    pub id: String,
    pub name: String,
}

pub fn render_list(posts: &[PostRow], post_type: &str, flash: Option<&str>, current_site: &str, is_global_admin: bool, user_email: &str) -> String {
    let title = if post_type == "page" { "Pages" } else { "Posts" };
    let new_label = if post_type == "page" { "New Page" } else { "New Post" };
    let new_href = if post_type == "page" { "/admin/pages/new" } else { "/admin/posts/new" };
    let edit_prefix = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };

    let rows = posts.iter().map(|p| {
        let path = if p.post_type == "page" {
            format!("/{}", p.slug)
        } else {
            format!("/blog/{}", p.slug)
        };
        let view_href = if current_site.is_empty() {
            path
        } else {
            format!("//{}{}", current_site, path)
        };
        format!(
            r#"<tr>
              <td><a href="{prefix}/{id}/edit">{title}</a></td>
              <td><span class="badge badge-{status}">{status}</span></td>
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
            author = crate::html_escape(&p.author_name),
            published = p.published_at.as_deref().map(|d| crate::html_escape(d)).unwrap_or_default(),
            view_href = crate::html_escape(&view_href),
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<p style="margin-bottom:1rem"><a href="{new_href}" class="btn btn-primary">{new_label}</a></p>
<table class="data-table">
  <thead><tr><th>Title</th><th>Status</th><th>Author</th><th>Published</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        new_href = new_href, new_label = new_label, rows = rows,
    );

    let path = if post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    crate::admin_page(title, path, flash, &content, current_site, is_global_admin, user_email)
}

pub fn render_editor(post: &PostEdit, flash: Option<&str>, current_site: &str, is_global_admin: bool, user_email: &str) -> String {
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

    let content = format!(
        r#"<link rel="stylesheet" href="/admin/static/quill/quill.snow.css">
<form method="POST" action="{action}">
  <div class="editor-layout">
    <div class="editor-main">
      <div class="form-group">
        <label for="title">Title</label>
        <input type="text" id="title" name="title" value="{title_val}" required class="title-input">
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
        <label for="excerpt">Excerpt</label>
        <textarea id="excerpt" name="excerpt" rows="3">{excerpt}</textarea>
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
        <input type="hidden" name="post_type" value="{post_type}">
        <button type="submit" class="btn btn-primary">Save</button>
      </div>
      {categories_section}
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

  // On submit, copy Quill HTML into the hidden input
  document.querySelector('form').addEventListener('submit', function() {{
    document.getElementById('content').value = quill.root.innerHTML;
  }});
}})();
</script>"#,
        action = action,
        title_val = crate::html_escape(&post.title),
        slug = crate::html_escape(&post.slug),
        content_js = serde_json::to_string(&post.content).unwrap_or_else(|_| "\"\"".into()),
        excerpt = crate::html_escape(&post.excerpt),
        status_options = status_options,
        published_at = crate::html_escape(&published_at),
        post_type = crate::html_escape(&post.post_type),
        categories_section = categories_section,
    );

    let path = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    crate::admin_page(&title, path, flash, &content, current_site, is_global_admin, user_email)
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

    #[test]
    fn post_view_link_uses_blog_prefix() {
        let html = render_list(&[make_row("post", "my-post")], "post", None, "", false, "");
        assert!(html.contains("href=\"/blog/my-post\""), "post view href should be /blog/{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn page_view_link_uses_root_prefix() {
        let html = render_list(&[make_row("page", "about")], "page", None, "", false, "");
        assert!(html.contains("href=\"/about\""), "page view href should be /{{slug}}");
        assert!(html.contains("target=\"_blank\""), "view link should open in new tab");
    }

    #[test]
    fn view_icon_present_in_both_post_and_page_lists() {
        let post_html = render_list(&[make_row("post", "hello")], "post", None, "", false, "");
        let page_html = render_list(&[make_row("page", "hello")], "page", None, "", false, "");
        assert!(post_html.contains("eye.svg"), "post list should include eye icon");
        assert!(page_html.contains("eye.svg"), "page list should include eye icon");
    }
}
