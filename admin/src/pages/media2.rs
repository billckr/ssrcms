use crate::{admin_page, PageContext};

pub fn render_list(ctx: &PageContext) -> String {
    let content = r##"
<style>
/* ── Media Manager 2 — page-scoped styles ─────────────────────────────── */
.mm-layout {
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
}

/* ── Toolbar (full width, top) ──────────────────────────────────────── */
.mm-toolbar {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: .6rem;
  padding: .65rem 1rem;
  background: #f8fafc;
  border-bottom: 1px solid var(--border);
  flex-wrap: wrap;
}
.mm-toolbar-left  { display: flex; align-items: center; gap: .5rem; flex: 1; min-width: 0; flex-wrap: wrap; }
.mm-toolbar-right { display: flex; align-items: center; gap: .5rem; flex-shrink: 0; }

.mm-search {
  display: flex;
  align-items: center;
  gap: .4rem;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: .3rem .6rem;
  min-width: 180px;
  max-width: 260px;
  flex: 1;
}
.mm-search svg { color: var(--muted); flex-shrink: 0; }
.mm-search input {
  border: none;
  outline: none;
  font-size: 13px;
  background: transparent;
  width: 100%;
  color: var(--text);
}
.mm-search input::placeholder { color: var(--muted); }

.mm-sort {
  padding: .3rem .55rem;
  font-size: 12px;
  font-family: inherit;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  color: var(--text);
  height: 2rem;
  cursor: pointer;
}

.mm-view-toggle { display: flex; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
.mm-view-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--surface);
  border: none;
  cursor: pointer;
  color: var(--muted);
  transition: background .15s, color .15s;
}
.mm-view-btn.active { background: var(--primary); color: #fff; }
.mm-view-btn:hover:not(.active) { background: #f1f5f9; color: var(--text); }

.mm-bulk-btn {
  font-size: 12px;
  padding: .3rem .65rem;
  height: 2rem;
}

/* ── Left panel ─────────────────────────────────────────────────────── */
.mm-sidebar {
  border-right: 1px solid var(--border);
  background: #fafbfc;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.mm-panel-section {
  padding: .65rem .85rem .4rem;
}
.mm-panel-section + .mm-panel-section {
  border-top: 1px solid var(--border);
}
.mm-panel-label {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: .07em;
  color: var(--muted);
  margin-bottom: .45rem;
}

/* Type filter pills */
.mm-type-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }
.mm-type-item a {
  display: flex;
  align-items: center;
  gap: .55rem;
  padding: .38rem .55rem;
  border-radius: 5px;
  font-size: 13px;
  color: var(--text);
  text-decoration: none;
  transition: background .12s;
}
.mm-type-item a:hover { background: #eef2f7; }
.mm-type-item a.active { background: #ede9fe; color: var(--primary); font-weight: 600; }
.mm-type-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.mm-type-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--muted);
  background: var(--border);
  border-radius: 10px;
  padding: .05rem .4rem;
  font-weight: 600;
}
.mm-type-item a.active .mm-type-count { background: #ddd6fe; color: var(--primary); }

/* Folder tree */
.mm-folder-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }
.mm-folder-item a {
  display: flex;
  align-items: center;
  gap: .5rem;
  padding: .35rem .5rem;
  border-radius: 5px;
  font-size: 13px;
  color: var(--text);
  text-decoration: none;
  transition: background .12s;
}
.mm-folder-item a:hover { background: #eef2f7; }
.mm-folder-item a.active { background: #ede9fe; color: var(--primary); }
.mm-folder-item svg { flex-shrink: 0; color: var(--muted); }
.mm-folder-item a.active svg { color: var(--primary); }
.mm-new-folder-btn {
  margin: .5rem .85rem .65rem;
  font-size: 12px;
  width: calc(100% - 1.7rem);
  justify-content: flex-start;
  gap: .4rem;
}

/* ── Main grid area ─────────────────────────────────────────────────── */
.mm-main {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}

/* Drop zone (top of grid area) */
.mm-dropzone {
  margin: .75rem .85rem .5rem;
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  padding: .85rem 1rem;
  display: flex;
  align-items: center;
  gap: .85rem;
  background: #fafbfc;
  cursor: pointer;
  transition: border-color .2s, background .2s;
  flex-shrink: 0;
}
.mm-dropzone:hover, .mm-dropzone.drag-over {
  border-color: var(--primary);
  background: #ede9fe;
}
.mm-dropzone-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  background: #ede9fe;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--primary);
  flex-shrink: 0;
}
.mm-dropzone-text strong { display: block; font-size: 13px; color: var(--text); }
.mm-dropzone-text span { font-size: 12px; color: var(--muted); }
.mm-dropzone-text .mm-browse-link { color: var(--primary); font-weight: 600; }

/* Grid */
.mm-grid-wrap {
  flex: 1;
  overflow-y: auto;
  padding: 0 .85rem .85rem;
}

.mm-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  gap: .65rem;
}

.mm-item {
  position: relative;
  border-radius: var(--radius);
  border: 2px solid transparent;
  overflow: hidden;
  cursor: pointer;
  background: #f1f5f9;
  transition: border-color .15s, box-shadow .15s;
  aspect-ratio: 1;
}
.mm-item:hover { border-color: #c7d2fe; box-shadow: 0 2px 8px rgba(79,70,229,.12); }
.mm-item.selected { border-color: var(--primary); box-shadow: 0 0 0 3px rgba(79,70,229,.18); }

.mm-item img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  pointer-events: none;
}
.mm-item-icon {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: .4rem;
  color: var(--muted);
}
.mm-item-icon svg { width: 32px; height: 32px; }
.mm-item-icon span { font-size: 11px; font-weight: 700; letter-spacing: .04em; text-transform: uppercase; }

/* Type accent strip at bottom */
.mm-item-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: .3rem .45rem;
  background: rgba(15,23,42,.55);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  gap: .3rem;
}
.mm-item-bar-name {
  font-size: 10px;
  color: #fff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  line-height: 1.3;
}
.mm-item-type-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

