//! Comment model — flat storage with optional parent for one level of threading.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comment {
    pub id:         Uuid,
    pub post_id:    Uuid,
    pub site_id:    Option<Uuid>,
    pub author_id:  Uuid,
    pub parent_id:  Option<Uuid>,
    pub body:       String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Flat JOIN result used internally when fetching comments with author names.
#[derive(Debug, sqlx::FromRow)]
struct CommentRow {
    id:                  Uuid,
    parent_id:           Option<Uuid>,
    body:                String,
    created_at:          DateTime<Utc>,
    deleted_at:          Option<DateTime<Utc>>,
    author_display_name: String,
}

/// Pagination envelope returned by `list_for_post`.
pub struct CommentPage {
    pub comments:     Vec<CommentContext>,
    pub current_page: usize,
    pub total_pages:  usize,
    pub total_count:  usize,
}

/// Comment context exposed to Tera templates.
/// Top-level comments carry their replies nested in `replies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentContext {
    pub id:         String,
    pub author_name: String,
    pub parent_id:  Option<String>,
    pub body:       String,
    /// True when the comment has been soft-deleted (body should render as [deleted]).
    pub is_deleted: bool,
    /// ISO 8601 timestamp.
    pub created_at: String,
    pub replies:    Vec<CommentContext>,
}

pub struct CreateComment {
    pub post_id:   Uuid,
    pub site_id:   Option<Uuid>,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body:      String,
}

/// Raw DB result used by `list_for_user` before UI mapping.
pub struct UserCommentRecord {
    pub id:            Uuid,
    pub body:          String,
    pub post_title:    String,
    pub post_slug:     String,
    pub site_hostname: String,
    pub created_at:    DateTime<Utc>,
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

/// Fetch all visible comments for a post and build a two-level tree.
///
/// Soft-delete rules applied here:
/// - Deleted reply            → excluded entirely.
/// - Deleted top-level with replies → kept, body replaced with "[deleted]", is_deleted = true.
/// - Deleted top-level with no replies → excluded entirely.
pub async fn list_for_post(
    pool:     &PgPool,
    post_id:  Uuid,
    page:     usize,
    per_page: usize,
) -> Result<CommentPage> {
    let rows: Vec<CommentRow> = sqlx::query_as(
        "SELECT c.id, c.parent_id, c.body, c.created_at, c.deleted_at, \
                u.display_name AS author_display_name \
         FROM comments c \
         JOIN users u ON u.id = c.author_id \
         WHERE c.post_id = $1 \
         ORDER BY c.created_at ASC",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    // First pass: count how many non-deleted replies each top-level comment has.
    let mut reply_counts: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();
    for row in &rows {
        if let Some(pid) = row.parent_id {
            if row.deleted_at.is_none() {
                *reply_counts.entry(pid).or_insert(0) += 1;
            }
        }
    }

    // Second pass: build top-level list and reply map, applying soft-delete rules.
    let mut top_level: Vec<CommentContext> = Vec::new();
    let mut replies_map: std::collections::HashMap<String, Vec<CommentContext>> =
        std::collections::HashMap::new();

    for row in rows {
        let is_reply = row.parent_id.is_some();
        let is_deleted = row.deleted_at.is_some();

        if is_reply {
            // Deleted replies are excluded entirely.
            if is_deleted { continue; }
            let ctx = CommentContext {
                id:          row.id.to_string(),
                author_name: row.author_display_name,
                parent_id:   row.parent_id.map(|id| id.to_string()),
                body:        row.body,
                is_deleted:  false,
                created_at:  row.created_at.format("%B %-d, %Y at %-I:%M %p").to_string(),
                replies:     vec![],
            };
            replies_map
                .entry(ctx.parent_id.clone().unwrap())
                .or_default()
                .push(ctx);
        } else {
            // Deleted top-level with no remaining replies → skip entirely.
            if is_deleted && reply_counts.get(&row.id).copied().unwrap_or(0) == 0 {
                continue;
            }
            let (body, flagged) = if is_deleted {
                (String::new(), true)
            } else {
                (row.body, false)
            };
            top_level.push(CommentContext {
                id:          row.id.to_string(),
                author_name: row.author_display_name,
                parent_id:   None,
                body,
                is_deleted:  flagged,
                created_at:  row.created_at.format("%B %-d, %Y at %-I:%M %p").to_string(),
                replies:     vec![],
            });
        }
    }

    // Attach replies to their parents.
    for comment in &mut top_level {
        if let Some(child_replies) = replies_map.remove(&comment.id) {
            comment.replies = child_replies;
        }
    }

    let total_count = top_level.len();
    let total_pages = if total_count == 0 { 1 } else { (total_count + per_page - 1) / per_page };
    let page = page.max(1).min(total_pages);
    let start = (page - 1) * per_page;
    let comments = top_level.into_iter().skip(start).take(per_page).collect();

    Ok(CommentPage { comments, current_page: page, total_pages, total_count })
}

/// Fetch a subscriber's own non-deleted comments for the current site, newest first.
pub async fn list_for_user(
    pool:    &PgPool,
    user_id: Uuid,
    site_id: Uuid,
) -> Result<Vec<UserCommentRecord>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id:            Uuid,
        body:          String,
        created_at:    DateTime<Utc>,
        post_title:    String,
        post_slug:     String,
        site_hostname: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT c.id, c.body, c.created_at, \
                p.title AS post_title, p.slug AS post_slug, \
                COALESCE(s.hostname, '') AS site_hostname \
         FROM comments c \
         JOIN posts p ON p.id = c.post_id \
         LEFT JOIN sites s ON s.id = c.site_id \
         WHERE c.author_id = $1 \
           AND c.site_id   = $2 \
           AND c.deleted_at IS NULL \
         ORDER BY c.created_at DESC",
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    Ok(rows.into_iter().map(|r| UserCommentRecord {
        id:            r.id,
        body:          r.body,
        post_title:    r.post_title,
        post_slug:     r.post_slug,
        site_hostname: r.site_hostname,
        created_at:    r.created_at,
    }).collect())
}

/// Soft-delete a comment. Only succeeds if the comment belongs to `author_id`
/// and has not already been soft-deleted.
pub async fn soft_delete(pool: &PgPool, id: Uuid, author_id: Uuid) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE comments \
         SET deleted_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND author_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(author_id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(result.rows_affected() == 1)
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

/// Count of visible (non-deleted) comments for a post (used in PostContext).
pub async fn count_for_post(pool: &PgPool, post_id: Uuid) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM comments WHERE post_id = $1 AND deleted_at IS NULL",
    )
    .bind(post_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}
