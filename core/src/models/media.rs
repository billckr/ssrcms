use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Media {
    pub id: Uuid,
    pub site_id: Option<Uuid>,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: String,
    pub title: String,
    pub caption: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: i64,
    pub uploaded_by: Uuid,
    pub folder_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Media {
    /// Build the public URL for this media item.
    pub fn url(&self, base_url: &str) -> String {
        format!("{}/uploads/{}", base_url, self.path)
    }
}

/// Serializable view of Media for template context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaContext {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub mime_type: String,
    pub alt_text: String,
    pub title: String,
    pub caption: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl MediaContext {
    pub fn from_media(media: &Media, base_url: &str) -> Self {
        MediaContext {
            id: media.id.to_string(),
            url: media.url(base_url),
            filename: media.filename.clone(),
            mime_type: media.mime_type.clone(),
            alt_text: media.alt_text.clone(),
            title: media.title.clone(),
            caption: media.caption.clone(),
            width: media.width,
            height: media.height,
        }
    }
}

/// Data required to register a new media record after upload.
#[derive(Debug)]
pub struct CreateMedia {
    pub site_id: Option<Uuid>,
    pub filename: String,
    pub mime_type: String,
    pub path: String,
    pub alt_text: String,
    pub title: String,
    pub caption: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: i64,
    pub uploaded_by: Uuid,
    pub folder_id: Option<Uuid>,
}

pub async fn update_media_meta(pool: &PgPool, id: Uuid, alt_text: &str, title: &str, caption: &str) -> Result<()> {
    let affected = sqlx::query(
        "UPDATE media SET alt_text = $1, title = $2, caption = $3 WHERE id = $4"
    )
    .bind(alt_text)
    .bind(title)
    .bind(caption)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!("media {id}")));
    }
    Ok(())
}

pub async fn create(pool: &PgPool, data: &CreateMedia) -> Result<Media> {
    let media = sqlx::query_as::<_, Media>(
        r#"
        INSERT INTO media (site_id, filename, mime_type, path, alt_text, title, caption, width, height, file_size, uploaded_by, folder_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *
        "#,
    )
    .bind(data.site_id)
    .bind(&data.filename)
    .bind(&data.mime_type)
    .bind(&data.path)
    .bind(&data.alt_text)
    .bind(&data.title)
    .bind(&data.caption)
    .bind(data.width)
    .bind(data.height)
    .bind(data.file_size)
    .bind(data.uploaded_by)
    .bind(data.folder_id)
    .fetch_one(pool)
    .await?;

    Ok(media)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Media> {
    sqlx::query_as::<_, Media>("SELECT * FROM media WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("media {id}")))
}

#[allow(dead_code)]
pub async fn update_alt_text(pool: &PgPool, id: Uuid, alt_text: &str) -> Result<()> {
    let affected = sqlx::query("UPDATE media SET alt_text = $1 WHERE id = $2")
        .bind(alt_text)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!("media {id}")));
    }
    Ok(())
}

pub async fn unassign_folder(pool: &PgPool, folder_id: Uuid, site_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE media SET folder_id = NULL WHERE folder_id = $1 AND site_id = $2")
        .bind(folder_id)
        .bind(site_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM media WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list(pool: &PgPool, site_id: Option<Uuid>, uploaded_by: Option<Uuid>, folder_id: Option<Uuid>, limit: i64, offset: i64) -> Result<Vec<Media>> {
    let items = sqlx::query_as::<_, Media>(
        "SELECT * FROM media \
         WHERE ($1::uuid IS NULL OR site_id = $1) \
           AND ($2::uuid IS NULL OR uploaded_by = $2) \
           AND ($3::uuid IS NULL OR folder_id = $3) \
         ORDER BY created_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(site_id)
    .bind(uploaded_by)
    .bind(folder_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(items)
}
