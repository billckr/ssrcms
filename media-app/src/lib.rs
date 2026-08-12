mod api;
mod components;
mod state;
mod types;
mod upload;
mod window_items;

use components::{
    ContentGrid, DeleteFolderButton, DeleteFolderModal, FolderSelect, FooterInfo, NewFolderButton,
    NewFolderModal, Pagination, Toolbar, TypeTabs,
};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

/// Mounts the independently-managed regions (sidebar filters, toolbar
/// upload dropzone, main grid/pagination, folder modals) into the existing
/// SSR-rendered page, replacing their static contents. All share
/// `state::state()`, so a change in one is reflected in the others without
/// any extra wiring. Called once from a small bootstrap `<script
/// type="module">` in the media library page after the wasm module loads.
///
/// Awaits the initial grid fetch *before* mounting anything, so the first
/// paint already has real data — otherwise each component would mount with
/// an empty `grid` signal, paint blank, then repaint a moment later once the
/// fetch resolved, producing a visible flash between the SSR fallback being
/// cleared and the real content appearing.
#[wasm_bindgen]
pub async fn mount() {
    console_error_panic_hook::set_once();
    state::initial_load().await;

    mount_into("mm-type-tabs-app", || view! { <TypeTabs /> });
    mount_into("mm-folder-select-app", || view! { <FolderSelect /> });
    mount_into("mm-delete-folder-app", || view! { <DeleteFolderButton /> });
    mount_into("mm-new-folder-btn-app", || view! { <NewFolderButton /> });
    mount_into("mm-new-folder-modal-app", || view! { <NewFolderModal /> });
    mount_into("mm-delete-folder-modal-app", || view! { <DeleteFolderModal /> });
    mount_into("mm-toolbar-app", || view! { <Toolbar /> });
    mount_into("mmGridWrap", || view! { <ContentGrid /> });
    mount_into("mmPagination", || view! { <Pagination /> });
    mount_into("mmFooterInfo", || view! { <FooterInfo /> });
}

/// Re-fetches the grid in place, without a page reload. Exported so the
/// legacy (non-module) inline `<script>` in media.rs — bulk move/delete,
/// which predates the island and can't `import` from an ES module — can
/// call it via `window.mediaAppRefresh()` after finishing its own fetches,
/// instead of falling back to `window.location.reload()`.
#[wasm_bindgen]
pub fn refresh_grid() {
    state::refresh();
}

/// Resets to a clean "All Media" view scoped to `type_filter` and refetches.
/// Exported so the parent page can reuse an already-warm picker iframe
/// instead of reloading it (and re-running the whole WASM bootstrap) on
/// every open — see `admin/src/lib.rs`'s `openMediaPicker`, which posts a
/// `resetPickerFilter` message that media.rs's inline `<script>` forwards
/// here via `window.mediaAppResetForPicker()`.
#[wasm_bindgen]
pub fn reset_for_picker(type_filter: Option<String>) {
    state::reset_for_picker(type_filter);
}

fn mount_into<F, V>(id: &str, view_fn: F)
where
    F: FnOnce() -> V + 'static,
    V: IntoView + 'static,
{
    let Some(window) = web_sys::window() else { return };
    let Some(doc) = window.document() else { return };
    let Some(el) = doc.get_element_by_id(id) else {
        leptos::logging::warn!("media-app: mount point #{id} not found");
        return;
    };
    // Clear the SSR fallback content — the island takes over from here.
    el.set_inner_html("");
    leptos::mount::mount_to(el.unchecked_into(), view_fn).forget();
}
