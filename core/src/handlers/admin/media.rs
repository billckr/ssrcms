use axum::{extract::{State}, extract::Path};
use uuid::Uuid;
use axum::response::{IntoResponse, Redirect};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use super::sanitize_media_text;

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match crate::models::media::get_by_id(&state.db, id).await {
        Ok(media) => {
            // Enforce site ownership: non-global-admin cannot delete another site's media.
            if !admin.caps.is_global_admin && media.site_id != admin.site_id {
                tracing::warn!(
                    "media delete forbidden: user {} tried to delete media {} (site {:?}) not belonging to their site {:?}",
                    admin.user.id, id, media.site_id, admin.site_id
                );
                return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
            }
            // Author restriction: authors can only delete their own uploads.
            if admin.site_role == "author" && media.uploaded_by != admin.user.id {
                tracing::warn!(
                    "media delete forbidden: author {} tried to delete media {} uploaded by {}",
                    admin.user.id, id, media.uploaded_by
                );
                return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
            }
            let path = std::path::Path::new(&state.config.uploads_dir).join(&media.path);
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("failed to delete media file {:?}: {:?}", path, e);
            }
            if let Err(e) = crate::models::media::delete(&state.db, id).await {
                tracing::error!("failed to delete media record {}: {:?}", id, e);
            }
        }
        Err(e) => {
            tracing::warn!("media {} not found for deletion: {:?}", id, e);
        }
    }
    Redirect::to("/admin/media2").into_response()
}

/// POST /admin/api/media/{id}/meta — update alt text, title, and caption for a media item.
pub async fn api_update_meta(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let alt_text = sanitize_media_text(body.get("alt_text").and_then(|v| v.as_str()).unwrap_or(""));
    let title    = sanitize_media_text(body.get("title").and_then(|v| v.as_str()).unwrap_or(""));
    let caption  = sanitize_media_text(body.get("caption").and_then(|v| v.as_str()).unwrap_or(""));
    // Verify site ownership before allowing update.
    match crate::models::media::get_by_id(&state.db, id).await {
        Ok(media) => {
            if !admin.caps.is_global_admin && media.site_id != admin.site_id {
                return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
            }
        }
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Not found"}))).into_response(),
    }
    match crate::models::media::update_media_meta(&state.db, id, &alt_text, &title, &caption).await {
        Ok(_) => axum::Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => {
            tracing::error!("failed to update meta for media {}: {:?}", id, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "Update failed"}))).into_response()
        }
    }
}

/// POST /admin/api/media/{id}/folder — assign or clear the folder for a media item.
pub async fn api_update_folder(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let folder_id: Option<Uuid> = body.get("folder_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());
    match crate::models::media::get_by_id(&state.db, id).await {
        Ok(media) => {
            if !admin.caps.is_global_admin && media.site_id != admin.site_id {
                return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error":"Forbidden"}))).into_response();
            }
        }
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error":"Not found"}))).into_response(),
    }
    match sqlx::query("UPDATE media SET folder_id = $1 WHERE id = $2")
        .bind(folder_id)
        .bind(id)
        .execute(&state.db)
        .await
    {
        Ok(_) => axum::Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => {
            tracing::error!("failed to update folder for media {}: {:?}", id, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error":"Update failed"}))).into_response()
        }
    }
}

/// GET /admin/api/media — JSON list of images accessible to the current user.
/// Authors see only their own uploads; admins/editors see all site media.
pub async fn api_list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    #[derive(serde::Serialize)]
    struct Item {
        id: String,
        filename: String,
        url: String,
        alt_text: String,
        title: String,
        caption: String,
        mime_type: String,
        file_size: i64,
    }

    let uploaded_by = if admin.site_role == "author" { Some(admin.user.id) } else { None };
    let raw = crate::models::media::list(&state.db, admin.site_id, uploaded_by, None, 500, 0)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("media api_list error: {:?}", e);
            vec![]
        });

    let items: Vec<Item> = raw
        .into_iter()
        .filter(|m| m.mime_type.starts_with("image/"))
        .map(|m| Item {
            id: m.id.to_string(),
            filename: m.filename.clone(),
            url: format!("/uploads/{}", m.path),
            alt_text: m.alt_text.clone(),
            title: m.title.clone(),
            caption: m.caption.clone(),
            mime_type: m.mime_type.clone(),
            file_size: m.file_size,
        })
        .collect();

    axum::Json(items)
}

pub async fn create_folder(
    State(state): State<AppState>,
    admin: AdminUser,
    axum::Form(body): axum::Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let name = body.get("name").map(|s| s.trim()).unwrap_or("");
    // Sanitize: letters, numbers and hyphens only, max 25 chars
    let clean: String = name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(25)
        .collect();
    let clean = clean.trim_matches('-').to_string();
    if clean.len() < 4 {
        return Redirect::to("/admin/media2").into_response();
    }
    if let Some(site_id) = admin.site_id {
        let _ = crate::models::media_folder::create(&state.db, site_id, &clean).await;
    }
    let redirect = body.get("redirect")
        .map(|s| s.as_str())
        .filter(|s| s.starts_with("/admin/"))
        .unwrap_or("/admin/media2");
    Redirect::to(redirect).into_response()
}

pub async fn delete_folder(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Form(body): axum::Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let delete_media = body.get("delete_media").map(|s| s == "true").unwrap_or(false);
    if let Some(site_id) = admin.site_id {
        if delete_media {
            // Delete all media files and DB records belonging to this folder.
            let items = crate::models::media::list(&state.db, Some(site_id), None, Some(id), 10_000, 0)
                .await
                .unwrap_or_default();
            for m in &items {
                let path = std::path::Path::new(&state.config.uploads_dir).join(&m.path);
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("delete_folder: could not remove file {:?}: {:?}", path, e);
                }
                let _ = crate::models::media::delete(&state.db, m.id).await;
            }
        } else {
            // Unassign folder from all images so they appear in All Media.
            let _ = crate::models::media::unassign_folder(&state.db, id, site_id).await;
        }
        let _ = crate::models::media_folder::delete(&state.db, id, site_id).await;
    }
    let redirect_to = body.get("redirect").map(|s| s.as_str()).unwrap_or("/admin/media2");
    Redirect::to(redirect_to).into_response()
}
