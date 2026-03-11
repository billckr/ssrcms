use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;
use crate::models::post::Post;

/// Save a post for a user. Silently ignores duplicate saves.
pub async fn save(pool: &PgPool, user_id: Uuid, post_id: Uuid, site_id: Option<Uuid>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO saved_posts (user_id, post_id, site_id)
           VALUES ($1, $2, $3)
           ON CONFLICT (user_id, post_id) DO NOTHING"#,
    )
    .bind(user_id)
    .bind(post_id)
    .bind(site_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a saved post for a user.
pub async fn unsave(pool: &PgPool, user_id: Uuid, post_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM saved_posts WHERE user_id = $1 AND post_id = $2")
        .bind(user_id)
        .bind(post_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check whether a user has saved a specific post.
pub async fn is_saved(pool: &PgPool, user_id: Uuid, post_id: Uuid) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM saved_posts WHERE user_id = $1 AND post_id = $2)",
    )
    .bind(user_id)
    .bind(post_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// List all saved posts for a user on a given site, newest saved first.
pub async fn list_for_user(pool: &PgPool, user_id: Uuid, site_id: Option<Uuid>) -> Result<Vec<Post>> {
    Ok(sqlx::query_as::<_, Post>(
        r#"SELECT p.*
           FROM posts p
           JOIN saved_posts sp ON sp.post_id = p.id
           WHERE sp.user_id = $1
             AND ($2::uuid IS NULL OR p.site_id = $2)
             AND p.status = 'published'
           ORDER BY sp.saved_at DESC"#,
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_all(pool)
    .await?)
}
