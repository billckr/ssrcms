use crate::{admin_page, html_escape, PageContext};

pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: String,
    pub title: String,
    pub caption: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: i64,
    pub folder_id: Option<String>,
    pub uploaded_by_name: String,
    pub uploaded_at: String,
}

pub struct FolderItem {
    pub id: String,
    pub name: String,
}

pub struct TypeCounts {
    pub all: i64,
    pub image: i64,
    pub video: i64,
    pub audio: i64,
    pub document: i64,
}

/// Classify a mime type into a broad category used for the type filter.
fn media_type_key(mime: &str) -> &'static str {
    if mime.starts_with("image/") { "image" }
    else if mime.starts_with("video/") { "video" }
    else if mime.starts_with("audio/") { "audio" }
    else { "document" }
}

fn type_color(key: &str) -> &'static str {
    match key {
        "image"    => "#10b981",
        "video"    => "#f59e0b",
        "audio"    => "#8b5cf6",
        "document" => "#64748b",
        _          => "#64748b",
    }
}

fn format_bytes(b: i64) -> String {
    if b < 1024 { format!("{} B", b) }
    else if b < 1024 * 1024 { format!("{:.1} KB", b as f64 / 1024.0) }
    else { format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)) }
}

pub fn render_list(
    items: &[MediaItem],
    folders: &[FolderItem],
    active_folder: Option<&str>,
    active_type: Option<&str>,
    total: i64,
    page: i64,
    page_size: i64,
    type_counts: TypeCounts,
    flash: Option<&str>,
    picker_mode: bool,
    // True only when opened from a post/page "Set Featured Image" button.
    // When false (opened from the nav media browser), the "Set Image" button
    // and postMessage JS are omitted.
    select_mode: bool,
    ctx: &PageContext,
) -> String {
    // ── Type counts (full library, not just current page) ───────────────────
    let count_image = type_counts.image;
    let count_video = type_counts.video;
    let count_audio = type_counts.audio;
    let count_doc   = type_counts.document;
    let count_all   = type_counts.all;

    // ── Grid items ───────────────────────────────────────────────────────────
    let grid_items: String = items.iter().enumerate().map(|(i, m)| {
        let type_key = media_type_key(&m.mime_type);
        let dot_color = type_color(type_key);
        let is_image = m.mime_type.starts_with("image/");
        let fname = html_escape(&m.filename);
        let alt   = html_escape(&m.alt_text);
        let _fsize = format_bytes(m.file_size);
        let _dims  = match (m.width, m.height) {
            (Some(w), Some(h)) => format!("{}×{}", w, h),
            _ => String::from("—"),
        };

        let thumb = if is_image {
            format!(
                r##"<img src="/uploads/{path}" alt="{alt}" style="width:100%;height:100%;object-fit:cover;display:block;pointer-events:none">"##,
                path = html_escape(&m.path),
                alt  = alt,
            )
        } else {
            let (icon_color, label) = match type_key {
                "video"    => ("#f59e0b", "VIDEO"),
                "audio"    => ("#8b5cf6", "AUDIO"),
                "document" => ("#ef4444", "DOC"),
                _          => ("#64748b", "FILE"),
            };
            format!(
                r##"<div class="mm-item-icon"><svg viewBox="0 0 24 24" fill="none" stroke="{c}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg><span style="color:{c}">{lbl}</span></div>"##,
                c = icon_color, lbl = label,
            )
        };

        // data attributes for JS filtering
        format!(
            r##"<div class="mm-item" data-idx="{idx}" data-type="{type_key}" data-name="{name_lower}" onclick="selectItem(this)">{thumb}<div class="mm-item-bar"><span class="mm-item-type-dot" style="background:{dot}"></span><span class="mm-item-bar-name">{fname}</span></div><div class="mm-item-check"></div></div>"##,
            idx       = i,
            type_key  = type_key,
            name_lower = html_escape(&m.filename.to_lowercase()),
            thumb     = thumb,
            dot       = dot_color,
            fname     = fname,
        )
    }).collect::<Vec<_>>().join("\n");

    // ── List rows ────────────────────────────────────────────────────────────
    let list_rows: String = items.iter().enumerate().map(|(i, m)| {
        let type_key  = media_type_key(&m.mime_type);
        let is_image  = m.mime_type.starts_with("image/");
        let fname     = html_escape(&m.filename);
        let fsize     = format_bytes(m.file_size);
        let dims      = match (m.width, m.height) {
            (Some(w), Some(h)) => format!("{}×{}", w, h),
            _ => String::from("—"),
        };
        let (pill_bg, pill_fg, pill_label) = match type_key {
            "image"    => ("#d1fae5", "#065f46", "IMAGE"),
            "video"    => ("#fef3c7", "#92400e", "VIDEO"),
            "audio"    => ("#ede9fe", "#4c1d95", "AUDIO"),
            "document" => ("#fee2e2", "#991b1b", "DOC"),
            _          => ("#e2e8f0", "#475569", "FILE"),
        };
        let thumb_html = if is_image {
            format!(
                r##"<img class="mm-list-thumb" src="/uploads/{}" alt="">"##,
                html_escape(&m.path)
            )
        } else {
            format!(
                r##"<div class="mm-list-thumb" style="display:flex;align-items:center;justify-content:center;background:var(--tint);font-size:18px">📄</div>"##
            )
        };
        format!(
            r##"<tr data-idx="{idx}" data-type="{type_key}" data-name="{name_lower}" onclick="selectItem(this)"><td><input type="checkbox" onclick="event.stopPropagation();selectItem(this.closest('tr'))"></td><td>{thumb}</td><td><strong style="font-size:13px">{fname}</strong></td><td><span class="mm-list-type-pill" style="background:{pbg};color:{pfg}">{plbl}</span></td><td style="color:var(--muted)">{fsize}</td><td style="color:var(--muted)">{dims}</td><td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem" onclick="event.stopPropagation();selectItem(this.closest('tr'))">Edit</button></td></tr>"##,
            idx        = i,
            type_key   = type_key,
            name_lower = html_escape(&m.filename.to_lowercase()),
            thumb      = thumb_html,
            fname      = fname,
            pbg        = pill_bg, pfg = pill_fg, plbl = pill_label,
            fsize      = fsize,
            dims       = dims,
        )
    }).collect::<Vec<_>>().join("\n");

    // ── Detail panel data (JSON for JS) ─────────────────────────────────────
    let items_json: String = {
        let parts: Vec<String> = items.iter().map(|m| {
            let type_key = media_type_key(&m.mime_type);
            let is_image = m.mime_type.starts_with("image/");
            let dims = match (m.width, m.height) {
                (Some(w), Some(h)) => format!("{}×{}", w, h),
                _ => String::from("—"),
            };
            format!(
                r##"{{"id":"{id}","filename":"{fn}","type":"{ty}","isImage":{img},"path":"/uploads/{path}","alt":"{alt}","title":"{title}","caption":"{caption}","size":"{size}","dims":"{dims}","uploader":"{uploader}","uploaded_at":"{uploaded_at}"}}"##,
                id          = html_escape(&m.id),
                fn          = html_escape(&m.filename),
                ty          = type_key,
                img         = is_image,
                path        = html_escape(&m.path),
                alt         = html_escape(&m.alt_text),
                title       = html_escape(&m.title),
                caption     = html_escape(&m.caption),
                size        = format_bytes(m.file_size),
                dims        = dims,
                uploader    = html_escape(&m.uploaded_by_name),
                uploaded_at = html_escape(&m.uploaded_at),
            )
        }).collect();
        format!("[{}]", parts.join(","))
    };

    // ── Folders JSON (for bulk "Move to" in JS) ──────────────────────────────
    let folders_json: String = {
        let parts: Vec<String> = folders.iter().map(|f| {
            format!(r##"{{"id":"{id}","name":"{name}"}}"##,
                id   = html_escape(&f.id),
                name = html_escape(&f.name),
            )
        }).collect();
        format!("[{}]", parts.join(","))
    };

    // ── Folder dropdown options ──────────────────────────────────────────────
    let folder_items_html: String = {
        let all_selected = if active_folder.is_none() { " selected" } else { "" };
        let mut opts = format!(
            r##"<option value=""{sel}>All Media</option>"##,
            sel = all_selected,
        );
        let mut sorted: Vec<&FolderItem> = folders.iter().collect();
        sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for f in &sorted {
            let sel = if active_folder == Some(f.id.as_str()) { " selected" } else { "" };
            opts.push_str(&format!(
                r##"<option value="{id}"{sel}>{name}</option>"##,
                id   = html_escape(&f.id),
                name = html_escape(&f.name),
                sel  = sel,
            ));
        }
        opts
    };

    // ── URL param helpers ────────────────────────────────────────────────────
    // frame_qs preserves the current mode across internal navigation.
    let frame_qs = if select_mode { "&picker=1" } else if picker_mode { "&browser=1" } else { "" };
    let frame_base = if select_mode { "/admin/media?picker=1" } else if picker_mode { "/admin/media?browser=1" } else { "/admin/media" };
    let folder_qs      = active_folder.map(|f| format!("&folder_id={}", f)).unwrap_or_default();
    let type_all_url   = if let Some(f) = active_folder {
        format!("/admin/media?folder_id={}{}", f, frame_qs)
    } else {
        frame_base.to_string()
    };
    let type_image_url = format!("/admin/media?type=image{}{}",    folder_qs, frame_qs);
    let type_video_url = format!("/admin/media?type=video{}{}",    folder_qs, frame_qs);
    let type_audio_url = format!("/admin/media?type=audio{}{}",    folder_qs, frame_qs);
    let type_doc_url   = format!("/admin/media?type=document{}{}", folder_qs, frame_qs);
    let type_all_active   = if active_type.is_none()             { "active" } else { "" };
    let type_image_active = if active_type == Some("image")    { "active" } else { "" };
    let type_video_active = if active_type == Some("video")    { "active" } else { "" };
    let type_audio_active = if active_type == Some("audio")    { "active" } else { "" };
    let type_doc_active   = if active_type == Some("document") { "active" } else { "" };
    let folder_onchange = format!(
        "if(this.value)window.location='/admin/media?folder_id='+this.value+'{}';else window.location='{}'",
        if let Some(t) = active_type { format!("&type={}{}", t, frame_qs) } else { frame_qs.to_string() },
        if let Some(t) = active_type {
            format!("/admin/media?type={}{}", t, frame_qs)
        } else {
            frame_base.to_string()
        },
    );
    let mut pager_parts: Vec<String> = Vec::new();
    if let Some(t) = active_type   { pager_parts.push(format!("type={}", t)); }
    if let Some(f) = active_folder { pager_parts.push(format!("folder_id={}", f)); }
    if select_mode      { pager_parts.push("picker=1".to_string()); }
    else if picker_mode { pager_parts.push("browser=1".to_string()); }
    let pager_suffix = if pager_parts.is_empty() { String::new() } else { format!("&{}", pager_parts.join("&")) };

    // ── Pagination ───────────────────────────────────────────────────────────
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as i64;

    let pagination_html = if total_pages <= 1 {
        String::new()
    } else {
        let mut p = String::new();
        // Prev
        if page > 1 {
            p.push_str(&format!(
                r##"<a href="/admin/media?page={}{}" class="page-btn">&lsaquo; Prev</a>"##,
                page - 1, pager_suffix
            ));
        } else {
            p.push_str(r##"<span class="page-btn page-btn-disabled">&lsaquo; Prev</span>"##);
        }
        // Page numbers (show at most 7 around current)
        let start = (page - 3).max(1);
        let end   = (page + 3).min(total_pages);
        if start > 1 {
            p.push_str(&format!(r##"<a href="/admin/media?page=1{}" class="page-btn">1</a>"##, pager_suffix));
            if start > 2 { p.push_str(r##"<span class="page-btn" style="pointer-events:none;color:var(--muted)">…</span>"##); }
        }
        for n in start..=end {
            if n == page {
                p.push_str(&format!(r##"<span class="page-btn page-btn-active">{}</span>"##, n));
            } else {
                p.push_str(&format!(
                    r##"<a href="/admin/media?page={n}{ps}" class="page-btn">{n}</a>"##,
                    n = n, ps = pager_suffix
                ));
            }
        }
        if end < total_pages {
            if end < total_pages - 1 { p.push_str(r##"<span class="page-btn" style="pointer-events:none;color:var(--muted)">…</span>"##); }
            p.push_str(&format!(r##"<a href="/admin/media?page={tp}{ps}" class="page-btn">{tp}</a>"##, tp = total_pages, ps = pager_suffix));
        }
        // Next
        if page < total_pages {
            p.push_str(&format!(
                r##"<a href="/admin/media?page={}{}" class="page-btn">Next &rsaquo;</a>"##,
                page + 1, pager_suffix
            ));
        } else {
            p.push_str(r##"<span class="page-btn page-btn-disabled">Next &rsaquo;</span>"##);
        }
        p
    };

    let showing_from = if total == 0 { 0 } else { (page - 1) * page_size + 1 };
    let showing_to   = (page * page_size).min(total);
    let footer_info  = format!("Showing {}–{} of {} files", showing_from, showing_to, total);

    // ── Page title ───────────────────────────────────────────────────────────
    let page_title = "Media Library".to_string();

    // ── Upload form: redirect URL + optional folder_id hidden input ──────────
    let redirect_url = if let Some(fid) = active_folder {
        format!("/admin/media?folder_id={}{}", fid, frame_qs)
    } else {
        frame_base.to_string()
    };
    // ── Detail actions HTML (differs in picker/select mode) ───────────────────
    let detail_actions_html = if select_mode {
        r##"    <div class="mm-detail-actions" id="mmDetailActions" style="display:none">
      <div class="icon-pill" style="margin-top:.3rem;justify-content:center">
        <button id="mmPickerSetBtn" class="icon-btn" title="Set Image" aria-label="Set Image" onclick="pickerSelectImage()"><img src="/admin/static/icons/check.svg" alt=""></button>
        <button id="mmSaveBtn" class="icon-btn" title="Save changes" aria-label="Save changes" onclick="saveDetail()"><img src="/admin/static/icons/save.svg" alt=""></button>
      </div>
    </div>"##.to_string()
    } else {
        r##"    <div class="mm-detail-actions" id="mmDetailActions" style="display:none">
      <div class="icon-pill" style="margin-top:.3rem;justify-content:center">
        <button id="mmSaveBtn" class="icon-btn" title="Save changes" aria-label="Save changes" onclick="saveDetail()"><img src="/admin/static/icons/save.svg" alt=""></button>
      </div>
    </div>"##.to_string()
    };
    // ── JS bridge for picker/select mode ─────────────────────────────────────
    let picker_js = if select_mode {
        r#"
  /* ── Picker bridge — sends selected image back to the parent page ─── */
  window.pickerSelectImage = function() {
    if (!activeDetailId) return;
    var alt   = (document.getElementById('mmDetailAlt')   || {}).value || '';
    var title = (document.getElementById('mmDetailTitle') || {}).value || '';
    var item  = activeDetailIdx !== null ? ITEMS[activeDetailIdx] : null;
    window.parent.postMessage({
      type:  'featuredImageSelected',
      id:    activeDetailId,
      path:  item ? item.path : '',
      alt:   alt.trim(),
      title: title.trim()
    }, '*');
  };"#.to_string()
    } else {
        String::new()
    };
    let folder_hidden = if let Some(fid) = active_folder {
        format!(r##"<input type="hidden" name="folder_id" value="{}">"##, html_escape(fid))
    } else {
        String::new()
    };

    let flash_html = match flash {
        Some(msg) => format!(r##"<div class="flash success">{}</div>"##, html_escape(msg)),
        None => String::new(),
    };

    // Collapsed search-icon-in-pill, expands on click — same pattern used
    // for search on /admin/users etc. filterItems() (below) already reads
    // from #mmSearch, so the input id here has to stay "mmSearch".
    let search_pill = crate::pill_search_toggle("mmSearch", "Search files\u{2026}", "");
    let pill_search_init = crate::pill_search_init_script();

    // When rendered inside the picker/browser iframe, put the modal's own
    // title and close button in the toolbar row instead of a separate
    // header bar above it (which would live in the parent document, one
    // iframe boundary away) — this keeps the whole modal chrome to one
    // row without crossing into the parent's DOM. The close button posts a
    // message up to whichever of openMediaPicker/openMediaBrowser's modals
    // is currently showing this iframe; see the matching listeners in
    // admin/src/lib.rs.
    let toolbar_title = if picker_mode {
        r#"<span class="mpicker-title">Media Library</span>"#.to_string()
    } else {
        String::new()
    };
    let toolbar_close = if picker_mode {
        r##"<div class="icon-pill" style="margin-top:0">
        <button type="button" class="icon-btn" title="Close" aria-label="Close" onclick="window.parent.postMessage({type:'closeMediaFrame'}, '*')"><img src="/admin/static/icons/x.svg" alt=""></button>
      </div>"##.to_string()
    } else {
        String::new()
    };

    // ── Delete-folder button (only when a folder is active) ──────────────────
    let delete_folder_btn_html = if active_folder.is_some() {
        r##"<button class="btn btn-danger mm-new-folder-btn" style="margin-top:.3rem" disabled title="Requires JavaScript"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>Delete folder</button>"##.to_string()
    } else {
        String::new()
    };

    let content = format!(r##"
{flash}
<style>
/* ── Force sidebar-collapsed layout on this page only ────────────────── */
.admin-sidebar {{
  transform: translateX(-100%);
  transition: transform .25s ease;
  box-shadow: none;
}}
body.sidebar-open .admin-sidebar {{
  transform: translateX(0);
  box-shadow: 4px 0 24px rgba(0,0,0,.25);
}}
.admin-main {{
  margin-left: 0 !important;
  width: 100%;
}}
.hamburger {{
  display: flex !important;
}}
/* ── Media Manager 2 — page-scoped styles ────────────────────────────── */
.mm-layout {{
  display: grid;
  grid-template-columns: 220px 1fr;
  grid-template-rows: auto 1fr;
  gap: 0;
  height: calc(100vh - 120px);
  min-height: 500px;
  /* Deliberately no background here — .mm-content-area/.mm-grid-wrap/.mm-main
     are all transparent (confirmed live), so whatever shows through is
     .mm-layout's own background. This component renders inside two
     different wrappers with two different body backgrounds: the standalone
     /admin/media page (admin_page(), body = var(--bg)) and the picker/
     browser iframe (picker_page(), body explicitly set to var(--surface)).
     Hardcoding either token here is only ever right in one of those two
     contexts — staying transparent means it always matches whichever body
     is actually underneath it, in both. */
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  overflow: hidden;
  position: relative;
}}

/* ── Toolbar ──────────────────────────────────────────────────────────── */
.mm-toolbar {{
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: .6rem;
  padding: .65rem 1rem;
  background: var(--surface);
  flex-wrap: wrap;
}}
.mm-toolbar-left  {{ display: flex; align-items: center; gap: .5rem; flex: 1; min-width: 0; flex-wrap: wrap; }}
.mm-toolbar-right {{ display: flex; align-items: center; gap: .5rem; flex-shrink: 0; }}

/* Generic "this toggle is on" state — grid/list view + the bulk-select
   toggle. Icon turns the same blue .icon-btn already uses on hover
   (reusing that exact filter value rather than inventing a new blue), so
   "active" reads as a persistent version of the hover color, not a new
   background/pill treatment. */
.icon-btn-active img {{ filter: invert(24%) sepia(94%) saturate(300%) hue-rotate(204deg) brightness(145%) contrast(193%); }}
:root[data-theme="dark"] .icon-btn-active img {{ filter: invert(36%) sepia(75%) saturate(252%) hue-rotate(197deg) brightness(133%) contrast(148%); }}

/* Brief post-save confirmation on the detail panel's Save button — same
   green-on-success convention as #save-btn.icon-btn-save-dirty on the post
   editor; needs the #mmSaveBtn id for the same specificity reason (out-wins
   the dark-mode img[src^=...] invert(1) rule). Error reuses the existing
   always-on danger red. */
#mmSaveBtn.icon-btn-active-green img {{ filter: var(--success-filter); }}

/* .icon-pill's own background is var(--tint) — invisible against
   .mm-toolbar/.mm-sidebar, which use that same var(--tint) as their base
   panel color. Give pills sitting on top of them the contrasting surface
   color instead. Applies equally to the WASM-rendered New Folder/Delete
   Folder pills in the sidebar, since this is a plain selector match against
   whatever ends up in the DOM, not tied to where the markup came from. */
.mm-toolbar .icon-pill {{ background: var(--tint); }}
.mm-sidebar .icon-pill {{ background: var(--surface); }}

/* ── Left panel ───────────────────────────────────────────────────────── */
.mm-sidebar {{
  background: var(--surface);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}}
.mm-panel-section {{ padding: .65rem .85rem .4rem; }}
.mm-panel-section + .mm-panel-section {{ border-top: 1px solid var(--border); }}
.mm-panel-label {{
  font-size: 13px; font-weight: 600;
  letter-spacing: .01em; color: var(--text); margin-bottom: .45rem;
  padding-left: .6rem;
}}

.mm-type-list {{ list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }}
.mm-type-item a {{
  display: flex; align-items: center; gap: .55rem;
  padding: .38rem .55rem; border-radius: 5px; font-size: 13px;
  color: var(--text); text-decoration: none; transition: background .12s;
  cursor: pointer;
}}
.mm-type-item a:hover {{ background: var(--tint); }}
.mm-type-item a.active {{ background: #ede9fe; color: var(--primary); font-weight: 600; }}
.mm-type-dot {{ width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }}
.mm-type-count {{
  margin-left: auto;
  display: inline-block;
  background: #f3f4f6;
  color: #374151;
  border-radius: 4px;
  padding: .15rem .5rem;
  font-size: .78rem;
  font-weight: 500;
}}
.mm-type-item a.active .mm-type-count {{ background: #ddd6fe; color: var(--primary); }}

.mm-folder-list {{ list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }}
.mm-folder-item a {{
  display: flex; align-items: center; gap: .5rem;
  padding: .35rem .5rem; border-radius: 5px; font-size: 13px;
  color: var(--text); text-decoration: none; transition: background .12s;
}}
.mm-folder-item a:hover {{ background: var(--tint); }}
.mm-folder-item a.active {{ background: #ede9fe; color: var(--primary); }}
.mm-folder-item svg {{ flex-shrink: 0; color: var(--muted); }}
.mm-folder-item a.active svg {{ color: var(--primary); }}
.mm-folder-select {{
  margin: .4rem .85rem .3rem;
  width: calc(100% - 1.7rem);
  padding: .35rem .5rem;
  font-size: 13px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  color: var(--text);
  cursor: pointer;
}}
.mm-new-folder-btn {{ margin: .5rem .85rem .65rem; font-size: 12px; width: calc(100% - 1.7rem); justify-content: flex-start; gap: .4rem; }}

/* ── Main ─────────────────────────────────────────────────────────────── */
.mm-main {{ display: flex; flex-direction: column; overflow: hidden; min-width: 0; position: relative; }}
/* Same fading accent line as .admin-header::after, but scoped to just the
   main content column's width (not the sidebar's) — .mm-main starts right
   of the sidebar, so anchoring this to its own top edge keeps the line
   centered over the content area only. */
.mm-main::before {{
  content: '';
  position: absolute;
  left: 0; right: 0; top: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--primary) 50%, transparent);
  /* .mm-content-area is also position:relative (z-index:auto) and comes
     later in DOM order, so without this it paints over this 1px line
     instead of the line showing on top. */
  z-index: 1;
}}

/* Content area: sits below the drop zone, holds grid + detail panel */
.mm-content-area {{
  flex: 1;
  position: relative;
  display: flex;
  overflow: hidden;
  min-height: 0;
}}

/* Drag-over feedback for the upload icon-btn — toggled by the same JS that
   used to target .mm-dropzone, now just an .icon-btn like its siblings. */
.icon-btn.drag-over {{ background: var(--tint); }}

.mm-grid-wrap {{ flex: 1; overflow-y: auto; padding: .85rem; min-width: 0; background: var(--tint); }}

.mm-grid {{
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  gap: .65rem;
}}
.mm-item {{
  position: relative; border-radius: var(--radius);
  border: 2px solid transparent; overflow: hidden;
  cursor: pointer; background: var(--tint);
  transition: border-color .15s, box-shadow .15s; aspect-ratio: 1;
}}
.mm-item:hover {{ border-color: #c7d2fe; box-shadow: 0 2px 8px rgba(79,70,229,.12); }}
.mm-item.selected {{ border-color: var(--primary); box-shadow: 0 0 0 3px rgba(79,70,229,.18); }}
.mm-item.hidden {{ display: none !important; }}

.mm-item img {{ width: 100%; height: 100%; object-fit: cover; display: block; pointer-events: none; }}
.mm-item-icon {{
  width: 100%; height: 100%;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: .4rem;
  color: var(--muted);
}}
.mm-item-icon svg {{ width: 32px; height: 32px; }}
.mm-item-icon span {{ font-size: 11px; font-weight: 700; letter-spacing: .04em; text-transform: uppercase; }}
.mm-item-bar {{
  position: absolute; bottom: 0; left: 0; right: 0;
  padding: .3rem .45rem; background: rgba(15,23,42,.55);
  backdrop-filter: blur(4px);
  display: flex; align-items: center; gap: .3rem;
}}
.mm-item-bar-name {{ font-size: 10px; color: #fff; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: 1; line-height: 1.3; }}
.mm-item-type-dot {{ width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }}

.mm-item-check {{
  position: absolute; top: .4rem; left: .4rem;
  width: 18px; height: 18px; border-radius: 4px;
  border: 2px solid rgba(255,255,255,.8); background: rgba(0,0,0,.25);
  display: flex; align-items: center; justify-content: center;
  opacity: 0; transition: opacity .15s;
}}
.mm-bulk-mode .mm-item-check,
.mm-item:hover .mm-item-check {{ opacity: 1; }}
.mm-item.selected .mm-item-check {{ background: var(--primary); border-color: var(--primary); opacity: 1; }}
.mm-item.selected .mm-item-check::after {{
  content: ''; display: block; width: 5px; height: 9px;
  border: 2px solid #fff; border-top: none; border-left: none;
  transform: rotate(45deg) translate(-1px,-1px);
}}

/* List view */
.mm-list {{ display: none; }}
.mm-view-list .mm-grid {{ display: none; }}
.mm-view-list .mm-list {{ display: table; width: 100%; border-collapse: collapse; }}
.mm-list thead th {{
  text-align: left; padding: .5rem .75rem;
  font-size: 11px; font-weight: 700; text-transform: uppercase;
  letter-spacing: .05em; color: var(--muted);
  border-bottom: 1px solid var(--border); background: var(--tint); white-space: nowrap;
}}
.mm-list td {{ padding: .5rem .75rem; font-size: 13px; border-bottom: 1px solid var(--border); vertical-align: middle; }}
.mm-list tr:hover td {{ background: var(--tint); }}
.mm-list tr.selected td {{ background: #ede9fe; }}
.mm-list tr.hidden {{ display: none !important; }}
/* Hide checkbox column in list view until Select mode is active */
#mmLayout:not(.mm-bulk-mode) .mm-list thead th:first-child,
#mmLayout:not(.mm-bulk-mode) .mm-list tbody td:first-child {{ display: none; }}
.mm-list .mm-list-thumb {{ width: 40px; height: 40px; border-radius: 4px; object-fit: cover; display: block; background: var(--tint); }}
.mm-list-type-pill {{ display: inline-block; padding: .15rem .45rem; border-radius: 99px; font-size: 11px; font-weight: 600; }}

/* Empty state */
.mm-empty {{ display: none; padding: 3rem; text-align: center; color: var(--muted); font-size: 13px; }}
.mm-empty.visible {{ display: block; }}

/* ── Detail panel — positioned inside .mm-content-area ───────────────── */
.mm-detail-panel {{
  position: absolute; top: 0; right: 0; bottom: 0; width: 280px;
  background: var(--surface); border-left: 1px solid var(--border);
  display: flex; flex-direction: column;
  transform: translateX(100%); transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 10; overflow: hidden;
}}
.mm-detail-panel.open {{ transform: translateX(0); }}

.mm-detail-body {{ flex: 1; overflow-y: auto; padding: .9rem; }}
.mm-detail-preview {{
  width: 100%; aspect-ratio: 4/3; object-fit: contain;
  border-radius: var(--radius); background: var(--tint); display: block; margin-bottom: .85rem;
}}
.mm-detail-preview-icon {{
  width: 100%; aspect-ratio: 4/3; display: flex;
  align-items: center; justify-content: center;
  background: var(--tint); border-radius: var(--radius); margin-bottom: .85rem; color: var(--muted);
}}
.mm-detail-filename {{ font-weight: 600; font-size: 13px; color: var(--text); word-break: break-all; margin-bottom: .5rem; }}
.mm-detail-stats {{ display: grid; grid-template-columns: auto 1fr; gap: .3rem .75rem; font-size: 12px; }}
.mm-detail-stat-label {{ color: var(--muted); }}
.mm-detail-stat-value {{ color: var(--text); font-weight: 500; }}

.mm-detail-field {{ margin-bottom: .65rem; }}
.mm-detail-field label {{
  display: block; font-size: 12px; font-weight: 600; color: var(--muted);
  margin-bottom: .3rem; text-transform: uppercase; letter-spacing: .04em;
}}
.mm-detail-field input[type=text],
.mm-detail-field textarea {{
  width: 100%; padding: .4rem .6rem; border: 1px solid var(--border);
  border-radius: var(--radius); font-size: 13px; font-family: inherit;
  background: var(--field-bg); color: var(--field-text);
}}
.mm-detail-field textarea {{ resize: vertical; min-height: 64px; }}
.mm-detail-field input:focus,
.mm-detail-field textarea:focus {{
  outline: none; border-color: var(--primary);
  box-shadow: 0 0 0 2px rgba(79,70,229,.12);
}}

.mm-detail-url {{
  display: flex; align-items: center; gap: .4rem;
  background: var(--tint); border: 1px solid var(--border);
  border-radius: var(--radius); padding: .35rem .6rem;
  font-size: 12px; color: var(--muted); word-break: break-all; margin-bottom: .65rem;
}}
.mm-detail-url span {{ flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}

.mm-detail-actions {{
  display: flex; flex-direction: column; gap: .5rem;
  padding: .85rem; border-top: 1px solid var(--border); flex-shrink: 0;
}}

/* ── Bulk action bar ──────────────────────────────────────────────────── */
.mm-bulk-bar {{
  position: absolute; bottom: 1rem; left: 50%;
  transform: translateX(-50%) translateY(80px);
  background: var(--surface); color: var(--text);
  border: 1px solid var(--border);
  border-radius: 10px; padding: .6rem 1rem;
  display: flex; align-items: center; gap: .75rem;
  box-shadow: 0 4px 16px rgba(0,0,0,.10);
  transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 20; white-space: nowrap; font-size: 13px;
}}
.mm-bulk-bar.visible {{ transform: translateX(-50%) translateY(0); }}
.mm-bulk-bar-count {{ font-weight: 700; color: var(--primary); }}
.mm-bulk-bar-sep {{ width: 1px; height: 18px; background: var(--border); }}
.mm-bulk-action {{
  background: none; border: none; color: var(--text); cursor: pointer;
  font-size: 13px; padding: .2rem .4rem; border-radius: 5px;
  font-family: inherit; transition: background .15s, color .15s;
}}
.mm-bulk-action:hover {{ background: var(--border); color: var(--text); }}
.mm-bulk-action.danger {{ color: var(--danger); }}
.mm-bulk-action.danger:hover {{ background: #fee2e2; color: var(--danger); }}
.mm-bulk-dismiss {{ background: none; border: none; color: var(--muted); cursor: pointer; font-size: 16px; line-height: 1; padding: .1rem .25rem; border-radius: 4px; }}
.mm-bulk-dismiss:hover {{ color: var(--text); }}

/* ── Footer / pagination ──────────────────────────────────────────────── */
.mm-footer {{
  display: flex; align-items: center; justify-content: space-between;
  padding: .55rem 1rem;
  background: var(--surface); flex-shrink: 0; flex-wrap: wrap; gap: .5rem;
}}
.mm-footer-info {{ font-size: 13px; color: var(--muted); margin-left: auto; }}

/* ── Dark mode — the neutral panel backgrounds above already switch via
   var(--tint); these are the remaining one-off accent colors (purple
   active/selected states, the neutral count pill, the danger hover) that
   don't map onto an existing variable, so they need explicit dark values. */
:root[data-theme="dark"] .mm-type-item a.active,
:root[data-theme="dark"] .mm-folder-item a.active,
:root[data-theme="dark"] .mm-list tr.selected td {{ background: rgba(129,140,248,.18); }}
:root[data-theme="dark"] .mm-type-count {{ background: var(--tint); color: var(--muted); }}
:root[data-theme="dark"] .mm-type-item a.active .mm-type-count {{ background: rgba(129,140,248,.28); color: var(--primary); }}
:root[data-theme="dark"] .mm-bulk-action.danger:hover {{ background: rgba(248,113,113,.18); color: var(--danger); }}

/* ── Responsive ───────────────────────────────────────────────────────── */
@media (max-width: 900px) {{
  .mm-layout {{ grid-template-columns: 1fr; height: auto; min-height: 0; }}
  .mm-sidebar {{ border-right: none; border-bottom: 1px solid var(--border); display: grid; grid-template-columns: 1fr 1fr; }}
  .mm-sidebar .mm-panel-section + .mm-panel-section {{ border-top: none; border-left: 1px solid var(--border); }}
  .mm-main {{ min-height: 500px; }}
  .mm-content-area {{ min-height: 400px; }}
  .mm-detail-panel {{ width: 100%; border-left: none; }}
}}
@media (max-width: 600px) {{
  .mm-toolbar {{ gap: .4rem; }}
  .mm-sidebar {{ grid-template-columns: 1fr; }}
  .mm-sidebar .mm-panel-section + .mm-panel-section {{ border-left: none; border-top: 1px solid var(--border); }}
  .mm-grid {{ grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); }}
}}
</style>

<div class="mm-layout" id="mmLayout">

  <!-- Toolbar -->
  <div class="mm-toolbar">
    <div class="mm-toolbar-left">{toolbar_title}</div>
    <div class="mm-toolbar-right">
      <div class="icon-pill" style="margin-top:0">
        {search_pill}
        <button class="icon-btn" id="mmBulkToggle" title="Select" aria-label="Select" onclick="toggleBulkMode()"><img src="/admin/static/icons/check-square.svg" alt=""></button>
        <button class="icon-btn icon-btn-active" id="viewGrid" onclick="setView('grid')" title="Grid view" aria-label="Grid view"><img src="/admin/static/icons/grid.svg" alt=""></button>
        <button class="icon-btn" id="viewList" onclick="setView('list')" title="List view" aria-label="List view"><img src="/admin/static/icons/list.svg" alt=""></button>
        <div id="mm-toolbar-app" style="display:contents">
        <form method="POST" action="/admin/media/upload" enctype="multipart/form-data" id="mm2UploadForm" style="display:contents">
          <input type="hidden" name="redirect" value="{redirect_url}">
          {folder_hidden}
          <input type="file" id="mm2FileInput" name="file" accept="image/*,application/pdf,video/*,audio/*"
                 style="position:absolute;width:1px;height:1px;opacity:0;overflow:hidden;pointer-events:none"
                 onchange="mm2Submit()">
          <button type="button" class="icon-btn" id="mmDropzone" title="Upload file" aria-label="Upload file"><img src="/admin/static/icons/upload.svg" alt=""></button>
        </form>
        </div>
      </div>
      {toolbar_close}
    </div>
  </div>

  <!-- Left sidebar -->
  <div class="mm-sidebar">
    <div class="mm-panel-section">
      <ul class="mm-type-list" id="mm-type-tabs-app">
        <li class="mm-type-item">
          <a href="{type_all_url}" class="{type_all_active}" data-type="all">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            All files
            <span class="mm-type-count" id="tc-all">{count_all}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="{type_image_url}" class="{type_image_active}" data-type="image">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>
            Images
            <span class="mm-type-count" id="tc-image">{count_image}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="{type_video_url}" class="{type_video_active}" data-type="video">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><polygon points="23 7 16 12 23 17 23 7"></polygon><rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect></svg>
            Video
            <span class="mm-type-count" id="tc-video">{count_video}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="{type_audio_url}" class="{type_audio_active}" data-type="audio">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>
            Audio
            <span class="mm-type-count" id="tc-audio">{count_audio}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="{type_doc_url}" class="{type_doc_active}" data-type="document">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
            Documents
            <span class="mm-type-count" id="tc-document">{count_doc}</span>
          </a>
        </li>
      </ul>
    </div>

    <div class="mm-panel-section" style="flex:1">
      <div id="mm-folder-select-app" style="display:contents">
      <select class="mm-folder-select" onchange="{folder_onchange}">
        {folder_items}
      </select>
      </div>
      <div id="mm-new-folder-btn-app" style="display:contents">
      <button class="btn btn-primary mm-new-folder-btn" disabled title="Requires JavaScript">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>
        Folder +
      </button>
      </div>
      <div id="mm-delete-folder-app" style="display:contents">{delete_folder_btn}</div>
    </div>
  </div>

  <!-- Main content -->
  <div class="mm-main" id="mmMain">

    <!-- Content area: grid + detail panel side by side -->
    <div class="mm-content-area">

    <!-- Grid / list wrap -->
    <div class="mm-grid-wrap" id="mmGridWrap">

      <div class="mm-grid" id="mmGrid">
        {grid_items}
      </div>

      <p class="mm-empty" id="mmEmpty">No files match your search.</p>

      <table class="mm-list" id="mmList">
        <thead>
          <tr>
            <th style="width:32px"></th>
            <th style="width:52px"></th>
            <th>Filename</th>
            <th>Type</th>
            <th>Size</th>
            <th>Dimensions</th>
            <th></th>
          </tr>
        </thead>
        <tbody id="mmListBody">
          {list_rows}
        </tbody>
      </table>

    </div>

  <!-- Detail panel — now inside mm-content-area, never overlaps drop zone -->
  <div class="mm-detail-panel" id="mmDetail">
    <div class="mm-detail-body" id="mmDetailBody">
      <p style="color:var(--muted);font-size:13px;text-align:center;padding:2rem 0">Select a file to see details.</p>
    </div>
    {detail_actions}
  </div>

    </div><!-- end mm-content-area -->

    <!-- Footer -->
    <div class="mm-footer">
      <div class="pagination" style="margin:0" id="mmPagination">
        {pagination}
      </div>
      <span class="mm-footer-info" id="mmFooterInfo">{footer_info}</span>
    </div>
  </div>

  <!-- Bulk action bar -->
  <div class="mm-bulk-bar icon-pill" id="mmBulkBar">
    <span class="mm-bulk-bar-count"><span class="mm-type-count" id="mmBulkCount">0</span> files</span>
    <div class="mm-bulk-bar-sep"></div>
    <button class="icon-btn" title="Move" aria-label="Move" onclick="bulkMoveTo()"><img src="/admin/static/icons/move.svg" alt=""></button>
    <button class="icon-btn" title="Download" aria-label="Download" onclick="bulkDownload()"><img src="/admin/static/icons/download.svg" alt=""></button>
    <button class="icon-btn icon-danger" title="Delete" aria-label="Delete" onclick="bulkDelete()"><img src="/admin/static/icons/trash.svg" alt=""></button>
  </div>

  <!-- Delete folder modal — owned entirely by the WASM island (renders
       nothing until opened), see media-app/src/components.rs DeleteFolderModal -->
  <div id="mm-delete-folder-modal-app"></div>

  <!-- Move-to folder modal -->
  <div id="mmMoveModal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:200;align-items:center;justify-content:center">
    <div class="modal-card" style="max-width:360px;width:90%">
      <h3 class="modal-card-header">Move to folder</h3>
      <div class="modal-card-body">
        <div class="form-group" style="margin-bottom:1rem">
          <select id="mmMoveSelect" style="width:100%;padding:.35rem .6rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--field-bg);color:var(--field-text)">
            <option value="">— No folder (All media) —</option>
          </select>
        </div>
        <p id="mmMoveError" style="display:none;font-size:13px;color:var(--danger);margin:-.5rem 0 1rem">Move failed. Please try again.</p>
        <div class="icon-pill" style="margin-top:0;justify-content:flex-end">
          <button class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('mmMoveModal').style.display='none'"><img src="/admin/static/icons/x.svg" alt=""></button>
          <button class="icon-btn" title="Move" aria-label="Move" onclick="bulkMoveConfirm()"><img src="/admin/static/icons/move.svg" alt=""></button>
        </div>
      </div>
    </div>
  </div>

  <!-- Delete media modal -->
  <div id="mmDeleteMediaModal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:200;align-items:center;justify-content:center">
    <div class="modal-card" style="max-width:400px;width:90%">
      <h3 class="modal-card-header">Delete files</h3>
      <div class="modal-card-body">
        <p id="mmDeleteMediaMsg" style="font-size:14px;color:var(--muted);margin-bottom:1rem;white-space:pre-line"></p>
        <div class="icon-pill" style="margin-top:0;justify-content:flex-end">
          <button class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('mmDeleteMediaModal').style.display='none'"><img src="/admin/static/icons/x.svg" alt=""></button>
          <button class="icon-btn icon-danger" title="Delete" aria-label="Delete" onclick="confirmBulkDelete()"><img src="/admin/static/icons/trash.svg" alt=""></button>
        </div>
      </div>
    </div>
  </div>

  <!-- New folder modal — owned entirely by the WASM island (renders
       nothing until opened), see media-app/src/components.rs NewFolderModal -->
  <div id="mm-new-folder-modal-app"></div>

</div>

<script>
(function() {{
  // Deliberately NOT `var` — these must be real `window` properties, not
  // closures-local bindings, so the media-app WASM island's refreshes
  // (window.ITEMS/window.FOLDERS reassignment on every folder/type/page
  // change) are actually visible to this legacy script's bare `ITEMS`/
  // `FOLDERS` references, which resolve up the scope chain to `window`.
  window.ITEMS   = {items_json};
  window.FOLDERS = {folders_json};
  var selected = new Set();
  var bulkMode = false;

  /* ── View toggle ─────────────────────────────────────────────────── */
  window.setView = function(v) {{
    var main = document.getElementById('mmMain');
    if (v === 'list') {{
      main.classList.add('mm-view-list');
      document.getElementById('viewGrid').classList.remove('icon-btn-active');
      document.getElementById('viewList').classList.add('icon-btn-active');
    }} else {{
      main.classList.remove('mm-view-list');
      document.getElementById('viewGrid').classList.add('icon-btn-active');
      document.getElementById('viewList').classList.remove('icon-btn-active');
    }}
  }};

  /* ── Live search (type filtering is server-side) ─────────────────── */
  window.filterItems = function() {{
    var q = document.getElementById('mmSearch').value.toLowerCase().trim();
    var gridItems = document.querySelectorAll('#mmGrid .mm-item');
    var listRows  = document.querySelectorAll('#mmListBody tr');
    var visible = 0, total = gridItems.length;

    gridItems.forEach(function(el) {{
      var match = !q || el.dataset.name.indexOf(q) !== -1;
      el.classList.toggle('hidden', !match);
      if (match) visible++;
    }});
    listRows.forEach(function(el) {{
      el.classList.toggle('hidden', !!q && el.dataset.name.indexOf(q) === -1);
    }});

    // Show empty state
    document.getElementById('mmEmpty').classList.toggle('visible', visible === 0);

    // Update footer while actively searching; clearing the search leaves it
    // alone rather than resetting to a stale page-load string — the WASM
    // FooterInfo component (media-app) already keeps it current whenever
    // the grid itself refreshes.
    var info = document.getElementById('mmFooterInfo');
    if (info && q) {{
      info.textContent = visible + ' of ' + total + ' files shown';
    }}
    // Hide pagination when searching
    var pg = document.getElementById('mmPagination');
    if (pg) pg.style.display = q ? 'none' : '';
  }};
  // pill_search_toggle's generated input has no built-in oninput hook (it's
  // a generic shared helper) — wire it here instead of a per-page attribute.
  var mmSearchInput = document.getElementById('mmSearch');
  if (mmSearchInput) mmSearchInput.addEventListener('input', filterItems);

  /* ── Item selection ──────────────────────────────────────────────── */
  window.selectItem = function(el) {{
    var idx = el.dataset.idx;
    if (idx === undefined) return;
    var data = ITEMS[parseInt(idx, 10)];
    if (!data) return;

    if (bulkMode) {{
      el.classList.toggle('selected');
      if (el.classList.contains('selected')) selected.add(idx);
      else selected.delete(idx);
      var cb = el.querySelector('input[type=checkbox]');
      if (cb) cb.checked = el.classList.contains('selected');
    }} else {{
      document.querySelectorAll('.mm-item.selected, #mmListBody tr.selected').forEach(function(e) {{
        e.classList.remove('selected');
      }});
      selected.clear();
      el.classList.add('selected');
      selected.add(idx);
      openDetail(data, parseInt(idx, 10));
    }}
    syncBulkBar();
  }};

  var activeDetailId  = null;
  var activeDetailIdx = null;

  window.saveDetail = function() {{
    if (!activeDetailId) return;
    var alt     = (document.getElementById('mmDetailAlt')     || {{}}).value || '';
    var title   = (document.getElementById('mmDetailTitle')   || {{}}).value || '';
    var caption = (document.getElementById('mmDetailCaption') || {{}}).value || '';
    var btn = document.getElementById('mmSaveBtn');
    if (btn) btn.disabled = true;
    var flashResult = function(ok) {{
      if (!btn) return;
      btn.disabled = false;
      btn.classList.add(ok ? 'icon-btn-active-green' : 'icon-danger-armed');
      btn.title = ok ? 'Saved' : 'Error';
      setTimeout(function() {{
        btn.classList.remove('icon-btn-active-green', 'icon-danger-armed');
        btn.title = 'Save changes';
      }}, 2000);
    }};
    fetch('/admin/api/media/' + activeDetailId + '/meta', {{
      method: 'POST',
      headers: {{'Content-Type': 'application/json'}},
      body: JSON.stringify({{ alt_text: alt.trim(), title: title.trim(), caption: caption.trim() }})
    }}).then(function(r) {{ return r.json(); }}).then(function(res) {{
      if (res.ok && activeDetailIdx !== null) {{
        var item = ITEMS[activeDetailIdx];
        if (item) {{ item.alt = alt.trim(); item.title = title.trim(); item.caption = caption.trim(); }}
      }}
      flashResult(!!res.ok);
    }}).catch(function() {{
      flashResult(false);
    }});
  }};

  function openDetail(data, idx) {{
    var body = document.getElementById('mmDetailBody');
    var actions = document.getElementById('mmDetailActions');
    var preview = data.isImage
      ? '<img class="mm-detail-preview" src="' + data.path + '" alt="">'
      : '<div class="mm-detail-preview-icon"><svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg></div>';
    body.innerHTML = preview +
      '<div class="mm-detail-meta">'
      + '<div class="mm-detail-filename">' + escHtml(data.filename) + '</div>'
      + '<div class="mm-detail-stats">'
      + '<span class="mm-detail-stat-label">Type</span><span class="mm-detail-stat-value">' + escHtml(data.type) + '</span>'
      + '<span class="mm-detail-stat-label">Size</span><span class="mm-detail-stat-value">' + escHtml(data.size) + '</span>'
      + '<span class="mm-detail-stat-label">Dims</span><span class="mm-detail-stat-value">' + escHtml(data.dims) + '</span>'
      + '<span class="mm-detail-stat-label">Path</span><span class="mm-detail-stat-value" style="word-break:break-all">' + escHtml(data.path) + '</span>'
      + '<span class="mm-detail-stat-label">Uploaded</span><span class="mm-detail-stat-value">' + escHtml(data.uploader) + ' — ' + escHtml(data.uploaded_at) + '</span>'
      + '</div></div>'
      + (data.type !== 'audio' ? '<div class="mm-detail-field" style="margin-top:.85rem"><label>Alt text</label><input type="text" id="mmDetailAlt" value="' + escHtml(data.alt) + '" placeholder="Describe the image…"></div>' : '<input type="hidden" id="mmDetailAlt" value="">')
      + '<div class="mm-detail-field"' + (data.type === 'audio' ? ' style="margin-top:.85rem"' : '') + '><label>Title</label><input type="text" id="mmDetailTitle" value="' + escHtml(data.title) + '"></div>'
      + '<div class="mm-detail-field"><label>' + (data.type === 'audio' ? 'Description' : 'Caption') + '</label><textarea id="mmDetailCaption" rows="3" placeholder="' + (data.type === 'audio' ? 'Optional description…' : 'Optional caption…') + '">' + escHtml(data.caption || '') + '</textarea></div>';
    activeDetailId  = data.id;
    activeDetailIdx = (idx !== undefined) ? idx : null;
    actions.style.display = '';
    document.getElementById('mmDetail').classList.add('open');
    // Collapse bulk bar when user focuses any detail field (mobile UX)
    ['mmDetailAlt', 'mmDetailTitle', 'mmDetailCaption'].forEach(function(id) {{
      var el = document.getElementById(id);
      if (el) el.addEventListener('focus', function() {{
        document.getElementById('mmBulkBar').classList.remove('visible');
      }}, {{ once: true }});
    }});
  }}

  window.closeDetail = function() {{
    document.getElementById('mmDetail').classList.remove('open');
    document.getElementById('mmDetailActions').style.display = 'none';
    document.querySelectorAll('.mm-item.selected, #mmListBody tr.selected').forEach(function(e) {{
      e.classList.remove('selected');
    }});
    selected.clear();
    syncBulkBar();
  }};

  /* ── Bulk mode ───────────────────────────────────────────────────── */
  window.toggleBulkMode = function() {{
    bulkMode = !bulkMode;
    var layout = document.getElementById('mmLayout');
    var btn = document.getElementById('mmBulkToggle');
    layout.classList.toggle('mm-bulk-mode', bulkMode);
    btn.classList.toggle('icon-btn-active', bulkMode);
  }};

  window.clearSelection = function() {{
    document.querySelectorAll('.mm-item.selected, #mmListBody tr.selected').forEach(function(e) {{
      e.classList.remove('selected');
      var cb = e.querySelector('input[type=checkbox]');
      if (cb) cb.checked = false;
    }});
    selected.clear();
    syncBulkBar();
  }};

  function syncBulkBar() {{
    var bar = document.getElementById('mmBulkBar');
    var n = selected.size;
    document.getElementById('mmBulkCount').textContent = n;
    bar.classList.toggle('visible', n > 0);
  }}

  /* ── Drop zone — click + drag-and-drop ──────────────────────────── */
  var dz    = document.getElementById('mmDropzone');
  var dzInp = document.getElementById('mm2FileInput');
  var dzFrm = document.getElementById('mm2UploadForm');

  // Click anywhere on the zone → open file dialog
  dz.addEventListener('click', function() {{ dzInp.click(); }});

  // Drag visual feedback
  ['dragover','dragenter'].forEach(function(ev) {{
    dz.addEventListener(ev, function(e) {{
      e.preventDefault();
      dz.classList.add('drag-over');
    }});
  }});
  dz.addEventListener('dragleave', function() {{ dz.classList.remove('drag-over'); }});

  // Drop → transfer files to input and submit
  dz.addEventListener('drop', function(e) {{
    e.preventDefault();
    dz.classList.remove('drag-over');
    var file = e.dataTransfer.files[0];
    if (!file) return;
    // Stuff the dropped file into the hidden input via DataTransfer
    try {{
      var dt = new DataTransfer();
      dt.items.add(file);
      dzInp.files = dt.files;
    }} catch(_) {{}}
    mm2ShowPending(file.name);
    dzFrm.submit();
  }});

  window.mm2Submit = function() {{
    var file = dzInp.files[0];
    if (!file) return;
    mm2ShowPending(file.name);
    dzFrm.submit();
  }};

  function mm2ShowPending(name) {{
    dz.style.opacity = '.6';
    dz.title = 'Uploading ' + escHtml(name) + '…';
  }}

  /* ── Bulk actions ────────────────────────────────────────────────── */
  window.bulkDelete = function() {{
    if (selected.size === 0) return;
    var names = Array.from(selected).map(function(i) {{ return ITEMS[parseInt(i,10)].filename; }});
    var msg = 'Delete ' + selected.size + ' file(s)?\n\n' + names.slice(0,5).join('\n') +
      (names.length > 5 ? '\n…and ' + (names.length-5) + ' more' : '');
    document.getElementById('mmDeleteMediaMsg').textContent = msg;
    document.getElementById('mmDeleteMediaModal').style.display = 'flex';
  }};

  window.confirmBulkDelete = function() {{
    document.getElementById('mmDeleteMediaModal').style.display = 'none';
    var ids = Array.from(selected).map(function(i) {{ return ITEMS[parseInt(i,10)].id; }});
    var chain = Promise.resolve();
    ids.forEach(function(id) {{
      chain = chain.then(function() {{ return fetch('/admin/media/' + id + '/delete', {{method:'POST'}}); }});
    }});
    chain.then(function() {{
      // selected stores idx (array positions) — after refresh the grid array
      // can shrink/reorder, so stale idx must not survive to the next action.
      selected.clear();
      syncBulkBar();
      if (window.mediaAppRefresh) {{ window.mediaAppRefresh(); }} else {{ window.location.reload(); }}
    }});
  }};

  window.bulkDownload = function() {{
    if (selected.size === 0) return;
    Array.from(selected).forEach(function(i) {{
      var item = ITEMS[parseInt(i,10)];
      var a = document.createElement('a');
      a.href = item.path; a.download = item.filename; a.target = '_blank';
      document.body.appendChild(a); a.click(); document.body.removeChild(a);
    }});
  }};

  window.bulkMoveTo = function() {{
    if (selected.size === 0) return;
    var sel = document.getElementById('mmMoveSelect');
    sel.innerHTML = '<option value="">— No folder (All media) —</option>';
    FOLDERS.forEach(function(f) {{
      var opt = document.createElement('option');
      opt.value = f.id; opt.textContent = f.name; sel.appendChild(opt);
    }});
    document.getElementById('mmMoveModal').style.display = 'flex';
  }};

  window.bulkMoveConfirm = function() {{
    document.getElementById('mmMoveError').style.display = 'none';
    var folderId = document.getElementById('mmMoveSelect').value;
    var ids = Array.from(selected).map(function(i) {{ return ITEMS[parseInt(i,10)].id; }});
    var chain = Promise.resolve(true);
    ids.forEach(function(id) {{
      chain = chain.then(function(ok) {{
        if (!ok) return false;
        return fetch('/admin/api/media/' + id + '/folder', {{
          method:'POST', headers:{{'Content-Type':'application/json'}},
          body: JSON.stringify({{folder_id: folderId}})
        }}).then(function(r) {{ return r.json(); }}).then(function(res) {{ return !!res.ok; }});
      }});
    }});
    chain.then(function() {{
      document.getElementById('mmMoveModal').style.display = 'none';
      // selected stores idx (array positions) — after refresh the grid array
      // can shrink/reorder, so stale idx must not survive to the next action.
      selected.clear();
      syncBulkBar();
      if (window.mediaAppRefresh) {{ window.mediaAppRefresh(); }} else {{ window.location.reload(); }}
    }}).catch(function() {{
      document.getElementById('mmMoveError').style.display = '';
    }});
  }};

  // New folder / delete folder are now owned entirely by the WASM island
  // (media-app/src/components.rs: NewFolderButton, NewFolderModal,
  // DeleteFolderButton, DeleteFolderModal) — the legacy prompt/submit/
  // confirm functions and their form.submit()-triggered full reloads were
  // removed in favor of fetch + in-place refresh.

  function escHtml(s) {{
    return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }}

  /* ── Click-outside closes detail panel ──────────────────────────── */
  document.addEventListener('click', function(e) {{
    var panel = document.getElementById('mmDetail');
    if (!panel || !panel.classList.contains('open')) return;
    if (panel.contains(e.target)) return;
    /* ignore clicks on mm-items (they handle their own open/close) */
    if (e.target.closest('.mm-item, #mmListBody tr')) return;
    /* ignore clicks on the bulk action bar and any open modals */
    if (e.target.closest('#mmBulkBar, #mmMoveModal, #mmDeleteFolderModal, #mmNewFolderModal, #mmDeleteMediaModal')) return;
    closeDetail();
  }}, true);
{picker_js}

  // Allow the parent page to relabel the Set/Insert button after load —
  // it's icon-only now, so this updates the tooltip/aria-label rather
  // than textContent (which would wipe out the <img> entirely).
  window.addEventListener('message', function(e) {{
    if (!e.data || e.data.type !== 'pickerSetLabel') return;
    var btn = document.getElementById('mmPickerSetBtn');
    if (btn) {{ btn.title = e.data.label; btn.setAttribute('aria-label', e.data.label); }}
  }});

  // Lets the parent page reuse an already-warm picker iframe on subsequent
  // opens instead of reloading the whole page (see admin/src/lib.rs's
  // openMediaPicker) — resets to a fresh "All Media" view scoped to the
  // requested type filter, in place, without re-running the WASM bootstrap.
  window.addEventListener('message', function(e) {{
    if (!e.data || e.data.type !== 'resetPickerFilter') return;
    if (window.mediaAppResetForPicker) {{ window.mediaAppResetForPicker(e.data.typeFilter || undefined); }}
  }});

  // This frame only re-reads the admin-theme localStorage value (shared
  // with the parent, same origin) on its own initial load. Once warm, it's
  // kept alive across opens/closes (see openMediaBrowser/openMediaPicker in
  // admin/src/lib.rs) instead of reloading, so a theme switch made while
  // this frame isn't showing would otherwise never reach its DOM. The
  // parent posts this on every open (and live, if the frame is open when
  // the switch happens) to keep it in sync without a reload.
  window.addEventListener('message', function(e) {{
    if (!e.data || e.data.type !== 'setTheme') return;
    if (e.data.dark) {{
      document.documentElement.setAttribute('data-theme', 'dark');
    }} else {{
      document.documentElement.removeAttribute('data-theme');
    }}
  }});
}})();
</script>
{pill_search_init}
<script type="module">
  // media-app WASM island: takes over folder/type filtering, pagination,
  // and upload from the static SSR fallback above once it loads. The
  // legacy detail-panel/bulk-action JS in the block above is untouched —
  // it just keeps reading whatever the island puts at window.ITEMS.
  import init, {{ mount, refresh_grid, reset_for_picker }} from '/admin/static/media-app/media_app.js';
  init('/admin/static/media-app/media_app_bg.wasm').then(function() {{
    mount();
    // Lets the legacy (non-module) script below re-fetch the grid in place
    // after bulk move/delete instead of a full window.location.reload().
    window.mediaAppRefresh = refresh_grid;
    // Lets the parent page reset filters on an already-warm picker iframe —
    // see the resetPickerFilter message listener above.
    window.mediaAppResetForPicker = reset_for_picker;
  }});
</script>
"##,
        flash         = flash_html,
        toolbar_title = toolbar_title,
        toolbar_close = toolbar_close,
        search_pill   = search_pill,
        pill_search_init = pill_search_init,
        redirect_url  = redirect_url,
        folder_hidden = folder_hidden,
        count_all    = count_all,
        count_image  = count_image,
        count_video  = count_video,
        count_audio  = count_audio,
        count_doc    = count_doc,
        delete_folder_btn = delete_folder_btn_html,
        folder_items = folder_items_html,
        folder_onchange = folder_onchange,
        type_all_url  = type_all_url,
        type_image_url = type_image_url,
        type_video_url = type_video_url,
        type_audio_url = type_audio_url,
        type_doc_url   = type_doc_url,
        type_all_active   = type_all_active,
        type_image_active = type_image_active,
        type_video_active = type_video_active,
        type_audio_active = type_audio_active,
        type_doc_active   = type_doc_active,
        grid_items   = grid_items,
        list_rows    = list_rows,
        items_json   = items_json,
        folders_json = folders_json,
        footer_info  = footer_info,
        pagination   = pagination_html,
        detail_actions       = detail_actions_html,
        picker_js    = picker_js,
    );

    if picker_mode {
        crate::picker_page(&content)
    } else {
        admin_page(&page_title, "/admin/media", flash, &content, ctx)
    }
}
