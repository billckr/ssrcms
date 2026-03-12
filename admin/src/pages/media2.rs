use crate::{admin_page, html_escape, PageContext};

pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: String,
    pub title: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: i64,
    pub folder_id: Option<String>,
}

pub struct FolderItem {
    pub id: String,
    pub name: String,
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
    total: i64,
    page: i64,
    page_size: i64,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    // ── Type counts ──────────────────────────────────────────────────────────
    let mut count_image = 0usize;
    let mut count_video = 0usize;
    let mut count_audio = 0usize;
    let mut count_doc   = 0usize;
    for item in items {
        match media_type_key(&item.mime_type) {
            "image"    => count_image += 1,
            "video"    => count_video += 1,
            "audio"    => count_audio += 1,
            "document" => count_doc   += 1,
            _          => {}
        }
    }
    let count_all = items.len();

    // ── Grid items ───────────────────────────────────────────────────────────
    let grid_items: String = items.iter().enumerate().map(|(i, m)| {
        let type_key = media_type_key(&m.mime_type);
        let dot_color = type_color(type_key);
        let is_image = m.mime_type.starts_with("image/");
        let fname = html_escape(&m.filename);
        let alt   = html_escape(&m.alt_text);
        let fsize = format_bytes(m.file_size);
        let dims  = match (m.width, m.height) {
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
                r##"<div class="mm-list-thumb" style="display:flex;align-items:center;justify-content:center;background:#f1f5f9;font-size:18px">📄</div>"##
            )
        };
        format!(
            r##"<tr data-idx="{idx}" data-type="{type_key}" data-name="{name_lower}" onclick="selectItem(this)"><td><input type="checkbox" onclick="event.stopPropagation()"></td><td>{thumb}</td><td><strong style="font-size:13px">{fname}</strong></td><td><span class="mm-list-type-pill" style="background:{pbg};color:{pfg}">{plbl}</span></td><td style="color:var(--muted)">{fsize}</td><td style="color:var(--muted)">{dims}</td><td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem" onclick="event.stopPropagation();selectItem(this.closest('tr'))">Edit</button></td></tr>"##,
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
                r##"{{"id":"{id}","filename":"{fn}","type":"{ty}","isImage":{img},"path":"/uploads/{path}","alt":"{alt}","title":"{title}","size":"{size}","dims":"{dims}"}}"##,
                id    = html_escape(&m.id),
                fn    = html_escape(&m.filename),
                ty    = type_key,
                img   = is_image,
                path  = html_escape(&m.path),
                alt   = html_escape(&m.alt_text),
                title = html_escape(&m.title),
                size  = format_bytes(m.file_size),
                dims  = dims,
            )
        }).collect();
        format!("[{}]", parts.join(","))
    };

    // ── Folder sidebar items ─────────────────────────────────────────────────
    let folder_items_html: String = {
        let mut html = String::from(
            r##"<li class="mm-folder-item"><a href="/admin/media2" class="##,
        );
        if active_folder.is_none() { html.push_str("active"); }
        html.push_str(r##""><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>All media</a></li>"##);
        for f in folders {
            let active_class = if active_folder == Some(f.id.as_str()) { " active" } else { "" };
            html.push_str(&format!(
                r##"<li class="mm-folder-item"><a href="/admin/media2?folder_id={id}" class="{ac}"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>{name}</a></li>"##,
                id   = html_escape(&f.id),
                name = html_escape(&f.name),
                ac   = active_class,
            ));
        }
        html
    };

    // ── Pagination ───────────────────────────────────────────────────────────
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as i64;
    let folder_param = active_folder.map(|id| format!("&folder_id={}", id)).unwrap_or_default();

    let pagination_html = if total_pages <= 1 {
        String::new()
    } else {
        let mut p = String::new();
        // Prev
        if page > 1 {
            p.push_str(&format!(
                r##"<a href="/admin/media2?page={}{}" class="page-btn">&lsaquo; Prev</a>"##,
                page - 1, folder_param
            ));
        } else {
            p.push_str(r##"<span class="page-btn page-btn-disabled">&lsaquo; Prev</span>"##);
        }
        // Page numbers (show at most 7 around current)
        let start = (page - 3).max(1);
        let end   = (page + 3).min(total_pages);
        if start > 1 {
            p.push_str(&format!(r##"<a href="/admin/media2?page=1{}" class="page-btn">1</a>"##, folder_param));
            if start > 2 { p.push_str(r##"<span class="page-btn" style="pointer-events:none;color:var(--muted)">…</span>"##); }
        }
        for n in start..=end {
            if n == page {
                p.push_str(&format!(r##"<span class="page-btn page-btn-active">{}</span>"##, n));
            } else {
                p.push_str(&format!(
                    r##"<a href="/admin/media2?page={n}{fp}" class="page-btn">{n}</a>"##,
                    n = n, fp = folder_param
                ));
            }
        }
        if end < total_pages {
            if end < total_pages - 1 { p.push_str(r##"<span class="page-btn" style="pointer-events:none;color:var(--muted)">…</span>"##); }
            p.push_str(&format!(r##"<a href="/admin/media2?page={tp}{fp}" class="page-btn">{tp}</a>"##, tp = total_pages, fp = folder_param));
        }
        // Next
        if page < total_pages {
            p.push_str(&format!(
                r##"<a href="/admin/media2?page={}{}" class="page-btn">Next &rsaquo;</a>"##,
                page + 1, folder_param
            ));
        } else {
            p.push_str(r##"<span class="page-btn page-btn-disabled">Next &rsaquo;</span>"##);
        }
        p
    };

    let showing_from = if total == 0 { 0 } else { (page - 1) * page_size + 1 };
    let showing_to   = (page * page_size).min(total);
    let footer_info  = format!("Showing {}–{} of {} files", showing_from, showing_to, total);

    let flash_html = match flash {
        Some(msg) => format!(r##"<div class="flash success">{}</div>"##, html_escape(msg)),
        None => String::new(),
    };

    let content = format!(r##"
{flash}
<style>
/* ── Media Manager 2 — page-scoped styles ────────────────────────────── */
.mm-layout {{
  display: grid;
  grid-template-columns: 220px 1fr;
  grid-template-rows: auto 1fr;
  gap: 0;
  height: calc(100vh - 120px);
  min-height: 500px;
  background: var(--surface);
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
  background: #f8fafc;
  border-bottom: 1px solid var(--border);
  flex-wrap: wrap;
}}
.mm-toolbar-left  {{ display: flex; align-items: center; gap: .5rem; flex: 1; min-width: 0; flex-wrap: wrap; }}
.mm-toolbar-right {{ display: flex; align-items: center; gap: .5rem; flex-shrink: 0; }}

.mm-search {{
  display: flex;
  align-items: center;
  gap: .4rem;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: .3rem .6rem;
  min-width: 180px;
  max-width: 280px;
  flex: 1;
}}
.mm-search svg {{ color: var(--muted); flex-shrink: 0; }}
.mm-search input {{
  border: none; outline: none; font-size: 13px;
  background: transparent; width: 100%; color: var(--text);
}}
.mm-search input::placeholder {{ color: var(--muted); }}

.mm-view-toggle {{ display: flex; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }}
.mm-view-btn {{
  width: 32px; height: 32px;
  display: flex; align-items: center; justify-content: center;
  background: var(--surface); border: none; cursor: pointer;
  color: var(--muted); transition: background .15s, color .15s;
}}
.mm-view-btn.active {{ background: var(--primary); color: #fff; }}
.mm-view-btn:hover:not(.active) {{ background: #f1f5f9; color: var(--text); }}

.mm-bulk-btn {{ font-size: 12px; padding: .3rem .65rem; height: 2rem; }}

/* ── Left panel ───────────────────────────────────────────────────────── */
.mm-sidebar {{
  border-right: 1px solid var(--border);
  background: #fafbfc;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}}
.mm-panel-section {{ padding: .65rem .85rem .4rem; }}
.mm-panel-section + .mm-panel-section {{ border-top: 1px solid var(--border); }}
.mm-panel-label {{
  font-size: 10px; font-weight: 700; text-transform: uppercase;
  letter-spacing: .07em; color: var(--muted); margin-bottom: .45rem;
}}

.mm-type-list {{ list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }}
.mm-type-item a {{
  display: flex; align-items: center; gap: .55rem;
  padding: .38rem .55rem; border-radius: 5px; font-size: 13px;
  color: var(--text); text-decoration: none; transition: background .12s;
  cursor: pointer;
}}
.mm-type-item a:hover {{ background: #eef2f7; }}
.mm-type-item a.active {{ background: #ede9fe; color: var(--primary); font-weight: 600; }}
.mm-type-dot {{ width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }}
.mm-type-count {{
  margin-left: auto; font-size: 11px; color: var(--muted);
  background: var(--border); border-radius: 10px;
  padding: .05rem .4rem; font-weight: 600;
}}
.mm-type-item a.active .mm-type-count {{ background: #ddd6fe; color: var(--primary); }}

.mm-folder-list {{ list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }}
.mm-folder-item a {{
  display: flex; align-items: center; gap: .5rem;
  padding: .35rem .5rem; border-radius: 5px; font-size: 13px;
  color: var(--text); text-decoration: none; transition: background .12s;
}}
.mm-folder-item a:hover {{ background: #eef2f7; }}
.mm-folder-item a.active {{ background: #ede9fe; color: var(--primary); }}
.mm-folder-item svg {{ flex-shrink: 0; color: var(--muted); }}
.mm-folder-item a.active svg {{ color: var(--primary); }}
.mm-new-folder-btn {{ margin: .5rem .85rem .65rem; font-size: 12px; width: calc(100% - 1.7rem); justify-content: flex-start; gap: .4rem; }}

/* ── Main ─────────────────────────────────────────────────────────────── */
.mm-main {{ display: flex; flex-direction: column; overflow: hidden; min-width: 0; }}

.mm-dropzone {{
  margin: .75rem .85rem .5rem;
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  padding: .85rem 1rem;
  display: flex; align-items: center; gap: .85rem;
  background: #fafbfc; cursor: pointer;
  transition: border-color .2s, background .2s; flex-shrink: 0;
}}
.mm-dropzone:hover, .mm-dropzone.drag-over {{ border-color: var(--primary); background: #ede9fe; }}
.mm-dropzone-icon {{
  width: 40px; height: 40px; border-radius: 8px;
  background: #ede9fe; display: flex; align-items: center;
  justify-content: center; color: var(--primary); flex-shrink: 0;
}}
.mm-dropzone-text strong {{ display: block; font-size: 13px; color: var(--text); }}
.mm-dropzone-text span {{ font-size: 12px; color: var(--muted); }}
.mm-dropzone-text .mm-browse-link {{ color: var(--primary); font-weight: 600; }}

.mm-grid-wrap {{ flex: 1; overflow-y: auto; padding: 0 .85rem .85rem; }}

.mm-grid {{
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  gap: .65rem;
}}
.mm-item {{
  position: relative; border-radius: var(--radius);
  border: 2px solid transparent; overflow: hidden;
  cursor: pointer; background: #f1f5f9;
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
  border-bottom: 1px solid var(--border); background: #f8fafc; white-space: nowrap;
}}
.mm-list td {{ padding: .5rem .75rem; font-size: 13px; border-bottom: 1px solid var(--border); vertical-align: middle; }}
.mm-list tr:hover td {{ background: #f8fafc; }}
.mm-list tr.selected td {{ background: #ede9fe; }}
.mm-list tr.hidden {{ display: none !important; }}
.mm-list .mm-list-thumb {{ width: 40px; height: 40px; border-radius: 4px; object-fit: cover; display: block; background: #f1f5f9; }}
.mm-list-type-pill {{ display: inline-block; padding: .15rem .45rem; border-radius: 99px; font-size: 11px; font-weight: 600; }}

/* Empty state */
.mm-empty {{ display: none; padding: 3rem; text-align: center; color: var(--muted); font-size: 13px; }}
.mm-empty.visible {{ display: block; }}

/* ── Detail panel ─────────────────────────────────────────────────────── */
.mm-detail-panel {{
  position: absolute; top: 41px; right: 0; bottom: 0; width: 280px;
  background: var(--surface); border-left: 1px solid var(--border);
  display: flex; flex-direction: column;
  transform: translateX(100%); transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 10; overflow: hidden;
}}
.mm-detail-panel.open {{ transform: translateX(0); }}

.mm-detail-header {{
  display: flex; align-items: center; justify-content: space-between;
  padding: .65rem .9rem; border-bottom: 1px solid var(--border);
  background: #f8fafc; flex-shrink: 0;
}}
.mm-detail-header span {{ font-size: 13px; font-weight: 600; color: var(--text); }}
.mm-detail-close {{
  background: none; border: none; cursor: pointer; color: var(--muted);
  padding: .2rem; border-radius: 4px;
  display: flex; align-items: center; justify-content: center;
}}
.mm-detail-close:hover {{ background: #f1f5f9; color: var(--text); }}

.mm-detail-body {{ flex: 1; overflow-y: auto; padding: .9rem; }}
.mm-detail-preview {{
  width: 100%; aspect-ratio: 4/3; object-fit: contain;
  border-radius: var(--radius); background: #f1f5f9; display: block; margin-bottom: .85rem;
}}
.mm-detail-preview-icon {{
  width: 100%; aspect-ratio: 4/3; display: flex;
  align-items: center; justify-content: center;
  background: #f1f5f9; border-radius: var(--radius); margin-bottom: .85rem; color: var(--muted);
}}
.mm-detail-filename {{ font-weight: 600; font-size: 13px; color: var(--text); word-break: break-all; margin-bottom: .5rem; }}
.mm-detail-stats {{ display: grid; grid-template-columns: 1fr 1fr; gap: .3rem .75rem; font-size: 12px; }}
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
  background: var(--surface); color: var(--text);
}}
.mm-detail-field textarea {{ resize: vertical; min-height: 64px; }}
.mm-detail-field input:focus,
.mm-detail-field textarea:focus {{
  outline: none; border-color: var(--primary);
  box-shadow: 0 0 0 2px rgba(79,70,229,.12);
}}

.mm-detail-url {{
  display: flex; align-items: center; gap: .4rem;
  background: #f1f5f9; border: 1px solid var(--border);
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
  background: #1e293b; color: #f8fafc;
  border-radius: 10px; padding: .6rem 1rem;
  display: flex; align-items: center; gap: .75rem;
  box-shadow: 0 8px 32px rgba(0,0,0,.25);
  transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 20; white-space: nowrap; font-size: 13px;
}}
.mm-bulk-bar.visible {{ transform: translateX(-50%) translateY(0); }}
.mm-bulk-bar-count {{ font-weight: 700; color: #a5b4fc; }}
.mm-bulk-bar-sep {{ width: 1px; height: 18px; background: #334155; }}
.mm-bulk-action {{
  background: none; border: none; color: #cbd5e1; cursor: pointer;
  font-size: 13px; padding: .2rem .4rem; border-radius: 5px;
  font-family: inherit; transition: background .15s, color .15s;
}}
.mm-bulk-action:hover {{ background: #334155; color: #f8fafc; }}
.mm-bulk-action.danger:hover {{ background: #7f1d1d; color: #fca5a5; }}
.mm-bulk-dismiss {{ background: none; border: none; color: #64748b; cursor: pointer; font-size: 16px; line-height: 1; padding: .1rem .25rem; border-radius: 4px; }}
.mm-bulk-dismiss:hover {{ color: #f8fafc; }}

/* ── Footer / pagination ──────────────────────────────────────────────── */
.mm-footer {{
  display: flex; align-items: center; justify-content: space-between;
  padding: .55rem 1rem; border-top: 1px solid var(--border);
  background: #f8fafc; flex-shrink: 0; flex-wrap: wrap; gap: .5rem;
}}
.mm-footer-info {{ font-size: 12px; color: var(--muted); }}

/* ── Responsive ───────────────────────────────────────────────────────── */
@media (max-width: 900px) {{
  .mm-layout {{ grid-template-columns: 1fr; height: auto; min-height: 0; }}
  .mm-sidebar {{ border-right: none; border-bottom: 1px solid var(--border); display: grid; grid-template-columns: 1fr 1fr; }}
  .mm-sidebar .mm-panel-section + .mm-panel-section {{ border-top: none; border-left: 1px solid var(--border); }}
  .mm-new-folder-btn {{ display: none; }}
  .mm-main {{ min-height: 500px; }}
  .mm-detail-panel {{ top: 0; width: 100%; height: 100%; border-left: none; }}
}}
@media (max-width: 600px) {{
  .mm-toolbar {{ gap: .4rem; }}
  .mm-search {{ min-width: 120px; }}
  .mm-sidebar {{ grid-template-columns: 1fr; }}
  .mm-sidebar .mm-panel-section + .mm-panel-section {{ border-left: none; border-top: 1px solid var(--border); }}
  .mm-grid {{ grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); }}
  .mm-dropzone-text span {{ display: none; }}
}}
</style>

<div class="mm-layout" id="mmLayout">

  <!-- Toolbar -->
  <div class="mm-toolbar">
    <div class="mm-toolbar-left">
      <div class="mm-search">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input type="text" placeholder="Search files…" id="mmSearch" oninput="filterItems()">
      </div>
    </div>
    <div class="mm-toolbar-right">
      <button class="btn btn-secondary mm-bulk-btn" id="mmBulkToggle" onclick="toggleBulkMode()">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right:.3rem"><rect x="3" y="5" width="4" height="4" rx="1"/><rect x="3" y="12" width="4" height="4" rx="1"/><rect x="3" y="19" width="4" height="4" rx="1"/><line x1="10" y1="7" x2="21" y2="7"/><line x1="10" y1="14" x2="21" y2="14"/><line x1="10" y1="21" x2="21" y2="21"/></svg>
        Select
      </button>
      <div class="mm-view-toggle">
        <button class="mm-view-btn active" id="viewGrid" onclick="setView('grid')" title="Grid view">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
        </button>
        <button class="mm-view-btn" id="viewList" onclick="setView('list')" title="List view">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
        </button>
      </div>
      <button class="btn btn-primary" style="font-size:13px;height:2rem;padding:.3rem .75rem">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="margin-right:.3rem"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Upload
      </button>
    </div>
  </div>

  <!-- Left sidebar -->
  <div class="mm-sidebar">
    <div class="mm-panel-section">
      <div class="mm-panel-label">Filter by type</div>
      <ul class="mm-type-list">
        <li class="mm-type-item">
          <a href="#" class="active" data-type="all" onclick="setTypeFilter(event,'all')">
            <span class="mm-type-dot" style="background:#4f46e5"></span>
            All files
            <span class="mm-type-count" id="tc-all">{count_all}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#" data-type="image" onclick="setTypeFilter(event,'image')">
            <span class="mm-type-dot" style="background:#10b981"></span>
            Images
            <span class="mm-type-count" id="tc-image">{count_image}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#" data-type="video" onclick="setTypeFilter(event,'video')">
            <span class="mm-type-dot" style="background:#f59e0b"></span>
            Video
            <span class="mm-type-count" id="tc-video">{count_video}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#" data-type="audio" onclick="setTypeFilter(event,'audio')">
            <span class="mm-type-dot" style="background:#8b5cf6"></span>
            Audio
            <span class="mm-type-count" id="tc-audio">{count_audio}</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#" data-type="document" onclick="setTypeFilter(event,'document')">
            <span class="mm-type-dot" style="background:#64748b"></span>
            Documents
            <span class="mm-type-count" id="tc-document">{count_doc}</span>
          </a>
        </li>
      </ul>
    </div>

    <div class="mm-panel-section" style="flex:1">
      <div class="mm-panel-label">Folders</div>
      <ul class="mm-folder-list">
        {folder_items}
      </ul>
      <button class="btn btn-secondary mm-new-folder-btn" onclick="promptNewFolder()">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        New folder
      </button>
    </div>
  </div>

  <!-- Main content -->
  <div class="mm-main" id="mmMain">

    <!-- Drop zone -->
    <div class="mm-dropzone" id="mmDropzone">
      <div class="mm-dropzone-icon">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>
      </div>
      <div class="mm-dropzone-text">
        <strong>Drop files here to upload</strong>
        <span>or <span class="mm-browse-link">browse your computer</span> — JPG, PNG, GIF, PDF, MP4…</span>
      </div>
    </div>

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

    <!-- Footer -->
    <div class="mm-footer">
      <span class="mm-footer-info" id="mmFooterInfo">{footer_info}</span>
      <div class="pagination" style="margin:0" id="mmPagination">
        {pagination}
      </div>
    </div>
  </div>

  <!-- Detail panel -->
  <div class="mm-detail-panel" id="mmDetail">
    <div class="mm-detail-header">
      <span>File details</span>
      <button class="mm-detail-close" onclick="closeDetail()">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
    <div class="mm-detail-body" id="mmDetailBody">
      <p style="color:var(--muted);font-size:13px;text-align:center;padding:2rem 0">Select a file to see details.</p>
    </div>
    <div class="mm-detail-actions" id="mmDetailActions" style="display:none">
      <button class="btn btn-primary" style="width:100%;justify-content:center">Save changes</button>
      <button class="btn btn-danger" style="width:100%;justify-content:center">Delete file</button>
    </div>
  </div>

  <!-- Bulk action bar -->
  <div class="mm-bulk-bar" id="mmBulkBar">
    <span class="mm-bulk-bar-count" id="mmBulkCount">0 selected</span>
    <div class="mm-bulk-bar-sep"></div>
    <button class="mm-bulk-action">Move to…</button>
    <button class="mm-bulk-action">Download</button>
    <button class="mm-bulk-action danger">Delete</button>
    <div class="mm-bulk-bar-sep"></div>
    <button class="mm-bulk-dismiss" onclick="clearSelection()" title="Clear">&#10005;</button>
  </div>

</div>

<script>
(function() {{
  var ITEMS = {items_json};
  var selected = new Set();
  var bulkMode = false;
  var activeType = 'all';

  /* ── View toggle ─────────────────────────────────────────────────── */
  window.setView = function(v) {{
    var main = document.getElementById('mmMain');
    if (v === 'list') {{
      main.classList.add('mm-view-list');
      document.getElementById('viewGrid').classList.remove('active');
      document.getElementById('viewList').classList.add('active');
    }} else {{
      main.classList.remove('mm-view-list');
      document.getElementById('viewGrid').classList.add('active');
      document.getElementById('viewList').classList.remove('active');
    }}
  }};

  /* ── Type filter ─────────────────────────────────────────────────── */
  window.setTypeFilter = function(e, type) {{
    e.preventDefault();
    activeType = type;
    document.querySelectorAll('.mm-type-item a').forEach(function(a) {{
      a.classList.toggle('active', a.dataset.type === type);
    }});
    filterItems();
  }};

  /* ── Live search + type filter ───────────────────────────────────── */
  window.filterItems = function() {{
    var q = document.getElementById('mmSearch').value.toLowerCase().trim();
    var gridItems = document.querySelectorAll('#mmGrid .mm-item');
    var listRows  = document.querySelectorAll('#mmListBody tr');
    var visible = 0, total = gridItems.length;

    gridItems.forEach(function(el) {{
      var match = (activeType === 'all' || el.dataset.type === activeType)
               && (!q || el.dataset.name.indexOf(q) !== -1);
      el.classList.toggle('hidden', !match);
      if (match) visible++;
    }});
    listRows.forEach(function(el) {{
      var match = (activeType === 'all' || el.dataset.type === activeType)
               && (!q || el.dataset.name.indexOf(q) !== -1);
      el.classList.toggle('hidden', !match);
    }});

    // Update counts in type pills
    var typeCounts = {{}};
    gridItems.forEach(function(el) {{
      if (!el.classList.contains('hidden')) {{
        typeCounts[el.dataset.type] = (typeCounts[el.dataset.type] || 0) + 1;
      }}
    }});
    ['image','video','audio','document'].forEach(function(t) {{
      var el = document.getElementById('tc-' + t);
      if (el) el.textContent = typeCounts[t] || 0;
    }});
    var allEl = document.getElementById('tc-all');
    if (allEl) allEl.textContent = visible;

    // Show empty state
    document.getElementById('mmEmpty').classList.toggle('visible', visible === 0);

    // Update footer
    var info = document.getElementById('mmFooterInfo');
    if (info) {{
      if (q || activeType !== 'all') {{
        info.textContent = visible + ' of ' + total + ' files shown';
      }} else {{
        info.textContent = '{footer_info}';
      }}
    }}
    // Hide pagination when filtering
    var pg = document.getElementById('mmPagination');
    if (pg) pg.style.display = (q || activeType !== 'all') ? 'none' : '';
  }};

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
    }} else {{
      document.querySelectorAll('.mm-item.selected, #mmListBody tr.selected').forEach(function(e) {{
        e.classList.remove('selected');
      }});
      selected.clear();
      el.classList.add('selected');
      selected.add(idx);
      openDetail(data);
    }}
    syncBulkBar();
  }};

  function openDetail(data) {{
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
      + '</div></div>'
      + '<div class="mm-detail-url"><span>' + escHtml(data.path) + '</span>'
      + '<button class="btn-link" style="font-size:12px;flex-shrink:0" onclick="navigator.clipboard.writeText(\'' + escHtml(data.path) + '\')">Copy</button></div>'
      + '<div class="mm-detail-field"><label>Alt text</label><input type="text" value="' + escHtml(data.alt) + '" placeholder="Describe the image…"></div>'
      + '<div class="mm-detail-field"><label>Title</label><input type="text" value="' + escHtml(data.title) + '"></div>';
    actions.style.display = '';
    document.getElementById('mmDetail').classList.add('open');
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
    if (bulkMode) {{
      btn.textContent = 'Done';
    }} else {{
      btn.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right:.3rem"><rect x="3" y="5" width="4" height="4" rx="1"/><rect x="3" y="12" width="4" height="4" rx="1"/><rect x="3" y="19" width="4" height="4" rx="1"/><line x1="10" y1="7" x2="21" y2="7"/><line x1="10" y1="14" x2="21" y2="14"/><line x1="10" y1="21" x2="21" y2="21"/></svg>Select';
    }}
  }};

  window.clearSelection = function() {{
    document.querySelectorAll('.mm-item.selected, #mmListBody tr.selected').forEach(function(e) {{
      e.classList.remove('selected');
    }});
    selected.clear();
    syncBulkBar();
  }};

  function syncBulkBar() {{
    var bar = document.getElementById('mmBulkBar');
    var n = selected.size;
    document.getElementById('mmBulkCount').textContent = n + ' selected';
    bar.classList.toggle('visible', n > 0);
  }}

  /* ── Drag-over on drop zone ──────────────────────────────────────── */
  var dz = document.getElementById('mmDropzone');
  ['dragover','dragenter'].forEach(function(ev) {{
    dz.addEventListener(ev, function(e) {{ e.preventDefault(); dz.classList.add('drag-over'); }});
  }});
  ['dragleave','drop'].forEach(function(ev) {{
    dz.addEventListener(ev, function(e) {{ e.preventDefault(); dz.classList.remove('drag-over'); }});
  }});

  /* ── New folder (placeholder) ────────────────────────────────────── */
  window.promptNewFolder = function() {{
    var name = prompt('Folder name (letters, numbers, hyphens):');
    if (!name) return;
    name = name.trim().replace(/[^a-zA-Z0-9\-]/g, '');
    if (!name || name.length < 2) {{ alert('Name too short.'); return; }}
    var form = document.createElement('form');
    form.method = 'POST';
    form.action = '/admin/media/folders/new';
    var inp = document.createElement('input');
    inp.name = 'name'; inp.value = name;
    form.appendChild(inp);
    document.body.appendChild(form);
    form.submit();
  }};

  function escHtml(s) {{
    return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }}
}})();
</script>
"##,
        flash        = flash_html,
        count_all    = count_all,
        count_image  = count_image,
        count_video  = count_video,
        count_audio  = count_audio,
        count_doc    = count_doc,
        folder_items = folder_items_html,
        grid_items   = grid_items,
        list_rows    = list_rows,
        items_json   = items_json,
        footer_info  = footer_info,
        pagination   = pagination_html,
    );

    admin_page("Media Library", "/admin/media2", flash, &content, ctx)
}
