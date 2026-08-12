use leptos::prelude::*;
use leptos::web_sys;
use wasm_bindgen::JsCast;
use crate::state::{self, state};
use crate::types::GridItem;

fn type_key_label(k: &str) -> &'static str {
    match k {
        "image" => "IMAGE",
        "video" => "VIDEO",
        "audio" => "AUDIO",
        _ => "DOC",
    }
}

// ── Type filter tabs (mounted inside the existing <ul class="mm-type-list">) ──

#[component]
pub fn TypeTabs() -> impl IntoView {
    let s = state();

    let tab = move |key: Option<&'static str>, label: &'static str, count: i64| {
        let active = move || s.type_filter.get().as_deref() == key;
        view! {
            <li class="mm-type-item">
                <a
                    href="#"
                    class:active=active
                    on:click=move |ev| {
                        ev.prevent_default();
                        state::set_type_filter(key.map(|k| k.to_string()));
                    }
                >
                    {label}
                    <span class="mm-type-count">{count}</span>
                </a>
            </li>
        }
    };

    view! {
        {move || {
            let tc = s.grid.get().map(|g| g.type_counts).unwrap_or_default();
            vec![
                tab(None, "All files", tc.all),
                tab(Some("image"), "Images", tc.image),
                tab(Some("video"), "Video", tc.video),
                tab(Some("audio"), "Audio", tc.audio),
                tab(Some("document"), "Documents", tc.document),
            ]
        }}
    }
}

// ── Folder select (mounted in place of the existing <select>) ─────────────

#[component]
pub fn FolderSelect() -> impl IntoView {
    let s = state();

    view! {
        <select
            class="mm-folder-select"
            on:change=move |ev| {
                let v = event_target_value(&ev);
                state::set_folder(if v.is_empty() { None } else { Some(v) });
            }
        >
            <option value="">"All Media"</option>
            {move || {
                let mut folders = s.grid.get().map(|g| g.folders).unwrap_or_default();
                folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                let current = s.folder_id.get();
                folders.into_iter().map(|f| {
                    let selected = current.as_deref() == Some(f.id.as_str());
                    view! {
                        <option value=f.id.clone() selected=selected>{f.name}</option>
                    }
                }).collect::<Vec<_>>()
            }}
        </select>
    }
}

/// Mounted inside `#mm-delete-folder-app` — only rendered when a folder is
/// selected, same as the old server-rendered version, but reactive to the
/// island's own `folder_id` signal instead of the page-load query string
/// (which never changes now that folder switching happens client-side).
#[component]
pub fn DeleteFolderButton() -> impl IntoView {
    let s = state();
    view! {
        {move || {
            s.folder_id.get().map(|_fid| {
                view! {
                    <div class="icon-pill" style="margin-top:.3rem">
                        <button class="icon-btn icon-danger" title="Delete folder" aria-label="Delete folder"
                            on:click=move |_| state::open_delete_folder_modal()>
                            <img src="/admin/static/icons/trash.svg" alt="" />
                        </button>
                    </div>
                }
            })
        }}
    }
}

/// Mounted in place of the old "Folder +" button — just opens the modal
/// below (its own separate mount point, sharing state).
#[component]
pub fn NewFolderButton() -> impl IntoView {
    view! {
        <div class="icon-pill" style="margin-top:0">
            <button class="icon-btn" title="New folder" aria-label="New folder" on:click=move |_| state::open_new_folder_modal()>
                <img src="/admin/static/icons/folder-plus.svg" alt="" />
            </button>
        </div>
    }
}

/// Mounted inside `#mm-new-folder-modal-app`. Renders nothing when closed —
/// unlike the other mount points, this one owns its own visibility rather
/// than toggling a pre-existing SSR wrapper's display style, since the
/// modal only ever needs to exist once JS has taken over anyway.
#[component]
pub fn NewFolderModal() -> impl IntoView {
    let s = state();
    view! {
        {move || {
            s.show_new_folder_modal.get().then(|| view! {
                <div style="position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:200;display:flex;align-items:center;justify-content:center">
                    <div class="modal-card" style="max-width:360px;width:90%">
                        <h3 class="modal-card-header">"New folder"</h3>
                        <div class="modal-card-body">
                            <div class="form-group" style="margin-bottom:.35rem">
                                <input type="text" placeholder="Folder name" maxlength="25"
                                    style="width:100%;padding:.35rem .6rem;border:1px solid var(--border);border-radius:var(--radius);font-size:14px;background:var(--field-bg);color:var(--field-text);box-sizing:border-box"
                                    prop:value=move || s.new_folder_name.get()
                                    on:input=move |ev| s.new_folder_name.set(event_target_value(&ev))
                                    on:keydown=move |ev| { if ev.key() == "Enter" { ev.prevent_default(); state::submit_new_folder(); } }
                                />
                            </div>
                            <p style="font-size:12px;color:var(--muted);margin:0 0 1rem">
                                "4\u{2013}25 characters: letters, numbers, and hyphens only."
                            </p>
                            {move || s.new_folder_error.get().map(|e| view! {
                                <p style="font-size:13px;color:var(--danger);margin:-.5rem 0 1rem">{e}</p>
                            })}
                            <div class="icon-pill" style="margin-top:0;justify-content:flex-end">
                                <button class="icon-btn" title="Cancel" aria-label="Cancel" on:click=move |_| s.show_new_folder_modal.set(false)>
                                    <img src="/admin/static/icons/x.svg" alt="" />
                                </button>
                                <button class="icon-btn" title="Create" aria-label="Create"
                                    disabled=move || state::sanitize_folder_name(&s.new_folder_name.get()).len() < 4
                                    on:click=move |_| state::submit_new_folder()>
                                    <img src="/admin/static/icons/check.svg" alt="" />
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            })
        }}
    }
}

