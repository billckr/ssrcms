//! Background tasks that run on a timer inside the Tokio runtime.
//!
//! These replace any need for external cron jobs or systemd timers for
//! time-sensitive CMS operations.

use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use uuid::Uuid;

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

/// Spawn a background task that drains the view-tracking channel into the DB.
///
/// Every 60 seconds the task wakes up and calls `try_recv` in a tight loop to
/// drain every message that accumulated since the last cycle.  Messages are
/// collected into a local `HashSet` first so duplicate views (same post + same
/// anonymized IP + same day) are discarded before any DB work happens — this
/// mirrors the deduplication the old `Arc<Mutex<HashSet>>` buffer provided, but
/// without any shared mutable state between request handlers.
///
/// Each row is inserted with `ON CONFLICT DO NOTHING` as a second safety net
/// against duplicates that span flush cycles (e.g. after a process restart).
///
/// Why the receiver lives here and not in AppState:
///   Only one task should ever read from the receiver.  Keeping it out of
///   AppState enforces that at the type level — `UnboundedReceiver` is not
///   `Clone`, so it cannot accidentally be shared or double-consumed.
pub fn spawn_view_flush(pool: PgPool, mut rx: mpsc::UnboundedReceiver<(Uuid, String, chrono::NaiveDate)>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;

            // Drain every message queued since the last cycle into a local
            // HashSet.  The HashSet deduplicates (post_id, ip_hash, date)
            // triples — a visitor refreshing a page rapidly produces only one
            // row per day in the DB.
            let mut batch: std::collections::HashSet<(Uuid, String, chrono::NaiveDate)> =
                std::collections::HashSet::new();
            while let Ok(record) = rx.try_recv() {
                batch.insert(record);
            }

            if batch.is_empty() {
                continue;
            }

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
