//! Admin taxonomy (categories & tags) management page.

pub struct TermItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub post_count: i64,
}

pub fn render(terms: &[TermItem], taxonomy: &str, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let title = if taxonomy == "category" { "Categories" } else { "Tags" };
    let path = if taxonomy == "category" { "/admin/categories" } else { "/admin/tags" };

    let rows = terms.iter().map(|t| {
        format!(
            r#"<tr>
              <td>{name}</td>
              <td>{slug}</td>
              <td>{count}</td>
              <td class="actions">
                <form method="POST" action="{path}/{id}/delete" style="display:inline" onsubmit="return confirm('Delete?')">
                  <button class="icon-btn icon-danger" title="Delete" type="submit">
                    <img src="/admin/static/icons/trash-2.svg" alt="Delete">
                  </button>
                </form>
              </td>
            </tr>"#,
            name = crate::html_escape(&t.name),
            slug = crate::html_escape(&t.slug),
            count = t.post_count,
            path = path,
            id = crate::html_escape(&t.id),
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<div class="two-col">
  <div>
    <h2>All {title}</h2>
    <table class="data-table">
      <thead><tr><th>Name</th><th>Slug</th><th>Posts</th><th>Actions</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>
  </div>
  <div>
    <h2>Add New {title}</h2>
    <form method="POST" action="{path}/new">
      <div class="form-group">
        <label for="name">Name</label>
        <input type="text" id="name" name="name" required>
      </div>
      <div class="form-group">
        <label for="slug">Slug (optional)</label>
        <input type="text" id="slug" name="slug"
          onkeydown="if(event.key===' '){{ event.preventDefault(); var i=this.selectionStart; this.value=this.value.slice(0,i)+'-'+this.value.slice(this.selectionEnd); this.selectionStart=this.selectionEnd=i+1; }}"
          onblur="this.value=this.value.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'');">
        <small>Lowercase, hyphens only. Auto-generated from name if left blank.</small>
      </div>
      <input type="hidden" name="taxonomy" value="{taxonomy}">
      <button type="submit" class="btn btn-primary">Add {title_s}</button>
    </form>
  </div>
</div>"#,
        title = title,
        rows = rows,
        path = path,
        taxonomy = taxonomy,
        title_s = if taxonomy == "category" { "Category" } else { "Tag" },
    );

    crate::admin_page(title, path, flash, &content, ctx)
}
