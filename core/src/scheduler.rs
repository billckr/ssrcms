//! Background tasks that run on a timer inside the Tokio runtime.
//!
//! These replace any need for external cron jobs or systemd timers for
//! time-sensitive CMS operations.

use sqlx::PgPool;
use tokio::time::{interval, Duration};

use crate::app_state::ViewBuffer;

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

/// Spawn a background task that drains the in-memory view buffer into the DB.
///
/// Runs every 60 seconds. Swaps the buffer under the lock (minimising lock hold
/// time), then batch-inserts with ON CONFLICT DO NOTHING so duplicate views from
/// the same buffer cycle are silently ignored.
pub fn spawn_view_flush(pool: PgPool, buffer: ViewBuffer) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;

            // Swap out the buffer under the lock; release immediately.
            let batch = {
                let mut guard = buffer.lock().unwrap();
                if guard.is_empty() {
                    continue;
                }
                std::mem::take(&mut *guard)
            };

            let count = batch.len();
            for (post_id, ip_hash, viewed_date) in batch {
                let _ = sqlx::query(
                    "INSERT INTO post_views (post_id, ip_hash, viewed_date)
                     VALUES ($1, $2, $3)
                     ON CONFLICT DO NOTHING",
                )
                .bind(post_id)
                .bind(&ip_hash)
                .bind(viewed_date)
                .execute(&pool)
                .await
                .map_err(|e| tracing::warn!("view flush error: {:?}", e));
            }

            tracing::debug!("view flush: wrote {} record(s)", count);
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
