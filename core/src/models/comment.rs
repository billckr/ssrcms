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
    pub post_id:    Uuid,
    pub site_id:    Option<Uuid>,
    pub author_id:  Uuid,
    pub parent_id:  Option<Uuid>,
    pub body:       String,
    pub ip_address: Option<String>,
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
        "INSERT INTO comments (post_id, site_id, author_id, parent_id, body, ip_address) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(data.post_id)
    .bind(data.site_id)
    .bind(data.author_id)
    .bind(data.parent_id)
    .bind(&data.body)
    .bind(&data.ip_address)
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

/// Common English stop words — mirrors the list in `search/index.rs`.
/// Stripped from user search input before building ILIKE clauses so that
/// searching "the rust comment" only filters on meaningful terms.
static COMMENT_STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "up", "about", "into", "through", "is",
    "was", "are", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might",
    "shall", "can", "i", "me", "my", "we", "our", "you", "your", "he",
    "him", "his", "she", "her", "it", "its", "they", "them", "their",
    "this", "that", "these", "those", "what", "which", "who", "whom",
    "not", "no", "so", "if", "as", "than", "too", "very", "just", "also",
    "more", "most", "other", "some", "such", "only", "own", "same",
];

/// Split a search string into lowercase terms, stripping stop words.
/// Returns an empty Vec if all terms are stop words (→ no filter applied).
fn search_terms(input: &str) -> Vec<String> {
    input.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !COMMENT_STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Count a user's non-deleted comments for a site — used for pagination.
/// When `search` is provided, only counts rows matching all search terms.
pub async fn count_for_user(
    pool:    &PgPool,
    user_id: Uuid,
    site_id: Uuid,
    search:  Option<&str>,
) -> Result<i64> {
    let terms = search.map(search_terms).unwrap_or_default();

    // Base query joins posts so we can search post title as well as body.
    let mut sql = "SELECT COUNT(*) \
                   FROM comments c \
                   JOIN posts p ON p.id = c.post_id \
                   WHERE c.author_id = $1 AND c.site_id = $2 AND c.deleted_at IS NULL"
        .to_string();

    // Each search term adds an AND clause; both body and post title are searched.
    // Positional params start at $3 (after user_id and site_id).
    for i in 0..terms.len() {
        let n = i + 3;
        sql.push_str(&format!(
            " AND (LOWER(c.body) LIKE ${n} OR LOWER(p.title) LIKE ${n})"
        ));
    }

    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(user_id)
        .bind(site_id);
    for term in &terms {
        q = q.bind(format!("%{term}%"));
    }
    q.fetch_one(pool).await.map_err(AppError::from)
}

/// Fetch a page of a subscriber's own non-deleted comments, newest first.
/// When `search` is provided, results are filtered to rows matching all terms
/// (after stop-word stripping) in either the comment body or the post title.
pub async fn list_for_user(
    pool:    &PgPool,
    user_id: Uuid,
    site_id: Uuid,
    search:  Option<&str>,
    limit:   i64,
    offset:  i64,
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

    let terms = search.map(search_terms).unwrap_or_default();

    let mut sql = "SELECT c.id, c.body, c.created_at, \
                          p.title AS post_title, p.slug AS post_slug, \
                          COALESCE(s.hostname, '') AS site_hostname \
                   FROM comments c \
                   JOIN posts p ON p.id = c.post_id \
                   LEFT JOIN sites s ON s.id = c.site_id \
                   WHERE c.author_id = $1 \
                     AND c.site_id   = $2 \
                     AND c.deleted_at IS NULL"
        .to_string();

    for i in 0..terms.len() {
        let n = i + 3;
        sql.push_str(&format!(
            " AND (LOWER(c.body) LIKE ${n} OR LOWER(p.title) LIKE ${n})"
        ));
    }

    // LIMIT and OFFSET params come after all search-term params.
    let limit_n  = terms.len() + 3;
    let offset_n = terms.len() + 4;
    sql.push_str(&format!(" ORDER BY c.created_at DESC LIMIT ${limit_n} OFFSET ${offset_n}"));

    let mut q = sqlx::query_as::<_, Row>(&sql)
        .bind(user_id)
        .bind(site_id);
    for term in &terms {
        q = q.bind(format!("%{term}%"));
    }
    let rows = q.bind(limit).bind(offset).fetch_all(pool).await.map_err(AppError::from)?;

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