/* Checkbox overlay */
.mm-item-check {
  position: absolute;
  top: .4rem;
  left: .4rem;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  border: 2px solid rgba(255,255,255,.8);
  background: rgba(0,0,0,.25);
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity .15s;
}
.mm-bulk-mode .mm-item-check,
.mm-item:hover .mm-item-check { opacity: 1; }
.mm-item.selected .mm-item-check {
  background: var(--primary);
  border-color: var(--primary);
  opacity: 1;
}
.mm-item.selected .mm-item-check::after {
  content: '';
  display: block;
  width: 5px;
  height: 9px;
  border: 2px solid #fff;
  border-top: none;
  border-left: none;
  transform: rotate(45deg) translate(-1px,-1px);
}

/* List view */
.mm-list { display: none; }
.mm-view-list .mm-grid { display: none; }
.mm-view-list .mm-list { display: table; width: 100%; border-collapse: collapse; }
.mm-list thead th {
  text-align: left;
  padding: .5rem .75rem;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: .05em;
  color: var(--muted);
  border-bottom: 1px solid var(--border);
  background: #f8fafc;
  white-space: nowrap;
}
.mm-list td {
  padding: .5rem .75rem;
  font-size: 13px;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}
.mm-list tr:hover td { background: #f8fafc; }
.mm-list tr.selected td { background: #ede9fe; }
.mm-list .mm-list-thumb {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  object-fit: cover;
  display: block;
  background: #f1f5f9;
}
.mm-list-type-pill {
  display: inline-block;
  padding: .15rem .45rem;
  border-radius: 99px;
  font-size: 11px;
  font-weight: 600;
}

/* ── Detail panel (right) ───────────────────────────────────────────── */
.mm-detail-panel {
  position: absolute;
  top: 41px; /* below toolbar */
  right: 0;
  bottom: 0;
  width: 280px;
  background: var(--surface);
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  transform: translateX(100%);
  transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 10;
  overflow: hidden;
}
.mm-detail-panel.open { transform: translateX(0); }

.mm-detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: .65rem .9rem;
  border-bottom: 1px solid var(--border);
  background: #f8fafc;
  flex-shrink: 0;
}
.mm-detail-header span { font-size: 13px; font-weight: 600; color: var(--text); }
.mm-detail-close {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--muted);
  padding: .2rem;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.mm-detail-close:hover { background: #f1f5f9; color: var(--text); }

.mm-detail-body { flex: 1; overflow-y: auto; padding: .9rem; }

.mm-detail-preview {
  width: 100%;
  aspect-ratio: 4/3;
  object-fit: contain;
  border-radius: var(--radius);
  background: #f1f5f9;
  display: block;
  margin-bottom: .85rem;
}
.mm-detail-preview-icon {
  width: 100%;
  aspect-ratio: 4/3;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f1f5f9;
  border-radius: var(--radius);
  margin-bottom: .85rem;
  color: var(--muted);
}
.mm-detail-preview-icon svg { width: 48px; height: 48px; }

.mm-detail-meta { margin-bottom: .85rem; }
.mm-detail-filename {
  font-weight: 600;
  font-size: 13px;
  color: var(--text);
  word-break: break-all;
  margin-bottom: .5rem;
}
.mm-detail-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: .3rem .75rem;
  font-size: 12px;
}
.mm-detail-stat-label { color: var(--muted); }
.mm-detail-stat-value { color: var(--text); font-weight: 500; }

.mm-detail-field { margin-bottom: .65rem; }
.mm-detail-field label {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  margin-bottom: .3rem;
  text-transform: uppercase;
  letter-spacing: .04em;
}
.mm-detail-field input[type=text],
.mm-detail-field textarea {
  width: 100%;
  padding: .4rem .6rem;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-size: 13px;
  font-family: inherit;
  background: var(--surface);
  color: var(--text);
}
.mm-detail-field textarea { resize: vertical; min-height: 64px; }
.mm-detail-field input:focus,
.mm-detail-field textarea:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 2px rgba(79,70,229,.12);
}

