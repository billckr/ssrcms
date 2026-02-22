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

pub async fn upload(
    State(state): State<AppState>,
    admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_data: Option<(String, String, Vec<u8>)> = None; // (filename, mime, bytes)
    let mut alt_text: Option<String> = None;

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
            alt_text = field.text().await.ok().filter(|s: &String| !s.is_empty());
        }
    }

    let (filename, mime, bytes) = match file_data {
        Some(d) => d,
        None => return Redirect::to("/admin/media").into_response(),
    };

    // Generate unique filename.
    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stored_name = format!("{}.{}", Uuid::new_v4(), ext);

    // Write to uploads directory.
    let upload_path = Path::new(&state.config.uploads_dir).join(&stored_name);
    if let Err(e) = tokio::fs::write(&upload_path, &bytes).await {
        tracing::error!("failed to write upload: {}", e);
        return Redirect::to("/admin/media").into_response();
    }

    let file_size = bytes.len() as i64;

    // Insert into DB.
    let create = CreateMedia {
        filename,
        mime_type: mime,
        path: stored_name,
        alt_text: alt_text.unwrap_or_default(),
        width: None,
        height: None,
        file_size,
        uploaded_by: admin.user.id,
    };

    if let Err(e) = crate::models::media::create(&state.db, &create).await {
        tracing::error!("failed to save media record: {}", e);
    }

    Redirect::to("/admin/media").into_response()
}
