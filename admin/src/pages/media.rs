//! Admin media library page.

pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: Option<String>,
}

pub struct FolderItem {
    pub id: String,
    pub name: String,
}

pub fn render_list(items: &[MediaItem], folders: &[FolderItem], active_folder: Option<&str>, flash: Option<&str>, ctx: &crate::PageContext) -> String {
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

    // Build folder dropdown options
    let folder_options: String = folders.iter().map(|f| {
        let selected = if active_folder == Some(f.id.as_str()) { " selected" } else { "" };
        format!(r#"<option value="{id}"{selected}>{name}</option>"#,
            id = crate::html_escape(&f.id),
            name = crate::html_escape(&f.name),
            selected = selected,
        )
    }).collect::<Vec<_>>().join("\n");

    // Delete folder button — shown only when a specific folder is active
    let delete_folder_btn = if let Some(active_id) = active_folder {
        format!(
            r#"<form method="POST" action="/admin/media/folders/{id}/delete" style="display:inline"
                  onsubmit="return confirm('Delete folder? Images will become unorganised.')">
              <button class="btn btn-danger" type="submit" style="font-size:12px;padding:.3rem .6rem">Delete Folder</button>
            </form>"#,
            id = crate::html_escape(active_id),
        )
    } else {
        String::new()
    };

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
    <div style="display:flex;align-items:center;gap:.5rem;flex-wrap:wrap;margin-top:.75rem">
      <!-- folder dropdown -->
      <select id="folder-select" name="folder_id" form="upload-form"
              style="padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text)"
              onchange="filterByFolder(this.value)">
        <option value="">All Media</option>
        {folder_options}
      </select>
      <button type="submit" form="upload-form" class="btn btn-primary">Upload</button>
      <!-- new folder inline form -->
      <button type="button" class="btn btn-secondary" onclick="toggleNewFolder()" id="new-folder-btn">Folder +</button>
      <span id="new-folder-form" style="display:none;gap:.35rem;align-items:center">
        <input id="new-folder-input" type="text" maxlength="25" placeholder="Folder name&hellip;"
               oninput="this.value=this.value.replace(/[^a-zA-Z0-9\-]/g,'')"
               style="padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text)">
        <button type="button" class="btn btn-primary" onclick="submitNewFolder()">Create</button>
        <button type="button" class="btn btn-secondary" onclick="toggleNewFolder()">Cancel</button>
      </span>
      {delete_folder_btn}
      <!-- right side -->
      <button type="button" class="btn btn-secondary" onclick="openMediaPicker('browse')" style="margin-left:auto">Browse</button>
      <input id="media-search" type="search" placeholder="Search media&hellip;"
             style="padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text);width:100%;max-width:260px"
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

function filterByFolder(folderId) {{
  var url = '/admin/media';
  if (folderId) url += '?folder_id=' + encodeURIComponent(folderId);
  window.location.href = url;
}}

function toggleNewFolder() {{
  var form = document.getElementById('new-folder-form');
  var btn = document.getElementById('new-folder-btn');
  var visible = form.style.display !== 'none' && form.style.display !== '';
  form.style.display = visible ? 'none' : 'flex';
  if (!visible) document.getElementById('new-folder-input').focus();
}}

function submitNewFolder() {{
  var name = document.getElementById('new-folder-input').value.trim();
  if (!name) return;
  var form = document.createElement('form');
  form.method = 'POST';
  form.action = '/admin/media/folders/new';
  var input = document.createElement('input');
  input.name = 'name';
  input.value = name;
  form.appendChild(input);
  document.body.appendChild(form);
  form.submit();
}}

// Initialise count on load.
document.addEventListener('DOMContentLoaded', function() {{ filterMediaGrid(''); }});
</script>"#,
        folder_options = folder_options,
        delete_folder_btn = delete_folder_btn,
        grid = grid,
    );
    content.push_str(&crate::media_picker_modal_html());
    crate::admin_page("Media Library", "/admin/media", flash, &content, ctx)
}
