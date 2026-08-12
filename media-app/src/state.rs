use leptos::prelude::*;
use crate::types::GridResponse;

/// Filter/pagination + fetched-data signals shared across the three
/// independently-mounted regions (sidebar, toolbar, main content) so a
/// change in one (e.g. clicking a type tab) is reflected in the others
/// (the grid re-fetches; the sidebar's active-tab styling updates).
#[derive(Copy, Clone)]
pub struct SharedState {
    pub folder_id: RwSignal<Option<String>>,
    pub type_filter: RwSignal<Option<String>>,
    pub page: RwSignal<i64>,
    pub grid: RwSignal<Option<GridResponse>>,
    pub loading: RwSignal<bool>,
    pub show_new_folder_modal: RwSignal<bool>,
    pub new_folder_name: RwSignal<String>,
    pub new_folder_error: RwSignal<Option<String>>,
    pub show_delete_folder_modal: RwSignal<bool>,
    pub delete_folder_error: RwSignal<Option<String>>,
}

thread_local! {
    static STATE: SharedState = SharedState {
        folder_id: RwSignal::new(None),
        type_filter: RwSignal::new(None),
        page: RwSignal::new(1),
        grid: RwSignal::new(None),
        loading: RwSignal::new(false),
        show_new_folder_modal: RwSignal::new(false),
        new_folder_name: RwSignal::new(String::new()),
        new_folder_error: RwSignal::new(None),
        show_delete_folder_modal: RwSignal::new(false),
        delete_folder_error: RwSignal::new(None),
    };
}

pub fn state() -> SharedState {
    STATE.with(|s| *s)
}

/// Fetches the grid for the current filter/page signals and writes the
/// result into `grid` (plus syncs the `window.ITEMS`/`window.FOLDERS`
/// globals the legacy JS reads). Shared by `refresh()` (background,
/// spawned) and `initial_load()` (awaited directly, before anything mounts).
async fn fetch_and_apply() {
    let s = state();
    let folder = s.folder_id.get_untracked();
    let type_f = s.type_filter.get_untracked();
    let page = s.page.get_untracked();
    match crate::api::fetch_grid(folder.as_deref(), type_f.as_deref(), page).await {
        Ok(data) => {
            crate::window_items::sync_items(&data.items);
            crate::window_items::sync_folders(&data.folders);
            s.grid.set(Some(data));
        }
        Err(e) => {
            leptos::logging::error!("media-app: grid fetch failed: {e}");
        }
    }
}

/// Re-fetch the grid in the background. Called whenever a filter/page
/// setter runs, or after a bulk action completes.
pub fn refresh() {
    let s = state();
    s.loading.set(true);
    leptos::task::spawn_local(async move {
        fetch_and_apply().await;
        s.loading.set(false);
    });
}

/// Awaited directly by `mount()` before any component is mounted, so the
/// island's very first paint already has real data instead of an empty
/// placeholder that a moment later gets replaced once a background fetch
/// resolves — avoids a visible flash of empty content between the SSR
/// fallback being cleared and the fetch completing.
pub async fn initial_load() {
    fetch_and_apply().await;
}

pub fn set_type_filter(t: Option<String>) {
    let s = state();
    s.type_filter.set(t);
    s.page.set(1);
    refresh();
}

pub fn set_folder(f: Option<String>) {
    let s = state();
    s.folder_id.set(f);
    s.page.set(1);
    refresh();
}

pub fn set_page(p: i64) {
    state().page.set(p);
    refresh();
}

/// Resets to a clean "All Media" view scoped to `type_filter`, then does a
/// single refresh. Called when the parent page re-opens an already-warm
/// picker iframe (see `mount_into`'s docs / lib.rs) instead of reloading
/// the whole page — replaces what a fresh page load used to give for free
/// (starting from a known filter/folder state, fresh data), without paying
/// the cost of re-running the entire WASM bootstrap on every open. Setting
/// all three signals before the one `refresh()` call (rather than reusing
/// `set_folder`/`set_type_filter`, which each trigger their own refresh)
/// avoids firing two redundant fetches back to back.
pub fn reset_for_picker(type_filter: Option<String>) {
    let s = state();
    s.folder_id.set(None);
    s.type_filter.set(type_filter);
    s.page.set(1);
    refresh();
}

/// Mirrors the server's own sanitization in `create_folder` (media.rs):
/// ASCII alphanumerics/hyphens only, capped at 25 chars, edges trimmed.
/// Client-side, this only gates the Create button's disabled state — the
/// server re-validates regardless.
pub fn sanitize_folder_name(raw: &str) -> String {
    let filtered: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(25).collect();
    filtered.trim_matches('-').to_string()
}

pub fn open_new_folder_modal() {
    let s = state();
    s.new_folder_name.set(String::new());
    s.new_folder_error.set(None);
    s.show_new_folder_modal.set(true);
}

pub fn submit_new_folder() {
    let s = state();
    let name = sanitize_folder_name(&s.new_folder_name.get_untracked());
    if name.len() < 4 {
        return;
    }
    leptos::task::spawn_local(async move {
        match crate::api::create_folder(&name).await {
            Ok(()) => {
                s.show_new_folder_modal.set(false);
                refresh();
            }
            Err(e) => {
                leptos::logging::error!("media-app: create_folder failed: {e}");
                s.new_folder_error.set(Some("Could not create folder. Please try again.".to_string()));
            }
        }
    });
}

pub fn open_delete_folder_modal() {
    let s = state();
    s.delete_folder_error.set(None);
    s.show_delete_folder_modal.set(true);
}

pub fn confirm_delete_folder(delete_media: bool) {
    let s = state();
    let Some(folder_id) = s.folder_id.get_untracked() else { return };
    leptos::task::spawn_local(async move {
        match crate::api::delete_folder(&folder_id, delete_media).await {
            Ok(()) => {
                s.show_delete_folder_modal.set(false);
                // The deleted folder was necessarily the active one (that's
                // the only time this button/modal is reachable) — switch
                // back to All Media, which also refreshes.
                set_folder(None);
            }
            Err(e) => {
                leptos::logging::error!("media-app: delete_folder failed: {e}");
                s.delete_folder_error.set(Some("Could not delete folder. Please try again.".to_string()));
            }
        }
    });
}
