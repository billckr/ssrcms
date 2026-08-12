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
}

thread_local! {
    static STATE: SharedState = SharedState {
        folder_id: RwSignal::new(None),
        type_filter: RwSignal::new(None),
        page: RwSignal::new(1),
        grid: RwSignal::new(None),
        loading: RwSignal::new(false),
    };
}

pub fn state() -> SharedState {
    STATE.with(|s| *s)
}

/// Re-fetch the grid using the current filter/page signals and write the
/// result into `grid`. Called on init and whenever a filter/page setter runs.
pub fn refresh() {
    let s = state();
    s.loading.set(true);
    leptos::task::spawn_local(async move {
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
        s.loading.set(false);
    });
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
