//! Background tasks that run on a timer inside the Tokio runtime.
//!
//! These replace any need for external cron jobs or systemd timers for
//! time-sensitive CMS operations.

use sqlx::PgPool;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use uuid::Uuid;

/// The latest published GitHub Release, as last seen by `spawn_release_check`.
#[derive(Debug, Clone)]
pub struct LatestRelease {
    /// The release's tag name, e.g. "v0.1.0-alpha18".
    pub tag_name: String,
    /// Link to the release page — shown as a secondary, opt-in link; most
    /// admins won't know or care what GitHub is, so `body` below is the
    /// primary in-app "what's new" text.
    pub html_url: String,
    /// Release notes body (GitHub's auto-generated notes today — a bulleted
    /// PR list once the project uses PRs, or hand-written notes if a real
    /// changelog process gets adopted later). Empty string if GitHub
    /// returned no body.
    pub body: String,
}

#[derive(serde::Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
}

/// Spawn a background task that periodically checks GitHub Releases for the
/// latest published SynapCMS release, so the admin dashboard can show an
/// "update available" notice.
///
/// Checked every 6 hours (well within GitHub's 60 req/hr unauthenticated
/// rate limit for a single instance). Failures (offline install, GitHub
/// unreachable, rate-limited) are logged and otherwise ignored — the next
/// tick tries again, and the notice simply doesn't show in the meantime.
pub fn spawn_release_check(latest_release: Arc<RwLock<Option<LatestRelease>>>) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .user_agent("SynapCMS-UpdateCheck/1.0")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut ticker = interval(Duration::from_secs(6 * 60 * 60));
        loop {
            ticker.tick().await;
            match check_latest_release(&client).await {
                Ok(release) => {
                    if let Ok(mut w) = latest_release.write() {
                        *w = Some(release);
                    }
                }
                Err(e) => tracing::warn!("release check: failed to fetch latest release: {:?}", e),
            }
        }
    });
}

async fn check_latest_release(client: &reqwest::Client) -> Result<LatestRelease, reqwest::Error> {
    let resp = client
        .get("https://api.github.com/repos/billckr/ssrcms/releases/latest")
        .send()
        .await?
        .error_for_status()?
        .json::<GithubReleaseResponse>()
        .await?;
    Ok(LatestRelease {
        tag_name: resp.tag_name,
        html_url: resp.html_url,
        body: resp.body.unwrap_or_default(),
    })
}

/// Spawn a background task that publishes scheduled posts whose `published_at`
/// has passed.
///
/// Runs every 60 seconds. Updates `status = 'published'` for all posts where
/// `status = 'scheduled' AND published_at <= NOW()`. Logs the count of posts
/// promoted on each cycle that has work to do.
pub fn spawn_scheduled_publisher(pool: PgPool, search_index: Arc<crate::search::SearchIndex>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            match publish_due_posts(&pool, &search_index).await {
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
pub fn spawn_view_flush(
    pool: PgPool,
    mut rx: mpsc::UnboundedReceiver<(Uuid, String, chrono::NaiveDate)>,
) {
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

async fn publish_due_posts(
    pool: &PgPool,
    search_index: &crate::search::SearchIndex,
) -> Result<u64, sqlx::Error> {
    let posts = sqlx::query_as::<_, crate::models::post::Post>(
        r#"
        UPDATE posts
        SET status = 'published'
        WHERE status = 'scheduled'
          AND published_at <= NOW()
        RETURNING *
        "#,
    )
    .fetch_all(pool)
    .await?;
    for post in &posts {
        crate::search::indexer::index_post(search_index, post);
    }
    Ok(posts.len() as u64)
}
