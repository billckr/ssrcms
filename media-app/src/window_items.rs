//! Keeps `window.ITEMS`/`window.FOLDERS` (globals the legacy detail-panel/
//! bulk-action JS in media.rs's inline `<script>` reads — `ITEMS[idx]`,
//! `FOLDERS.forEach(...)`) in sync with whatever the WASM island just
//! fetched. That JS is untouched — it just reads whatever is sitting at
//! `window.ITEMS`/`window.FOLDERS`, indexed to match the DOM order the
//! island produces. Critically, the legacy script assigns to these via
//! `window.ITEMS = ...` (not `var ITEMS = ...`) specifically so it keeps
//! reading the live, WASM-updated value rather than a frozen page-load copy.

use crate::types::{GridFolder, GridItem};

pub fn sync_items(items: &[GridItem]) {
    let Some(window) = web_sys::window() else { return };
    let arr = js_sys::Array::new();
    for it in items {
        let obj = js_sys::Object::new();
        let set = |key: &str, val: wasm_bindgen::JsValue| {
            let _ = js_sys::Reflect::set(&obj, &wasm_bindgen::JsValue::from_str(key), &val);
        };
        set("id", it.id.clone().into());
        set("filename", it.filename.clone().into());
        set("type", it.type_key.clone().into());
        set("isImage", it.is_image.into());
        // Legacy JS (openDetail's preview <img>, bulkDownload's href) expects
        // this prefix baked in, matching what the original SSR-embedded
        // items_json always did — the grid thumbnails add "/uploads/"
        // themselves in Rust, but the detail panel does not.
        set("path", format!("/uploads/{}", it.path).into());
        set("alt", it.alt.clone().into());
        set("title", it.title.clone().into());
        set("caption", it.caption.clone().into());
        set("size", it.size.clone().into());
        set("dims", it.dims.clone().into());
        set("uploader", it.uploader.clone().into());
        set("uploaded_at", it.uploaded_at.clone().into());
        arr.push(&obj);
    }
    let _ = js_sys::Reflect::set(&window, &wasm_bindgen::JsValue::from_str("ITEMS"), &arr);
}

pub fn sync_folders(folders: &[GridFolder]) {
    let Some(window) = web_sys::window() else { return };
    let arr = js_sys::Array::new();
    for f in folders {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &wasm_bindgen::JsValue::from_str("id"), &f.id.clone().into());
        let _ = js_sys::Reflect::set(&obj, &wasm_bindgen::JsValue::from_str("name"), &f.name.clone().into());
        arr.push(&obj);
    }
    let _ = js_sys::Reflect::set(&window, &wasm_bindgen::JsValue::from_str("FOLDERS"), &arr);
}
