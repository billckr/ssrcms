use axum::{extract::{Query, State}, response::Html, extract::Path};
use uuid::Uuid;
use axum::response::{IntoResponse, Redirect};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use super::sanitize_media_text;

const PAGE_SIZE: i64 = 10;

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Html<String> {
    let folder_id: Option<Uuid> = params.get("folder_id").and_then(|s| s.parse().ok());
    let page_raw: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
    let select_mode = params.get("picker").map(|s| s == "1").unwrap_or(false);
    let picker_mode = select_mode || params.get("browser").map(|s| s == "1").unwrap_or(false);
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
        let mut tc = admin::pages::media::TypeCounts { all: 0, image: 0, video: 0, audio: 0, document: 0 };
        for r in rows {
            let mime: String = r.get("mime_type");
            let n: i64 = r.get("n");
            if mime.starts_with("image/")      { tc.image    += n; }
            else if mime.starts_with("video/") { tc.video    += n; }
            else if mime.starts_with("audio/") { tc.audio    += n; }
            else                               { tc.document += n; }
        }
        tc.all = tc.image + tc.video + tc.audio + tc.document;
        tc
    };

    let raw = {
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

    // Bulk-fetch display names for all uploaders on this page.
    let uploader_ids: Vec<Uuid> = raw.iter().map(|m| m.uploaded_by).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let uploader_names: std::collections::HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE id = ANY($1)"
    )
    .bind(&uploader_ids[..])
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let cs = state.site_hostname(admin.site_id);

    let items: Vec<admin::pages::media::MediaItem> = raw.iter().map(|m| {
        // Replace the UUID prefix in the stored path with the site hostname so
        // public-facing paths read /uploads/{hostname}/file instead of /uploads/{uuid}/file.
        let display_path = if !cs.is_empty() {
            let filename = m.path.splitn(2, '/').nth(1).unwrap_or(&m.path);
            format!("{}/{}", cs, filename)
        } else {
            m.path.clone()
        };
        admin::pages::media::MediaItem {
        id: m.id.to_string(),
        filename: m.filename.clone(),
        mime_type: m.mime_type.clone(),
        path: display_path,
        alt_text: m.alt_text.clone(),
        title: m.title.clone(),
        caption: m.caption.clone(),
        width: m.width,
        height: m.height,
        file_size: m.file_size,
        folder_id: m.folder_id.map(|u| u.to_string()),
        uploaded_by_name: uploader_names.get(&m.uploaded_by).cloned().unwrap_or_else(|| "Unknown".to_string()),
        uploaded_at: m.created_at.format("%b %-d, %Y").to_string(),
    }}).collect();

    let folder_items: Vec<admin::pages::media::FolderItem> = folders.iter().map(|f| admin::pages::media::FolderItem {
        id: f.id.to_string(),
        name: f.name.clone(),
    }).collect();

    let active_folder_id = params.get("folder_id").map(|s| s.as_str());

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::media::render_list(
        &items,
        &folder_items,
        active_folder_id,
        active_type,
        total,
        page,
        PAGE_SIZE,
        type_counts,
        None,
        picker_mode,
        select_mode,
        &ctx,
    ))
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
    // Verify the target folder belongs to the same site as the media file.
    // Prevents assigning a file to a folder owned by a different site.
    if let Some(fid) = folder_id {
        if !admin.caps.is_global_admin {
            let folder_site: Option<uuid::Uuid> = sqlx::query_scalar(
                "SELECT site_id FROM media_folders WHERE id = $1"
            )
            .bind(fid)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
            match folder_site {
                None => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error":"Folder not found"}))).into_response(),
                Some(fsid) if Some(fsid) != admin.site_id => {
                    tracing::warn!(
                        "api_update_folder: user {} tried to assign media {} to folder {} belonging to site {}",
                        admin.user.id, id, fid, fsid
                    );
                    return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error":"Forbidden"}))).into_response();
                }
                _ => {}
            }
        }
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
        return Redirect::to("/admin/media").into_response();
    }
    if let Some(site_id) = admin.site_id {
        let _ = crate::models::media_folder::create(&state.db, site_id, &clean).await;
    }
    let redirect = body.get("redirect")
        .map(|s| s.as_str())
        .filter(|s| s.starts_with("/admin/"))
        .unwrap_or("/admin/media");
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
    let redirect_to = body.get("redirect").map(|s| s.as_str()).unwrap_or("/admin/media");
    Redirect::to(redirect_to).into_response()
}
