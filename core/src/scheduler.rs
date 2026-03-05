//! Background tasks that run on a timer inside the Tokio runtime.
//!
//! These replace any need for external cron jobs or systemd timers for
//! time-sensitive CMS operations.

use sqlx::PgPool;
use tokio::time::{interval, Duration};

/// Spawn a background task that publishes scheduled posts whose `published_at`
/// has passed.
///
/// Runs every 60 seconds. Updates `status = 'published'` for all posts where
/// `status = 'scheduled' AND published_at <= NOW()`. Logs the count of posts
/// promoted on each cycle that has work to do.
pub fn spawn_scheduled_publisher(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            match publish_due_posts(&pool).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("scheduler: published {} scheduled post(s)", n),
                Err(e) => tracing::warn!("scheduler: failed to publish scheduled posts: {:?}", e),
            }
        }
    });
}

async fn publish_due_posts(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE posts
        SET status = 'published'
        WHERE status = 'scheduled'
          AND published_at <= NOW()
        "#,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
