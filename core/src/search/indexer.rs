//! Functions for keeping the Tantivy index in sync with the posts table.

use sqlx::PgPool;

use crate::models::post::Post;
use super::SearchIndex;

/// Index (or re-index) a single post.
pub fn index_post(index: &SearchIndex, post: &Post) {
    // Strip HTML tags from content before indexing so queries match plain text.
    let plain_content = ammonia::clean_text(&post.content);
    let site_id_str = post.site_id.map(|id| id.to_string()).unwrap_or_default();
    if let Err(e) = index.upsert(
        &post.id.to_string(),
        &site_id_str,
        &post.title,
        &plain_content,
        &post.slug,
        &post.post_type,
    ) {
        tracing::error!("failed to index post {}: {}", post.id, e);
    }
}

/// Remove a post from the index.
pub fn delete_post(index: &SearchIndex, id: &str) {
    if let Err(e) = index.delete(id) {
        tracing::error!("failed to delete post {} from index: {}", id, e);
    }
}

/// Rebuild the entire index from scratch — indexes all published posts and pages.
/// Runs as a background task on startup.
pub async fn rebuild_index(index: SearchIndex, pool: PgPool) {
    tracing::info!("rebuilding search index from database...");

    let posts = match sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE status = 'published' ORDER BY published_at DESC",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("search index rebuild failed — could not fetch posts: {}", e);
            return;
        }
    };

    let count = posts.len();
    for post in &posts {
        index_post(&index, post);
    }

    tracing::info!("search index built: {} documents indexed", count);
}
