//! Comment model — flat storage with optional parent for one level of threading.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comment {
    pub id: Uuid,
    pub post_id: Uuid,
    pub site_id: Option<Uuid>,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Flat JOIN result used internally when fetching comments with author names.
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct CommentRow {
    id: Uuid,
    post_id: Uuid,
    site_id: Option<Uuid>,
    author_id: Uuid,
    parent_id: Option<Uuid>,
    body: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    author_display_name: String,
}

/// Comment context exposed to Tera templates.
/// Top-level comments carry their replies nested in `replies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentContext {
    pub id: String,
    pub author_name: String,
    pub parent_id: Option<String>,
    pub body: String,
    /// ISO 8601 timestamp.
    pub created_at: String,
    pub replies: Vec<CommentContext>,
}

pub struct CreateComment {
    pub post_id: Uuid,
    pub site_id: Option<Uuid>,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
}

pub async fn create(pool: &PgPool, data: &CreateComment) -> Result<Comment> {
    let comment = sqlx::query_as::<_, Comment>(
        "INSERT INTO comments (post_id, site_id, author_id, parent_id, body) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(data.post_id)
    .bind(data.site_id)
    .bind(data.author_id)
    .bind(data.parent_id)
    .bind(&data.body)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;
    Ok(comment)
}

/// Fetch all comments for a post joined with author display names,
/// and build a tree: top-level comments contain their direct replies.
pub async fn list_for_post(pool: &PgPool, post_id: Uuid) -> Result<Vec<CommentContext>> {
    let rows: Vec<CommentRow> = sqlx::query_as(
        "SELECT c.id, c.post_id, c.site_id, c.author_id, c.parent_id, c.body, \
                c.created_at, c.updated_at, u.display_name AS author_display_name \
         FROM comments c \
         JOIN users u ON u.id = c.author_id \
         WHERE c.post_id = $1 \
         ORDER BY c.created_at ASC",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    // Build tree: separate top-level and replies.
    let mut top_level: Vec<CommentContext> = Vec::new();
    let mut replies: std::collections::HashMap<String, Vec<CommentContext>> = std::collections::HashMap::new();

    for row in rows {
        let ctx = CommentContext {
            id: row.id.to_string(),
            author_name: row.author_display_name,
            parent_id: row.parent_id.map(|id| id.to_string()),
            body: row.body,
            created_at: row.created_at.format("%B %-d, %Y at %-I:%M %p").to_string(),
            replies: vec![],
        };
        if let Some(ref pid) = ctx.parent_id {
            replies.entry(pid.clone()).or_default().push(ctx);
        } else {
            top_level.push(ctx);
        }
    }

    // Attach replies to their parents.
    for comment in &mut top_level {
        if let Some(child_replies) = replies.remove(&comment.id) {
            comment.replies = child_replies;
        }
    }

    Ok(top_level)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Comment> {
    sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("comment {id}")))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Count of approved comments for a post (used in PostContext).
pub async fn count_for_post(pool: &PgPool, post_id: Uuid) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE post_id = $1")
        .bind(post_id)
        .fetch_one(pool)
        .await
        .map_err(AppError::from)
}
