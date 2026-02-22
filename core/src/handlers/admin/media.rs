use axum::{extract::State, response::Html, extract::Path};
use uuid::Uuid;
use axum::response::{IntoResponse, Redirect};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::media::MediaItem;

pub async fn list(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    let raw = crate::models::media::list(&state.db, 200, 0).await.unwrap_or_default();
    let items: Vec<MediaItem> = raw.iter().map(|m| MediaItem {
        id: m.id.to_string(),
        filename: m.filename.clone(),
        mime_type: m.mime_type.clone(),
        path: m.path.clone(),
        alt_text: if m.alt_text.is_empty() { None } else { Some(m.alt_text.clone()) },
    }).collect();
    Html(admin::pages::media::render_list(&items, None))
}

pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Ok(media) = crate::models::media::get_by_id(&state.db, id).await {
        // Delete file from disk.
        let path = std::path::Path::new(&state.config.uploads_dir).join(&media.path);
        let _ = std::fs::remove_file(path);
        let _ = crate::models::media::delete(&state.db, id).await;
    }
    Redirect::to("/admin/media")
}
