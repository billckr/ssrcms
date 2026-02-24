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
    let raw = crate::models::media::list(&state.db, admin.site_id, 200, 0).await.unwrap_or_else(|e| {
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
    Html(admin::pages::media::render_list(&items, None, &cs, admin.is_global_admin, admin.is_visiting_foreign_site, &admin.user.email))
}

pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match crate::models::media::get_by_id(&state.db, id).await {
        Ok(media) => {
            // Enforce site ownership: non-global-admin cannot delete another site's media.
            if !admin.is_global_admin && media.site_id != admin.site_id {
                tracing::warn!(
                    "media delete forbidden: user {} tried to delete media {} (site {:?}) not belonging to their site {:?}",
                    admin.user.id, id, media.site_id, admin.site_id
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
