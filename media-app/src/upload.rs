//! Multipart upload via XHR (not `fetch`) specifically because `fetch` has
//! no upload-progress event — `xhr.upload.onprogress` is the only way to
//! show a real progress percentage for large files.

use leptos::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{FormData, ProgressEvent, XmlHttpRequest};

pub fn upload_file(file: web_sys::File, folder_id: Option<String>, progress: RwSignal<Option<i32>>) {
    let form = FormData::new().unwrap();
    let _ = form.append_with_str("redirect", "");
    if let Some(fid) = &folder_id {
        let _ = form.append_with_str("folder_id", fid);
    }
    let _ = form.append_with_blob("file", &file);

    let xhr = XmlHttpRequest::new().unwrap();
    xhr.open("POST", "/admin/media/upload").unwrap();

    let upload = xhr.upload().unwrap();
    let onprogress = Closure::<dyn FnMut(ProgressEvent)>::new(move |ev: ProgressEvent| {
        if ev.length_computable() {
            let pct = ((ev.loaded() / ev.total()) * 100.0) as i32;
            progress.set(Some(pct.min(99)));
        }
    });
    upload.set_onprogress(Some(onprogress.as_ref().unchecked_ref()));
    onprogress.forget();

    let xhr_clone = xhr.clone();
    let onloadend = Closure::<dyn FnMut()>::new(move || {
        progress.set(None);
        if xhr_clone.status().unwrap_or(0) < 400 {
            crate::state::refresh();
        } else {
            leptos::logging::error!("media-app: upload failed, status {:?}", xhr_clone.status());
        }
    });
    xhr.set_onloadend(Some(onloadend.as_ref().unchecked_ref()));
    onloadend.forget();

    let _ = xhr.send_with_opt_form_data(Some(&form));
    let _: JsValue = JsValue::UNDEFINED; // keep types in scope for clarity
}