.mm-detail-url {
  display: flex;
  align-items: center;
  gap: .4rem;
  background: #f1f5f9;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: .35rem .6rem;
  font-size: 12px;
  color: var(--muted);
  word-break: break-all;
  margin-bottom: .65rem;
}
.mm-detail-url span { flex: 1; }

.mm-used-in { margin-top: .5rem; }
.mm-used-in-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: .04em;
  margin-bottom: .4rem;
}
.mm-used-in-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: .3rem; }
.mm-used-in-list li a {
  font-size: 12px;
  color: var(--primary);
  text-decoration: none;
  display: flex;
  align-items: center;
  gap: .35rem;
}
.mm-used-in-list li a:hover { text-decoration: underline; }

.mm-detail-actions {
  display: flex;
  flex-direction: column;
  gap: .5rem;
  padding: .85rem;
  border-top: 1px solid var(--border);
  flex-shrink: 0;
}

/* ── Bulk action bar (floating) ─────────────────────────────────────── */
.mm-bulk-bar {
  position: absolute;
  bottom: 1rem;
  left: 50%;
  transform: translateX(-50%) translateY(80px);
  background: #1e293b;
  color: #f8fafc;
  border-radius: 10px;
  padding: .6rem 1rem;
  display: flex;
  align-items: center;
  gap: .75rem;
  box-shadow: 0 8px 32px rgba(0,0,0,.25);
  transition: transform .22s cubic-bezier(.4,0,.2,1);
  z-index: 20;
  white-space: nowrap;
  font-size: 13px;
}
.mm-bulk-bar.visible { transform: translateX(-50%) translateY(0); }
.mm-bulk-bar-count { font-weight: 700; color: #a5b4fc; }
.mm-bulk-bar-sep { width: 1px; height: 18px; background: #334155; }
.mm-bulk-action {
  background: none;
  border: none;
  color: #cbd5e1;
  cursor: pointer;
  font-size: 13px;
  padding: .2rem .4rem;
  border-radius: 5px;
  font-family: inherit;
  transition: background .15s, color .15s;
}
.mm-bulk-action:hover { background: #334155; color: #f8fafc; }
.mm-bulk-action.danger:hover { background: #7f1d1d; color: #fca5a5; }
.mm-bulk-dismiss {
  background: none;
  border: none;
  color: #64748b;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  padding: .1rem .25rem;
  border-radius: 4px;
}
.mm-bulk-dismiss:hover { color: #f8fafc; }

/* ── Pagination ─────────────────────────────────────────────────────── */
.mm-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: .55rem 1rem;
  border-top: 1px solid var(--border);
  background: #f8fafc;
  flex-shrink: 0;
  flex-wrap: wrap;
  gap: .5rem;
}
.mm-footer-info { font-size: 12px; color: var(--muted); }

/* ── Responsive ─────────────────────────────────────────────────────── */
@media (max-width: 900px) {
  .mm-layout {
    grid-template-columns: 1fr;
    height: auto;
    min-height: 0;
  }
  .mm-sidebar {
    border-right: none;
    border-bottom: 1px solid var(--border);
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .mm-sidebar .mm-panel-section + .mm-panel-section {
    border-top: none;
    border-left: 1px solid var(--border);
  }
  .mm-new-folder-btn { display: none; }
  .mm-main { min-height: 500px; }
  .mm-detail-panel {
    top: 0;
    width: 100%;
    height: 100%;
    border-left: none;
    border-top: 1px solid var(--border);
  }
}

@media (max-width: 600px) {
  .mm-toolbar { gap: .4rem; }
  .mm-search { min-width: 120px; }
  .mm-sort { display: none; }
  .mm-sidebar { grid-template-columns: 1fr; }
  .mm-sidebar .mm-panel-section + .mm-panel-section { border-left: none; border-top: 1px solid var(--border); }
  .mm-grid { grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); }
  .mm-dropzone-text span { display: none; }
}
</style>

<div class="mm-layout" id="mmLayout">

  <!-- ── Toolbar ──────────────────────────────────────────────────────── -->
  <div class="mm-toolbar">
    <div class="mm-toolbar-left">
      <div class="mm-search">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input type="text" placeholder="Search files…" id="mmSearch">
      </div>
      <select class="mm-sort">
        <option>Newest first</option>
        <option>Oldest first</option>
        <option>Name A–Z</option>
        <option>Name Z–A</option>
        <option>Largest</option>
        <option>Smallest</option>
      </select>
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

  <!-- ── Left sidebar ─────────────────────────────────────────────────── -->
  <div class="mm-sidebar">
    <div class="mm-panel-section">
      <div class="mm-panel-label">Filter by type</div>
      <ul class="mm-type-list">
        <li class="mm-type-item">
          <a href="#" class="active">
            <span class="mm-type-dot" style="background:#4f46e5"></span>
            All files
            <span class="mm-type-count">148</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#">
            <span class="mm-type-dot" style="background:#10b981"></span>
            Images
            <span class="mm-type-count">112</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#">
            <span class="mm-type-dot" style="background:#f59e0b"></span>
            Video
            <span class="mm-type-count">9</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#">
            <span class="mm-type-dot" style="background:#8b5cf6"></span>
            Audio
            <span class="mm-type-count">4</span>
          </a>
        </li>
        <li class="mm-type-item">
          <a href="#">
            <span class="mm-type-dot" style="background:#64748b"></span>
            Documents
            <span class="mm-type-count">23</span>
          </a>
        </li>
      </ul>
    </div>

    <div class="mm-panel-section" style="flex:1">
      <div class="mm-panel-label">Folders</div>
      <ul class="mm-folder-list">
        <li class="mm-folder-item">
          <a href="#" class="active">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            All media
          </a>
        </li>
        <li class="mm-folder-item">
          <a href="#">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            Blog images
          </a>
        </li>
        <li class="mm-folder-item">
          <a href="#">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            Logos &amp; branding
          </a>
        </li>
        <li class="mm-folder-item">
          <a href="#">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            Team photos
          </a>
        </li>
        <li class="mm-folder-item">
          <a href="#">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            Downloads
          </a>
        </li>
      </ul>
      <button class="btn btn-secondary mm-new-folder-btn">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        New folder
      </button>
    </div>
  </div>

  <!-- ── Main content ─────────────────────────────────────────────────── -->
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

    <!-- Scrollable grid/list wrap -->
    <div class="mm-grid-wrap" id="mmGridWrap">

      <!-- Grid view -->
      <div class="mm-grid" id="mmGrid">

        <!-- Image items -->
        <div class="mm-item selected" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/a1/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">hero-banner.jpg</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/b2/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">team-photo-2024.jpg</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/c3/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">product-shot.png</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/d4/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">office-interior.jpg</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/e5/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">logo-white.png</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/f6/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">blog-cover-01.jpg</span>
          </div>
        </div>

        <!-- PDF document -->
        <div class="mm-item" onclick="selectItem(this, 'pdf')">
          <div class="mm-item-check"></div>
          <div class="mm-item-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="9" y1="13" x2="15" y2="13"/><line x1="9" y1="17" x2="12" y2="17"/></svg>
            <span style="color:#ef4444">PDF</span>
          </div>
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#ef4444"></span>
            <span class="mm-item-bar-name">annual-report.pdf</span>
          </div>
        </div>

        <!-- Video -->
        <div class="mm-item" onclick="selectItem(this, 'video')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/g7/200/200" alt="" style="filter:brightness(.7)">
          <div style="position:absolute;inset:0;display:flex;align-items:center;justify-content:center;pointer-events:none">
            <div style="width:36px;height:36px;border-radius:50%;background:rgba(255,255,255,.85);display:flex;align-items:center;justify-content:center">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="#1e293b"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            </div>
          </div>
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#f59e0b"></span>
            <span class="mm-item-bar-name">intro-video.mp4</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/h8/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">service-card.jpg</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/i9/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">about-page-hero.png</span>
          </div>
        </div>

        <!-- ZIP / archive -->
        <div class="mm-item" onclick="selectItem(this, 'zip')">
          <div class="mm-item-check"></div>
          <div class="mm-item-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="#8b5cf6" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
            <span style="color:#8b5cf6">ZIP</span>
          </div>
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#8b5cf6"></span>
            <span class="mm-item-bar-name">assets-export.zip</span>
          </div>
        </div>

        <div class="mm-item" onclick="selectItem(this, 'img')">
          <div class="mm-item-check"></div>
          <img src="https://picsum.photos/seed/j10/200/200" alt="">
          <div class="mm-item-bar">
            <span class="mm-item-type-dot" style="background:#10b981"></span>
            <span class="mm-item-bar-name">gallery-03.jpg</span>
          </div>
        </div>

      </div>

      <!-- List view (hidden by default) -->
      <table class="mm-list" id="mmList">
        <thead>
          <tr>
            <th style="width:32px"></th>
            <th style="width:52px"></th>
            <th>Filename</th>
            <th>Type</th>
            <th>Size</th>
            <th>Dimensions</th>
            <th>Uploaded</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr class="selected" onclick="selectItem(this, 'img')">
            <td><input type="checkbox" checked></td>
            <td><img class="mm-list-thumb" src="https://picsum.photos/seed/a1/80/80" alt=""></td>
            <td><strong style="font-size:13px">hero-banner.jpg</strong></td>
            <td><span class="mm-list-type-pill" style="background:#d1fae5;color:#065f46">IMAGE</span></td>
            <td style="color:var(--muted)">2.4 MB</td>
            <td style="color:var(--muted)">1920×1080</td>
            <td style="color:var(--muted)">Mar 10, 2026</td>
            <td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem">Edit</button></td>
          </tr>
          <tr onclick="selectItem(this, 'img')">
            <td><input type="checkbox"></td>
            <td><img class="mm-list-thumb" src="https://picsum.photos/seed/b2/80/80" alt=""></td>
            <td><strong style="font-size:13px">team-photo-2024.jpg</strong></td>
            <td><span class="mm-list-type-pill" style="background:#d1fae5;color:#065f46">IMAGE</span></td>
            <td style="color:var(--muted)">1.1 MB</td>
            <td style="color:var(--muted)">1200×800</td>
            <td style="color:var(--muted)">Feb 28, 2026</td>
            <td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem">Edit</button></td>
          </tr>
          <tr onclick="selectItem(this, 'pdf')">
            <td><input type="checkbox"></td>
            <td>
              <div class="mm-list-thumb" style="display:flex;align-items:center;justify-content:center;background:#fee2e2">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
              </div>
            </td>
            <td><strong style="font-size:13px">annual-report.pdf</strong></td>
            <td><span class="mm-list-type-pill" style="background:#fee2e2;color:#991b1b">PDF</span></td>
            <td style="color:var(--muted)">4.8 MB</td>
            <td style="color:var(--muted)">—</td>
            <td style="color:var(--muted)">Jan 15, 2026</td>
            <td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem">Edit</button></td>
          </tr>
          <tr onclick="selectItem(this, 'video')">
            <td><input type="checkbox"></td>
            <td><img class="mm-list-thumb" src="https://picsum.photos/seed/g7/80/80" alt="" style="filter:brightness(.75)"></td>
            <td><strong style="font-size:13px">intro-video.mp4</strong></td>
            <td><span class="mm-list-type-pill" style="background:#fef3c7;color:#92400e">VIDEO</span></td>
            <td style="color:var(--muted)">18.2 MB</td>
            <td style="color:var(--muted)">1920×1080</td>
            <td style="color:var(--muted)">Dec 5, 2025</td>
            <td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem">Edit</button></td>
          </tr>
        </tbody>
      </table>

    </div>

    <!-- Footer / pagination -->
    <div class="mm-footer">
      <span class="mm-footer-info">Showing 1–24 of 148 files</span>
      <div class="pagination" style="margin:0">
        <span class="page-btn page-btn-disabled">&lsaquo; Prev</span>
        <span class="page-btn page-btn-active">1</span>
        <a href="#" class="page-btn">2</a>
        <a href="#" class="page-btn">3</a>
        <span class="page-btn" style="color:var(--muted);pointer-events:none">…</span>
        <a href="#" class="page-btn">7</a>
        <a href="#" class="page-btn">Next &rsaquo;</a>
      </div>
    </div>
  </div>

  <!-- ── Detail panel ──────────────────────────────────────────────────── -->
  <div class="mm-detail-panel" id="mmDetail">
    <div class="mm-detail-header">
      <span>File details</span>
      <button class="mm-detail-close" onclick="closeDetail()" title="Close">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>

    <div class="mm-detail-body">
      <img class="mm-detail-preview" src="https://picsum.photos/seed/a1/400/300" alt="" id="mmDetailImg">
      <div class="mm-detail-meta">
        <div class="mm-detail-filename">hero-banner.jpg</div>
        <div class="mm-detail-stats">
          <span class="mm-detail-stat-label">Type</span>  <span class="mm-detail-stat-value">JPEG image</span>
          <span class="mm-detail-stat-label">Size</span>  <span class="mm-detail-stat-value">2.4 MB</span>
          <span class="mm-detail-stat-label">Dims</span>  <span class="mm-detail-stat-value">1920×1080</span>
          <span class="mm-detail-stat-label">Added</span> <span class="mm-detail-stat-value">Mar 10</span>
        </div>
      </div>

      <div class="mm-detail-url">
        <span>/uploads/2026/03/hero-banner.jpg</span>
        <button class="btn-link" style="font-size:12px;flex-shrink:0" onclick="navigator.clipboard.writeText('/uploads/2026/03/hero-banner.jpg')">Copy</button>
      </div>

      <div class="mm-detail-field">
        <label>Alt text</label>
        <input type="text" placeholder="Describe the image for screen readers…" value="Hero banner showing the company office">
      </div>
      <div class="mm-detail-field">
        <label>Caption</label>
        <textarea placeholder="Optional caption shown below the image…"></textarea>
      </div>
      <div class="mm-detail-field">
        <label>Title</label>
        <input type="text" value="Hero Banner 2024">
      </div>

      <div class="mm-used-in">
        <div class="mm-used-in-label">Used in</div>
        <ul class="mm-used-in-list">
          <li>
            <a href="#">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
              Home page (featured image)
            </a>
          </li>
          <li>
            <a href="#">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
              About Us (body content)
            </a>
          </li>
        </ul>
      </div>
    </div>

    <div class="mm-detail-actions">
      <button class="btn btn-primary" style="width:100%;justify-content:center">Save changes</button>
      <button class="btn btn-danger" style="width:100%;justify-content:center">Delete file</button>
    </div>
  </div>

  <!-- ── Floating bulk action bar ──────────────────────────────────────── -->
  <div class="mm-bulk-bar" id="mmBulkBar">
    <span class="mm-bulk-bar-count" id="mmBulkCount">1 selected</span>
    <div class="mm-bulk-bar-sep"></div>
    <button class="mm-bulk-action" onclick="alert('Move to folder — not wired yet')">Move to…</button>
    <button class="mm-bulk-action" onclick="alert('Download — not wired yet')">Download</button>
    <button class="mm-bulk-action danger" onclick="alert('Delete — not wired yet')">Delete</button>
    <div class="mm-bulk-bar-sep"></div>
    <button class="mm-bulk-dismiss" onclick="clearSelection()" title="Clear selection">&#10005;</button>
  </div>

</div>

<script>
(function() {
  var selected = new Set();
  var bulkMode = false;
  var detailOpen = false;

  // Open detail panel on page load (first item is pre-selected)
  document.getElementById('mmDetail').classList.add('open');
  selected.add('item0');
  detailOpen = true;
  syncBulkBar();

  window.setView = function(v) {
    var wrap = document.getElementById('mmMain');
    if (v === 'list') {
      wrap.classList.add('mm-view-list');
      document.getElementById('viewGrid').classList.remove('active');
      document.getElementById('viewList').classList.add('active');
    } else {
      wrap.classList.remove('mm-view-list');
      document.getElementById('viewGrid').classList.add('active');
      document.getElementById('viewList').classList.remove('active');
    }
  };

  window.toggleBulkMode = function() {
    bulkMode = !bulkMode;
    var layout = document.getElementById('mmLayout');
    var btn = document.getElementById('mmBulkToggle');
    if (bulkMode) {
      layout.classList.add('mm-bulk-mode');
      btn.textContent = 'Done';
    } else {
      layout.classList.remove('mm-bulk-mode');
      btn.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right:.3rem"><rect x="3" y="5" width="4" height="4" rx="1"/><rect x="3" y="12" width="4" height="4" rx="1"/><rect x="3" y="19" width="4" height="4" rx="1"/><line x1="10" y1="7" x2="21" y2="7"/><line x1="10" y1="14" x2="21" y2="14"/><line x1="10" y1="21" x2="21" y2="21"/></svg>Select';
    }
  };

  window.selectItem = function(el, type) {
    var items = document.querySelectorAll('.mm-item');
    var idx = Array.from(items).indexOf(el);
    var key = 'item' + idx;

    if (el.classList.contains('selected')) {
      el.classList.remove('selected');
      selected.delete(key);
    } else {
      if (!bulkMode) {
        // single select: deselect all others
        items.forEach(function(i, n) { i.classList.remove('selected'); selected.delete('item' + n); });
      }
      el.classList.add('selected');
      selected.add(key);
      if (!bulkMode) openDetail(type);
    }
    syncBulkBar();
  };

  function openDetail(type) {
    var panel = document.getElementById('mmDetail');
    if (type === 'img') {
      document.getElementById('mmDetailImg').style.display = 'block';
    }
    panel.classList.add('open');
    detailOpen = true;
  }

  window.closeDetail = function() {
    document.getElementById('mmDetail').classList.remove('open');
    detailOpen = false;
    var items = document.querySelectorAll('.mm-item');
    items.forEach(function(i, n) { i.classList.remove('selected'); selected.delete('item' + n); });
    syncBulkBar();
  };

  window.clearSelection = function() {
    var items = document.querySelectorAll('.mm-item');
    items.forEach(function(i, n) { i.classList.remove('selected'); selected.delete('item' + n); });
    syncBulkBar();
    if (detailOpen) {
      document.getElementById('mmDetail').classList.remove('open');
      detailOpen = false;
    }
  };

  function syncBulkBar() {
    var bar = document.getElementById('mmBulkBar');
    var count = selected.size;
    document.getElementById('mmBulkCount').textContent = count + (count === 1 ? ' selected' : ' selected');
    if (count > 0) {
      bar.classList.add('visible');
    } else {
      bar.classList.remove('visible');
    }
  }

  // Drag-over styling on drop zone
  var dz = document.getElementById('mmDropzone');
  ['dragover','dragenter'].forEach(function(ev) {
    dz.addEventListener(ev, function(e) { e.preventDefault(); dz.classList.add('drag-over'); });
  });
  ['dragleave','drop'].forEach(function(ev) {
    dz.addEventListener(ev, function(e) { e.preventDefault(); dz.classList.remove('drag-over'); });
  });
})();
</script>
"##;

    admin_page("Media 2", "/admin/media2", None, content, ctx)
}
