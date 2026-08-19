//! Shared "write bytes to the uploads dir + insert a `media` row" logic.
//! Used by the manual multipart upload handler (`upload.rs`) and by the WP
//! media importer (`wp_import.rs`) — both need the exact same filename
//! slugging, per-site directory layout, and dimension-detection behavior so
//! imported files aren't second-class citizens next to manually uploaded ones.

use std::path::Path;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::errors::{AppError, Result};
use crate::models::media::{CreateMedia, Media};

/// Convert an arbitrary filename stem into a URL-safe slug.
/// e.g. "My Photo (2026)!" → "my-photo-2026"
pub fn slugify_name(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = true;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen { result.push(c); }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    if result.ends_with('-') { result.pop(); }
    if result.is_empty() { result.push_str("upload"); }
    result
}

pub struct StoreInput {
    pub filename: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub alt_text: String,
    pub title: String,
    pub caption: String,
    pub folder_id: Option<Uuid>,
}

/// Writes the file to `{uploads_dir}/{site_id}/{slug}-{short_id}.{ext}` and
/// inserts the matching `media` row. Falls back to the flat uploads dir
/// (no site subfolder) when `site_id` is `None`, matching `upload.rs`'s
/// existing dev-only fallback behavior.
pub async fn store_and_create(
    state: &AppState,
    site_id: Option<Uuid>,
    uploaded_by: Uuid,
    input: StoreInput,
) -> Result<Media> {
    let ext = Path::new(&input.filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    let stem = Path::new(&input.filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");
    let slug = {
        let s = slugify_name(stem);
        if s.chars().count() > 80 { s.chars().take(80).collect() } else { s }
    };
    let short_id = &Uuid::new_v4().to_string()[..8];
    let stored_name = format!("{}-{}.{}", slug, short_id, ext);

    let (site_subdir, media_path) = if let Some(sid) = site_id {
        let subdir = Path::new(&state.config.uploads_dir).join(sid.to_string());
        tokio::fs::create_dir_all(&subdir)
            .await
            .map_err(|e| AppError::Internal(format!("failed to create upload dir: {e}")))?;
        (subdir, format!("{}/{}", sid, stored_name))
    } else {
        (Path::new(&state.config.uploads_dir).to_path_buf(), stored_name.clone())
    };

    let upload_path = site_subdir.join(&stored_name);
    tokio::fs::write(&upload_path, &input.bytes)
        .await
        .map_err(|e| AppError::Internal(format!("failed to write upload: {e}")))?;

    let file_size = input.bytes.len() as i64;
    let (img_width, img_height) = if input.mime.starts_with("image/") {
        match imagesize::blob_size(&input.bytes) {
            Ok(size) => (Some(size.width as i32), Some(size.height as i32)),
            Err(e) => {
                tracing::warn!("could not read image dimensions for {}: {:?}", input.filename, e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let create = CreateMedia {
        site_id,
        filename: input.filename,
        mime_type: input.mime,
        path: media_path,
        alt_text: input.alt_text,
        title: input.title,
        caption: input.caption,
        width: img_width,
        height: img_height,
        file_size,
        uploaded_by,
        folder_id: input.folder_id,
    };

    crate::models::media::create(&state.db, &create).await
}
