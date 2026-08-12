//! Admin taxonomy (categories & tags) management page.

pub struct TermItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub post_count: i64,
}

pub fn render(terms: &[TermItem], taxonomy: &str, sort: &str, dir: &str, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let title = if taxonomy == "category" { "Categories" } else { "Tags" };
    let path = if taxonomy == "category" { "/admin/categories" } else { "/admin/tags" };

    let list_html = if terms.is_empty() {
        format!(r#"<p class="muted">No {} found.</p>"#, title.to_lowercase())
    } else {
        render_table(terms, path, sort, dir)
    };

    let content = format!(
        r#"<div class="two-col">
  <div>
    {list_html}
  </div>
  <div>
    <div class="card-boxed">
      <h2 class="card-boxed-header">Add {title_s}</h2>
      <div class="card-boxed-body">
      <form method="POST" action="{path}/new" id="add-term-form">
        <div class="card-boxed-section">
          <div class="form-group">
            <label for="name">Name</label>
            <input type="text" id="name" name="name" required oninput="onNameInput()">
          </div>
        </div>
        <div class="card-boxed-section">
          <div class="form-group">
            <label for="slug">Slug (optional)</label>
            <input type="text" id="slug" name="slug" oninput="slugTouched = true"
              onkeydown="if(event.key===' '){{ event.preventDefault(); var i=this.selectionStart; this.value=this.value.slice(0,i)+'-'+this.value.slice(this.selectionEnd); this.selectionStart=this.selectionEnd=i+1; }}"
              onblur="this.value=this.value.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');">
            <small>Lowercase, hyphens only. Auto-filled from name as you type &mdash; edit it here to override.</small>
          </div>
        </div>
        <input type="hidden" name="taxonomy" value="{taxonomy}">
      </form>
      <div class="icon-pill">
        <button type="submit" form="add-term-form" id="add-term-btn" class="icon-btn" title="Add {title_s}" aria-label="Add {title_s}" disabled>
          <img src="/admin/static/icons/file-plus.svg" alt="">
        </button>
      </div>
      </div>
      <script>
        var slugTouched = false;
        function toSlug(s) {{
          return s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
        }}
        function onNameInput() {{
          var nameEl = document.getElementById('name');
          document.getElementById('add-term-btn').disabled = !nameEl.value.trim();
          if (!slugTouched) {{
            document.getElementById('slug').value = toSlug(nameEl.value);
          }}
        }}
      </script>
    </div>
  </div>
</div>"#,
        list_html = list_html,
        path = path,
        taxonomy = taxonomy,
        title_s = if taxonomy == "category" { "Category" } else { "Tag" },
    );

    crate::admin_page(title, path, flash, &content, ctx)
}

fn render_table(terms: &[TermItem], path: &str, sort: &str, dir: &str) -> String {
    let mut sorted: Vec<&TermItem> = terms.iter().collect();
    match sort {
        "slug"  => sorted.sort_by_key(|t| t.slug.to_lowercase()),
        "posts" => sorted.sort_by_key(|t| t.post_count),
        "name"  => sorted.sort_by_key(|t| t.name.to_lowercase()),
        _ => {}
    }
    let asc = dir != "desc";
    if !sort.is_empty() && !asc {
        sorted.reverse();
    }

    // Sortable column header: link toggles asc/desc for that column.
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == key;
        let next_dir = if is_active && asc { "desc" } else { "asc" };
        let arrow = if is_active { if asc { " \u{25B2}" } else { " \u{25BC}" } } else { "" };
        format!(
            r#"<th><a href="{path}?sort={key}&dir={next_dir}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    let rows = sorted.iter().map(|t| {
        format!(
            r#"<tr>
              <td>{name}</td>
              <td>{slug}</td>
              <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{count}</span></td>
              <td class="actions">
                <div class="icon-pill-actionbuttons">
                  <form method="POST" action="{path}/{id}/delete" style="display:inline" onsubmit="return confirm('Delete?')">
                    <button class="icon-btn icon-danger" title="Delete" type="submit">
                      <img src="/admin/static/icons/trash.svg" alt="Delete">
                    </button>
                  </form>
                </div>
              </td>
            </tr>"#,
            name = crate::html_escape(&t.name),
            slug = crate::html_escape(&t.slug),
            count = t.post_count,
            path = path,
            id = crate::html_escape(&t.id),
        )
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<table class="data-table">
      <thead><tr>{name_th}{slug_th}{posts_th}<th>Actions</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>"#,
        rows     = rows,
        name_th  = sort_th("Name", "name"),
        slug_th  = sort_th("Slug", "slug"),
        posts_th = sort_th("Posts", "posts"),
    )
}
