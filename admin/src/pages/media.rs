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
            format!(r#"<img src="/uploads/{}" alt="{}" class="media-thumb" loading="lazy">"#,
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

    // Delete folder button — shown only when a specific folder is active.
    // Rendered as a plain button (no form) to avoid nesting inside the upload form.
    // The actual delete forms live outside the upload form in the modal.
    let delete_folder_btn = if active_folder.is_some() {
        r#"<button type="button" class="btn btn-danger"
                 style="font-size:12px;padding:.3rem .6rem"
                 onclick="showDeleteFolderModal()">Delete Folder</button>"#.to_string()
    } else {
        String::new()
    };

    // Delete-folder modal forms (placed outside the upload form to avoid nesting).
    let delete_folder_modal = if let Some(active_id) = active_folder {
        let eid = crate::html_escape(active_id);
        format!(
            r#"<div id="delete-folder-modal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:900;align-items:center;justify-content:center">
  <div style="background:var(--surface);border-radius:8px;padding:1.5rem;max-width:400px;width:90%;box-shadow:0 4px 24px rgba(0,0,0,.25)">
    <h3 style="margin:0 0 .5rem;font-size:1rem;font-weight:600">Delete Folder</h3>
    <p style="margin:0 0 1.25rem;font-size:.875rem;color:var(--muted)">What should happen to the images inside this folder?</p>
    <div style="display:flex;flex-direction:column;gap:.5rem">
      <form method="POST" action="/admin/media/folders/{id}/delete">
        <input type="hidden" name="delete_media" value="false">
        <button type="submit" class="btn btn-secondary" style="width:100%;text-align:left">
          Keep images &mdash; move them to All Media
        </button>
      </form>
      <form method="POST" action="/admin/media/folders/{id}/delete">
        <input type="hidden" name="delete_media" value="true">
        <button type="submit" class="btn btn-danger" style="width:100%;text-align:left">
          Delete folder and all images inside it
        </button>
      </form>
      <button type="button" class="btn btn-secondary" onclick="hideDeleteFolderModal()">Cancel</button>
    </div>
  </div>
</div>"#,
            id = eid,
        )
    } else {
        String::new()
    };

    let mut content = format!(
        r#"<div class="form-section">
  <form method="POST" action="/admin/media/upload" enctype="multipart/form-data" id="upload-form">
    <input type="file" id="media-file" name="file" accept="image/*,application/pdf" required
      style="position:absolute;width:1px;height:1px;opacity:0;overflow:hidden;"
      onchange="showSelectedFile(this)">
    <div style="display:flex;align-items:center;gap:.5rem;flex-wrap:wrap">
      <!-- folder dropdown -->
      <select id="folder-select" name="folder_id" form="upload-form"
              style="padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text)"
              onchange="filterByFolder(this.value)">
        <option value="">All Media</option>
        {folder_options}
      </select>
      <!-- new folder inline form -->
      <button type="button" class="btn btn-primary" onclick="toggleNewFolder()" id="new-folder-btn">Folder +</button>
      <span id="new-folder-form" style="display:none;gap:.35rem;align-items:center">
        <input id="new-folder-input" type="text" minlength="4" maxlength="25"
               placeholder="Folder name&hellip;"
               oninput="this.value=this.value.replace(/[^a-zA-Z0-9\-]/g,'');this.setCustomValidity('')"
               title="Folder name must be 4–25 characters (letters, numbers, hyphens)"
               style="width:16ch;padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text)">
        <button type="button" class="btn btn-primary" onclick="submitNewFolder()">Create</button>
        <button type="button" class="btn btn-secondary" onclick="toggleNewFolder()">Cancel</button>
      </span>
      <button type="button" class="btn btn-primary" onclick="document.getElementById('media-file').click()">Media +</button>
      <button type="submit" form="upload-form" class="btn btn-primary" onclick="clearSelectedFile()">Upload</button>
      <span id="selected-file-info" style="display:none;font-size:.8rem;font-weight:700;color:#111827;background:#e2e8f0;border:1px solid #e2e8f0;border-radius:4px;padding:.2rem .6rem;white-space:nowrap"></span>
      {delete_folder_btn}
      <!-- right side -->
      <button type="button" class="btn btn-primary" onclick="openMediaPicker('browse')" style="margin-left:auto">Browse</button>
      <input id="media-search" type="search" placeholder="Search media&hellip;"
             style="width:22ch;padding:.4rem .75rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--surface);color:var(--text)"
             oninput="filterMediaGrid(this.value)">
    </div>
  </form>
</div>
{delete_folder_modal}
<div style="margin-bottom:.5rem"><span id="media-count" style="font-size:13px;color:var(--muted)"></span></div>
<div class="media-grid" id="media-grid">{grid}</div>

<script>
function showSelectedFile(input) {{
  var span = document.getElementById('selected-file-info');
  if (!span) return;
  if (!input.files || !input.files[0]) {{ span.style.display = 'none'; span.textContent = ''; return; }}
  var f = input.files[0];
  var size = f.size >= 1048576
    ? (f.size / 1048576).toFixed(1) + ' MB'
    : f.size >= 1024
      ? Math.round(f.size / 1024) + ' KB'
      : f.size + ' B';
  span.textContent = f.name + '  ' + size;
  span.style.display = '';
}}
function clearSelectedFile() {{
  var span = document.getElementById('selected-file-info');
  if (span) {{ span.textContent = ''; span.style.display = 'none'; }}
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
  var inp = document.getElementById('new-folder-input');
  var name = inp.value.trim();
  if (!name) {{
    inp.setCustomValidity('Please enter a folder name (4–25 characters, letters, numbers and hyphens only)');
    inp.reportValidity();
    return;
  }}
  inp.setCustomValidity('');
  if (!inp.reportValidity()) return;
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

function showDeleteFolderModal() {{
  document.getElementById('delete-folder-modal').style.display = 'flex';
}}

function hideDeleteFolderModal() {{
  document.getElementById('delete-folder-modal').style.display = 'none';
}}

// Initialise count on load.
document.addEventListener('DOMContentLoaded', function() {{ filterMediaGrid(''); }});
</script>"#,
        folder_options = folder_options,
        delete_folder_btn = delete_folder_btn,
        delete_folder_modal = delete_folder_modal,
        grid = grid,
    );
    content.push_str(&crate::media_picker_modal_html());
    crate::admin_page("Media Library", "/admin/media", flash, &content, ctx)
}