/// Mounted inside `#mm-delete-folder-modal-app`. Message/button layout
/// driven by the live `grid.total` for the currently-selected folder — the
/// legacy version read a `FOLDER_TOTAL` var frozen at page load, which went
/// stale the moment folder switching stopped reloading the page.
#[component]
pub fn DeleteFolderModal() -> impl IntoView {
    let s = state();
    view! {
        {move || {
            s.show_delete_folder_modal.get().then(|| {
                let total = s.grid.get().map(|g| g.total).unwrap_or(0);
                view! {
                    <div style="position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:200;display:flex;align-items:center;justify-content:center">
                        <div class="modal-card" style="max-width:400px;width:90%">
                            <h3 class="modal-card-header">"Delete folder"</h3>
                            <div class="modal-card-body">
                                <p style="font-size:14px;color:var(--muted);margin-bottom:1rem">
                                    {if total > 0 {
                                        format!("This folder contains {total} file(s). What would you like to do with them?")
                                    } else {
                                        "Are you sure you want to delete this empty folder?".to_string()
                                    }}
                                </p>
                                <div class="icon-pill" style="margin-top:0;justify-content:center">
                                    {(total > 0).then(|| view! {
                                        <button class="icon-btn" title="Move files to All Media, then delete folder" aria-label="Move files to All Media, then delete folder"
                                            on:click=move |_| state::confirm_delete_folder(false)>
                                            <img src="/admin/static/icons/folder-minus.svg" alt="" />
                                        </button>
                                    })}
                                    <button class="icon-btn icon-danger" title="Delete folder and all its files permanently" aria-label="Delete folder and all its files permanently"
                                        on:click=move |_| state::confirm_delete_folder(true)>
                                        <img src="/admin/static/icons/trash.svg" alt="" />
                                    </button>
                                    <button class="icon-btn" title="Cancel" aria-label="Cancel" on:click=move |_| s.show_delete_folder_modal.set(false)>
                                        <img src="/admin/static/icons/x.svg" alt="" />
                                    </button>
                                </div>
                                {move || s.delete_folder_error.get().map(|e| view! {
                                    <p style="font-size:13px;color:var(--danger);margin-top:.75rem">{e}</p>
                                })}
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}

// ── Toolbar: dropzone + hidden file input with real upload progress ───────

#[component]
pub fn Toolbar() -> impl IntoView {
    let s = state();
    let progress = RwSignal::new(None::<i32>);

    let do_upload = move |file: web_sys::File| {
        let folder_id = s.folder_id.get_untracked();
        progress.set(Some(0));
        crate::upload::upload_file(file, folder_id, progress);
    };

    let on_input_change = move |ev: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = ev.target().unwrap().unchecked_into();
        if let Some(files) = input.files() {
            if let Some(file) = files.get(0) {
                do_upload(file);
            }
        }
        input.set_value("");
    };

    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            if let Some(files) = dt.files() {
                if let Some(file) = files.get(0) {
                    do_upload(file);
                }
            }
        }
    };

    view! {
        <input
            type="file"
            id="mm2FileInput"
            accept="image/*,application/pdf,video/*,audio/*"
            style="position:absolute;width:1px;height:1px;opacity:0;overflow:hidden;pointer-events:none"
            on:change=on_input_change
        />
        <button
            type="button"
            class="icon-btn"
            title="Upload file"
            aria-label="Upload file"
            on:click=move |_| {
                if let Some(doc) = leptos::web_sys::window().and_then(|w| w.document()) {
                    if let Some(el) = doc.get_element_by_id("mm2FileInput") {
                        let _ = el.unchecked_into::<web_sys::HtmlElement>().click();
                    }
                }
            }
            on:dragover=move |ev: web_sys::DragEvent| ev.prevent_default()
            on:drop=on_drop
        >
            {move || match progress.get() {
                Some(p) if p < 100 => view! { <span style="font-size:11px">{format!("{p}%")}</span> }.into_any(),
                _ => view! { <img src="/admin/static/icons/upload.svg" alt="" /> }.into_any(),
            }}
        </button>
    }
}

// ── Main content: grid + list rows + pagination + footer ──────────────────

fn item_node(i: usize, m: &GridItem) -> impl IntoView {
    let onclick = "selectItem(this)";
    let thumb = if m.is_image {
        view! {
            <img src=format!("/uploads/{}", m.path) alt=m.alt.clone()
                style="width:100%;height:100%;object-fit:cover;display:block;pointer-events:none" />
        }.into_any()
    } else {
        view! {
            <div class="mm-item-icon">{type_key_label(&m.type_key)}</div>
        }.into_any()
    };
    view! {
        <div class="mm-item" data-idx=i data-type=m.type_key.clone()
            data-name=m.filename.to_lowercase() onclick=onclick>
            {thumb}
            <div class="mm-item-bar">
                <span class="mm-item-type-dot"></span>
                <span class="mm-item-bar-name">{m.filename.clone()}</span>
            </div>
            <div class="mm-item-check"></div>
        </div>
    }
}

fn list_row(i: usize, m: &GridItem) -> impl IntoView {
    let onclick = "selectItem(this)";
    let thumb = if m.is_image {
        view! { <img class="mm-list-thumb" src=format!("/uploads/{}", m.path) alt="" /> }.into_any()
    } else {
        view! { <div class="mm-list-thumb"></div> }.into_any()
    };
    view! {
        <tr data-idx=i data-type=m.type_key.clone() data-name=m.filename.to_lowercase() onclick=onclick>
            <td><input type="checkbox" onclick="event.stopPropagation();selectItem(this.closest('tr'))" /></td>
            <td>{thumb}</td>
            <td><strong style="font-size:13px">{m.filename.clone()}</strong></td>
            <td><span class="mm-list-type-pill">{type_key_label(&m.type_key)}</span></td>
            <td style="color:var(--muted)">{m.size.clone()}</td>
            <td style="color:var(--muted)">{m.dims.clone()}</td>
            <td><button class="btn btn-secondary" style="font-size:12px;padding:.2rem .5rem"
                onclick="event.stopPropagation();selectItem(this.closest('tr'))">"Edit"</button></td>
        </tr>
    }
}

/// Mounted inside the existing `#mmGridWrap` div (which keeps its id/class
/// in the SSR markup) — renders the grid tiles and list-view table as its
/// children, matching the original two-sibling structure so `.mm-view-list`
/// CSS toggling (via the pre-existing `setView()` JS) keeps working.
#[component]
pub fn ContentGrid() -> impl IntoView {
    let s = state();

    view! {
        <div class="mm-grid" id="mmGrid">
            {move || {
                s.grid.get().map(|g| {
                    g.items.iter().enumerate().map(|(i, m)| item_node(i, m)).collect::<Vec<_>>()
                }).unwrap_or_default()
            }}
        </div>
        <table class="mm-list" id="mmList">
            <thead>
                <tr>
                    <th style="width:32px"></th>
                    <th style="width:52px"></th>
                    <th>"Filename"</th>
                    <th>"Type"</th>
                    <th>"Size"</th>
                    <th>"Dimensions"</th>
                    <th></th>
                </tr>
            </thead>
            <tbody id="mmListBody">
                {move || {
                    s.grid.get().map(|g| {
                        g.items.iter().enumerate().map(|(i, m)| list_row(i, m)).collect::<Vec<_>>()
                    }).unwrap_or_default()
                }}
            </tbody>
        </table>
    }
}

/// Mounted inside the existing `#mmPagination` div.
#[component]
pub fn Pagination() -> impl IntoView {
    let page_btn = |n: i64, label: String| {
        let is_active = move || state().page.get() == n;
        view! {
            <a href="#" class="page-btn" class:page-btn-active=is_active
                on:click=move |ev| { ev.prevent_default(); state::set_page(n); }>
                {label}
            </a>
        }
    };

    view! {
        {move || {
            let s = state();
            let g = s.grid.get();
            let (page, total_pages) = g.as_ref().map(|g| (g.page, g.total_pages)).unwrap_or((1, 1));
            if total_pages <= 1 {
                return vec![];
            }
            let mut nodes = Vec::new();
            if page > 1 {
                nodes.push(page_btn(page - 1, "\u{2039} Prev".to_string()).into_any());
            }
            let start = (page - 3).max(1);
            let end = (page + 3).min(total_pages);
            for n in start..=end {
                nodes.push(page_btn(n, n.to_string()).into_any());
            }
            if page < total_pages {
                nodes.push(page_btn(page + 1, "Next \u{203a}".to_string()).into_any());
            }
            nodes
        }}
    }
}

/// Mounted inside the existing `#mmFooterInfo` span.
#[component]
pub fn FooterInfo() -> impl IntoView {
    let s = state();
    view! {
        {move || {
            s.grid.get().map(|g| {
                let from = if g.total == 0 { 0 } else { (g.page - 1) * g.page_size + 1 };
                let to = (g.page * g.page_size).min(g.total);
                format!("Showing {}\u{2013}{} of {} files", from, to, g.total)
            }).unwrap_or_default()
        }}
    }
}
