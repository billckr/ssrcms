mod api;
mod components;
mod state;
mod types;
mod upload;
mod window_items;

use components::{ContentGrid, DeleteFolderButton, FolderSelect, FooterInfo, Pagination, Toolbar, TypeTabs};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

/// Mounts the three independent regions (sidebar filters, toolbar upload
/// dropzone, main grid/pagination) into the existing SSR-rendered page,
/// replacing their static contents. All three share `state::state()`, so a
/// change in one is reflected in the others without any extra wiring.
/// Called once from a small bootstrap `<script type="module">` in the
/// media library page after the wasm module loads.
#[wasm_bindgen]
pub fn mount() {
    console_error_panic_hook::set_once();

    mount_into("mm-type-tabs-app", || view! { <TypeTabs /> });
    mount_into("mm-folder-select-app", || view! { <FolderSelect /> });
    mount_into("mm-delete-folder-app", || view! { <DeleteFolderButton /> });
    mount_into("mm-toolbar-app", || view! { <Toolbar /> });
    mount_into("mmGridWrap", || view! { <ContentGrid /> });
    mount_into("mmPagination", || view! { <Pagination /> });
    mount_into("mmFooterInfo", || view! { <FooterInfo /> });

    state::refresh();
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
