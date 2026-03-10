//! Multipart file upload handler for admin media.

use axum::{
    extract::{Multipart, State},
    response::{IntoResponse, Redirect},
};
use std::path::Path;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::media::CreateMedia;
use super::sanitize_media_text;

/// Convert an arbitrary filename stem into a URL-safe slug.
/// e.g. "My Photo (2026)!" → "my-photo-2026"
fn slugify_name(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive hyphens and trim edges.
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = true; // true = skip leading hyphens
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen { result.push(c); }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen.
    if result.ends_with('-') { result.pop(); }
    if result.is_empty() { result.push_str("upload"); }
    result
}

pub async fn upload(
    State(state): State<AppState>,
    admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_data: Option<(String, String, Vec<u8>)> = None; // (filename, mime, bytes)
    let mut alt_text: Option<String> = None;
    let mut folder_id: Option<Uuid> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name: String = field.name().unwrap_or("").to_string();
        if name == "file" {
            let filename: String = field.file_name().unwrap_or("upload").to_string();
            let mime: String = field.content_type().unwrap_or("application/octet-stream").to_string();
            if let Ok(bytes) = field.bytes().await {
                let raw: Vec<u8> = bytes.to_vec();
                file_data = Some((filename, mime, raw));
            }
        } else if name == "alt_text" {
            alt_text = field.text().await.ok()
                .map(|s| sanitize_media_text(&s))
                .filter(|s| !s.is_empty());
        } else if name == "folder_id" {
            folder_id = field.text().await.ok()
                .and_then(|s| s.parse().ok());
        }
    }

    let (filename, mime, bytes) = match file_data {
        Some(d) => d,
        None => return Redirect::to("/admin/media").into_response(),
    };

    // Generate unique, SEO-friendly filename from the original name.
    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    let stem = Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");
    let slug = {
        let s = slugify_name(stem);
        // Cap at 80 chars so stored filenames stay reasonable on all filesystems.
        if s.chars().count() > 80 { s.chars().take(80).collect() } else { s }
    };
    let short_id = &Uuid::new_v4().to_string()[..8];
    let stored_name = format!("{}-{}.{}", slug, short_id, ext);

    // Write to uploads directory.
    let upload_path = Path::new(&state.config.uploads_dir).join(&stored_name);
    if let Err(e) = tokio::fs::write(&upload_path, &bytes).await {
        tracing::error!("failed to write upload: {}", e);
        return Redirect::to("/admin/media").into_response();
    }

    let file_size = bytes.len() as i64;

    // Read image dimensions directly from bytes (no disk I/O needed).
    let (img_width, img_height) = if mime.starts_with("image/") {
        match imagesize::blob_size(&bytes) {
            Ok(size) => (Some(size.width as i32), Some(size.height as i32)),
            Err(e) => {
                tracing::warn!("could not read image dimensions for {}: {:?}", filename, e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // Insert into DB.
    let create = CreateMedia {
        site_id: admin.site_id,
        filename,
        mime_type: mime,
        path: stored_name,
        alt_text: alt_text.unwrap_or_default(),
        title: String::new(),
        caption: String::new(),
        width: img_width,
        height: img_height,
        file_size,
        uploaded_by: admin.user.id,
        folder_id,
    };

    if let Err(e) = crate::models::media::create(&state.db, &create).await {
        tracing::error!("failed to save media record: {}", e);
    }

    Redirect::to("/admin/media").into_response()
}
