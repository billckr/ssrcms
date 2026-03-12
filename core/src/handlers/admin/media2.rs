use axum::{extract::{Query, State}, response::Html};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

const PAGE_SIZE: i64 = 50;

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Html<String> {
    let folder_id: Option<Uuid> = params.get("folder_id").and_then(|s| s.parse().ok());
    let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let uploaded_by = if admin.site_role == "author" { Some(admin.user.id) } else { None };

    let folders = if let Some(sid) = admin.site_id {
        crate::models::media_folder::list(&state.db, sid).await.unwrap_or_default()
    } else {
        vec![]
    };

    let total = crate::models::media::count(&state.db, admin.site_id, uploaded_by, folder_id)
        .await.unwrap_or(0);

    let raw = crate::models::media::list(&state.db, admin.site_id, uploaded_by, folder_id, PAGE_SIZE, offset)
        .await.unwrap_or_default();

    let items: Vec<admin::pages::media2::MediaItem> = raw.iter().map(|m| admin::pages::media2::MediaItem {
        id: m.id.to_string(),
        filename: m.filename.clone(),
        mime_type: m.mime_type.clone(),
        path: m.path.clone(),
        alt_text: m.alt_text.clone(),
        title: m.title.clone(),
        caption: m.caption.clone(),
        width: m.width,
        height: m.height,
        file_size: m.file_size,
        folder_id: m.folder_id.map(|u| u.to_string()),
    }).collect();

    let folder_items: Vec<admin::pages::media2::FolderItem> = folders.iter().map(|f| admin::pages::media2::FolderItem {
        id: f.id.to_string(),
        name: f.name.clone(),
    }).collect();

    let active_folder_id = params.get("folder_id").map(|s| s.as_str());

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::media2::render_list(
        &items,
        &folder_items,
        active_folder_id,
        total,
        page,
        PAGE_SIZE,
        None,
        &ctx,
    ))
}
