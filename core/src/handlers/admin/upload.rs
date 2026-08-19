//! Multipart file upload handler for admin media.

use axum::{
    extract::{Multipart, State},
    response::{IntoResponse, Redirect},
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use super::media_store::{store_and_create, StoreInput};
use super::sanitize_media_text;

pub async fn upload(
    State(state): State<AppState>,
    admin: AdminUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_data: Option<(String, String, Vec<u8>)> = None; // (filename, mime, bytes)
    let mut alt_text: Option<String> = None;
    let mut folder_id: Option<Uuid> = None;
    let mut redirect_to: String = "/admin/media".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name: String = field.name().unwrap_or("").to_string();
        if name == "redirect" {
            if let Ok(v) = field.text().await {
                // Only allow internal /admin/... redirects.
                if v.starts_with("/admin/") {
                    redirect_to = v;
                }
            }
        } else if name == "file" {
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
        None => return Redirect::to(&redirect_to).into_response(),
    };

    let input = StoreInput {
        filename,
        mime,
        bytes,
        alt_text: alt_text.unwrap_or_default(),
        title: String::new(),
        caption: String::new(),
        folder_id,
    };

    if let Err(e) = store_and_create(&state, admin.site_id, admin.user.id, input).await {
        tracing::error!("failed to save media record: {}", e);
    }

    Redirect::to(&redirect_to).into_response()
}
