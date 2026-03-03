use axum::{extract::State, response::Html, extract::Path};
use uuid::Uuid;
use axum::response::{IntoResponse, Redirect};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::media::MediaItem;

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let uploaded_by = if admin.site_role == "author" { Some(admin.user.id) } else { None };
    let raw = crate::models::media::list(&state.db, admin.site_id, uploaded_by, 200, 0).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list media: {:?}", e);
        vec![]
    });
    let items: Vec<MediaItem> = raw.iter().map(|m| MediaItem {
        id: m.id.to_string(),
        filename: m.filename.clone(),
        mime_type: m.mime_type.clone(),
        path: m.path.clone(),
        alt_text: if m.alt_text.is_empty() { None } else { Some(m.alt_text.clone()) },
    }).collect();
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::media::render_list(&items, None, &ctx))
}

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
    Redirect::to("/admin/media").into_response()
}

/// POST /admin/api/media/{id}/meta — update alt text, title, and caption for a media item.
pub async fn api_update_meta(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let alt_text = body.get("alt_text").and_then(|v| v.as_str()).unwrap_or("");
    let title    = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let caption  = body.get("caption").and_then(|v| v.as_str()).unwrap_or("");
    // Verify site ownership before allowing update.
    match crate::models::media::get_by_id(&state.db, id).await {
        Ok(media) => {
            if !admin.caps.is_global_admin && media.site_id != admin.site_id {
                return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
            }
        }
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Not found"}))).into_response(),
    }
    match crate::models::media::update_media_meta(&state.db, id, alt_text, title, caption).await {
        Ok(_) => axum::Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => {
            tracing::error!("failed to update meta for media {}: {:?}", id, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "Update failed"}))).into_response()
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
    }

    let uploaded_by = if admin.site_role == "author" { Some(admin.user.id) } else { None };
    let raw = crate::models::media::list(&state.db, admin.site_id, uploaded_by, 500, 0)
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
        })
        .collect();

    axum::Json(items)
}
