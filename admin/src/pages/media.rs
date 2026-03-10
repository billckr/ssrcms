//! Admin media library page.

pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: Option<String>,
}

pub fn render_list(items: &[MediaItem], flash: Option<&str>, ctx: &crate::PageContext) -> String {
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

    let mut content = format!(
        r#"<div class="form-section">
  <form method="POST" action="/admin/media/upload" enctype="multipart/form-data" id="upload-form">
    <div class="drop-zone" id="drop-zone">
      <input type="file" id="media-file" name="file" accept="image/*,application/pdf" required
        style="position:absolute;width:1px;height:1px;opacity:0;overflow:hidden;"
        onchange="updateDropZone(this.files[0])">
      <div class="drop-zone-content">
        <img src="/admin/static/icons/upload.svg" alt="" class="drop-zone-icon">
        <p class="drop-zone-text">Drag &amp; drop a file here</p>
        <p class="drop-zone-sub">or <label for="media-file" class="drop-zone-browse">browse to choose</label></p>
        <p class="drop-zone-filename" id="drop-filename" style="display:none"></p>
      </div>
    </div>
    <div class="form-group" style="margin-top:0.75rem">
      <div style="display:flex;justify-content:space-between;margin-bottom:4px">
        <label style="font-size:12px;font-weight:600;color:var(--muted)">Alt Text <span style="font-weight:400">(optional)</span></label>
        <span id="upload-alt-count" style="font-size:11px;color:var(--muted)">35/35</span>
      </div>
      <input type="text" name="alt_text" maxlength="35" placeholder="Describe this image..."
             oninput="mpickerCount('upload-alt-input','upload-alt-count')" id="upload-alt-input">
    </div>
    <div style="display:flex;align-items:center;gap:.5rem;flex-wrap:wrap;margin-top:.75rem">
      <button type="submit" class="btn btn-primary">Upload</button>
      <button type="button" class="btn btn-secondary" onclick="openMediaPicker('browse')">Browse</button>
      <input id="media-search" type="search" placeholder="Search media&hellip;"
             style="margin-left:auto;padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text);width:100%;max-width:260px"
             oninput="filterMediaGrid(this.value)">
    </div>
  </form>
</div>
<div style="margin-bottom:.5rem"><span id="media-count" style="font-size:13px;color:var(--muted)"></span></div>
<div class="media-grid" id="media-grid">{grid}</div>
<style>
.drop-zone {{
  border: 2px dashed var(--border, #cbd5e1);
  border-radius: 8px;
  padding: 2rem;
  text-align: center;
  cursor: pointer;
  transition: border-color 0.2s, background 0.2s;
  background: var(--surface, #f8fafc);
  position: relative;
}}
.drop-zone.drag-over {{
  border-color: var(--primary, #3b82f6);
  background: #eff6ff;
}}
.drop-zone.has-file {{
  border-color: #22c55e;
  background: #f0fdf4;
}}
.drop-zone-icon {{ width: 2.5rem; height: 2.5rem; opacity: 0.4; margin-bottom: 0.5rem; }}
.drop-zone-text {{ font-size: 1rem; color: var(--text, #1e293b); margin: 0 0 0.25rem; font-weight: 500; }}
.drop-zone-sub {{ font-size: 0.875rem; color: var(--text-muted, #64748b); margin: 0; }}
.drop-zone-browse {{ color: var(--primary, #3b82f6); cursor: pointer; text-decoration: underline; }}
.drop-zone-filename {{ font-size: 0.875rem; color: #16a34a; font-weight: 500; margin: 0.5rem 0 0; }}
</style>
<script>
(function() {{
  var zone = document.getElementById('drop-zone');
  var input = document.getElementById('media-file');

  zone.addEventListener('click', function(e) {{
    if (e.target.tagName !== 'LABEL') input.click();
  }});

  zone.addEventListener('dragover', function(e) {{
    e.preventDefault();
    zone.classList.add('drag-over');
  }});

  zone.addEventListener('dragleave', function() {{
    zone.classList.remove('drag-over');
  }});

  zone.addEventListener('drop', function(e) {{
    e.preventDefault();
    zone.classList.remove('drag-over');
    var file = e.dataTransfer.files[0];
    if (file) {{
      var dt = new DataTransfer();
      dt.items.add(file);
      input.files = dt.files;
      updateDropZone(file);
    }}
  }});
}})();

function updateDropZone(file) {{
  if (!file) return;
  var zone = document.getElementById('drop-zone');
  var label = document.getElementById('drop-filename');
  zone.classList.add('has-file');
  label.textContent = file.name;
  label.style.display = 'block';
}}

function filterMediaGrid(q) {{
  var cards = document.querySelectorAll('#media-grid .media-card');
  var lower = q.toLowerCase().trim();
  var visible = 0;
  cards.forEach(function(card) {{
    var name = (card.querySelector('.media-name') || {{}}).textContent || '';
    var show = !lower || name.toLowerCase().indexOf(lower) !== -1;
    card.style.display = show ? '' : 'none';
    if (show) visible++;
  }});
  var ct = document.getElementById('media-count');
  if (ct) ct.textContent = lower ? visible + ' of ' + cards.length + ' items' : cards.length + ' items';
}}

// Initialise count on load.
document.addEventListener('DOMContentLoaded', function() {{ filterMediaGrid(''); }});
</script>"#,
        grid = grid,
    );
    content.push_str(&crate::media_picker_modal_html());
    crate::admin_page("Media Library", "/admin/media", flash, &content, ctx)
}
