use axum::{extract::{Query, State}, response::Html};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

const PAGE_SIZE: i64 = 10;

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Html<String> {
    let folder_id: Option<Uuid> = params.get("folder_id").and_then(|s| s.parse().ok());
    let page_raw: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
    // Validate type param — only accept known values
    let active_type: Option<&str> = params.get("type")
        .map(|s| s.as_str())
        .filter(|s| ["image", "video", "audio", "document"].contains(s));

    let uploaded_by = if admin.site_role == "author" { Some(admin.user.id) } else { None };

    let folders = if let Some(sid) = admin.site_id {
        crate::models::media_folder::list(&state.db, sid).await.unwrap_or_default()
    } else {
        vec![]
    };

    let type_sql = match active_type {
        Some("image")    => " AND mime_type LIKE 'image/%'",
        Some("video")    => " AND mime_type LIKE 'video/%'",
        Some("audio")    => " AND mime_type LIKE 'audio/%'",
        Some("document") => " AND mime_type NOT LIKE 'image/%' AND mime_type NOT LIKE 'video/%' AND mime_type NOT LIKE 'audio/%'",
        _                => "",
    };

    let total = {
        let sql = format!(
            "SELECT COUNT(*) FROM media \
             WHERE ($1::uuid IS NULL OR site_id = $1) \
               AND ($2::uuid IS NULL OR uploaded_by = $2) \
               AND ($3::uuid IS NULL OR folder_id = $3){}",
            type_sql
        );
        sqlx::query(&sql)
            .bind(admin.site_id)
            .bind(uploaded_by)
            .bind(folder_id)
            .fetch_one(&state.db)
            .await
            .map(|r: sqlx::postgres::PgRow| { use sqlx::Row as _; r.get::<i64, _>(0) })
            .unwrap_or(0)
    };

    // Clamp page to valid range
    let total_pages = ((total as f64) / (PAGE_SIZE as f64)).ceil() as i64;
    let page = if total_pages > 0 { page_raw.min(total_pages) } else { 1 };
    let offset = (page - 1) * PAGE_SIZE;

    // Count by type across the whole folder (not just current page)
    let type_counts = {
        use sqlx::Row as _;
        let rows = sqlx::query(
            "SELECT mime_type, COUNT(*) AS n FROM media \
             WHERE ($1::uuid IS NULL OR site_id = $1) \
               AND ($2::uuid IS NULL OR uploaded_by = $2) \
               AND ($3::uuid IS NULL OR folder_id = $3) \
             GROUP BY mime_type",
        )
        .bind(admin.site_id)
        .bind(uploaded_by)
        .bind(folder_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
        let mut tc = admin::pages::media2::TypeCounts { all: total, image: 0, video: 0, audio: 0, document: 0 };
        for r in rows {
            let mime: String = r.get("mime_type");
            let n: i64 = r.get("n");
            if mime.starts_with("image/")      { tc.image    += n; }
            else if mime.starts_with("video/") { tc.video    += n; }
            else if mime.starts_with("audio/") { tc.audio    += n; }
            else                               { tc.document += n; }
        }
        tc
    };

    let raw = {
        use sqlx::Row as _;
        let sql = format!(
            "SELECT * FROM media \
             WHERE ($1::uuid IS NULL OR site_id = $1) \
               AND ($2::uuid IS NULL OR uploaded_by = $2) \
               AND ($3::uuid IS NULL OR folder_id = $3){} \
             ORDER BY created_at DESC LIMIT $4 OFFSET $5",
            type_sql
        );
        sqlx::query_as::<_, crate::models::media::Media>(&sql)
            .bind(admin.site_id)
            .bind(uploaded_by)
            .bind(folder_id)
            .bind(PAGE_SIZE)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
    };

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
        active_type,
        total,
        page,
        PAGE_SIZE,
        type_counts,
        None,
        &ctx,
    ))
}
