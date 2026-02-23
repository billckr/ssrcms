//! Admin media library page.

pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: Option<String>,
}

pub fn render_list(items: &[MediaItem], flash: Option<&str>, current_site: &str, is_global_admin: bool) -> String {
    let grid = items.iter().map(|m| {
        let is_image = m.mime_type.starts_with("image/");
        let preview = if is_image {
            format!(r#"<img src="/uploads/{}" alt="{}" class="media-thumb">"#,
                crate::html_escape(&m.path),
                crate::html_escape(m.alt_text.as_deref().unwrap_or("")))
        } else {
            format!(r#"<div class="media-thumb media-file">&#x1F4C4; {}</div>"#,
                crate::html_escape(&m.mime_type))
        };
        format!(
            r#"<div class="media-card">
              {preview}
              <div class="media-name">{filename}</div>
              <form method="POST" action="/admin/media/{id}/delete" onsubmit="return confirm('Delete?')" style="display:inline">
                <button class="icon-btn icon-danger" title="Delete" type="submit">
                  <img src="/admin/static/icons/trash-2.svg" alt="Delete">
                </button>
              </form>
            </div>"#,
            preview = preview,
            filename = crate::html_escape(&m.filename),
            id = crate::html_escape(&m.id),
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<div class="form-section">
  <h2>Upload</h2>
  <form method="POST" action="/admin/media/upload" enctype="multipart/form-data">
    <input type="file" name="file" accept="image/*,application/pdf" required>
    <input type="text" name="alt_text" placeholder="Alt text (optional)">
    <button type="submit" class="btn btn-primary">Upload</button>
  </form>
</div>
<div class="media-grid">{grid}</div>"#,
        grid = grid,
    );

    crate::admin_page("Media Library", "/admin/media", flash, &content, current_site, is_global_admin)
}
