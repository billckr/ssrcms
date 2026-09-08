//! Small, dependency-free abuse limiter for local authentication endpoints.
//!
//! This is intentionally process-local: it protects the current single-instance
//! deployment without a paid/shared service. Move the same policy to PostgreSQL
//! or an existing Redis deployment before running multiple app instances.

use axum::http::HeaderMap;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Mutex, time::{Duration, Instant}};

#[derive(Default)]
struct Bucket {
    attempts: Vec<Instant>,
}

static ATTEMPTS: Lazy<Mutex<HashMap<String, Bucket>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn client_ip(headers: &HeaderMap) -> &str {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown")
}

fn consume(key: String, limit: usize, window: Duration, now: Instant) -> bool {
    let mut buckets = ATTEMPTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if buckets.len() >= 10_000 {
        buckets.retain(|_, bucket| {
            bucket.attempts.retain(|at| now.duration_since(*at) < window);
            !bucket.attempts.is_empty()
        });
        // Bound memory even during a distributed identifier-flood attack.
        if buckets.len() >= 10_000 && !buckets.contains_key(&key) {
            return false;
        }
    }
    let bucket = buckets.entry(key).or_default();
    bucket.attempts.retain(|at| now.duration_since(*at) < window);
    if bucket.attempts.len() >= limit {
        return false;
    }
    bucket.attempts.push(now);
    true
}

/// Limit both a source IP and an account identifier. Returns false when either
/// bucket is full. Limits are deliberately high enough to avoid easy lockout.
pub fn allow(flow: &str, headers: &HeaderMap, identity: &str) -> bool {
    let now = Instant::now();
    let window = Duration::from_secs(15 * 60);
    let normalized = identity.trim().to_lowercase();
    let ip_ok = consume(format!("{flow}:ip:{}", client_ip(headers)), 60, window, now);
    let identity_ok = consume(format!("{flow}:identity:{normalized}"), 30, window, now);
    ip_ok && identity_ok
}

#[cfg(test)]
mod tests {
    use super::{consume, Duration, Instant};

    #[test]
    fn bucket_rejects_after_limit() {
        let now = Instant::now();
        let key = format!("test:{}", uuid::Uuid::new_v4());
        assert!(consume(key.clone(), 2, Duration::from_secs(60), now));
        assert!(consume(key.clone(), 2, Duration::from_secs(60), now));
        assert!(!consume(key, 2, Duration::from_secs(60), now));
    }
}
