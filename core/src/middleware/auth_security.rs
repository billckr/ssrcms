//! Small, dependency-free abuse limiter for local authentication endpoints.
//!
//! This is intentionally process-local: it protects the current single-instance
//! deployment without a paid/shared service. Move the same policy to PostgreSQL
//! or an existing Redis deployment before running multiple app instances.

use axum::http::HeaderMap;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Bucket {
    attempts: Vec<Instant>,
}

static ATTEMPTS: Lazy<Mutex<HashMap<String, Bucket>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
    let mut buckets = ATTEMPTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if buckets.len() >= 10_000 {
        buckets.retain(|_, bucket| {
            bucket
                .attempts
                .retain(|at| now.duration_since(*at) < window);
            !bucket.attempts.is_empty()
        });
        // Bound memory even during a distributed identifier-flood attack.
        if buckets.len() >= 10_000 && !buckets.contains_key(&key) {
            return false;
        }
    }
    let bucket = buckets.entry(key).or_default();
    bucket
        .attempts
        .retain(|at| now.duration_since(*at) < window);
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

// ── Escalating backoff for repeated login failures ──────────────────────────
//
// `allow()` above is a flat ceiling — a hard ceiling against a flood of
// attempts, same cost per attempt right up until the window's limit. This is
// a separate, second layer specifically for *wrong-password* guessing against
// one specific account: the first few mistakes cost nothing (typos happen),
// but each one after that makes the next attempt wait longer before it's even
// evaluated, capped so a persistent attacker still eventually gets another
// try rather than being locked out forever. Keyed by identity only (not IP),
// scoped per flow — so it still bites a botnet spreading guesses across many
// IPs at one account, which a purely IP-based scheme would miss.

struct FailureState {
    consecutive: u32,
    locked_until: Instant,
}

static FAILURES: Lazy<Mutex<HashMap<String, FailureState>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Failures 1-3 cost nothing; the 4th+ each roughly double the wait, capped.
const FREE_ATTEMPTS: u32 = 3;
const BASE_DELAY_SECS: u64 = 2;
const MAX_DELAY: Duration = Duration::from_secs(5 * 60);
/// Purely an overflow guard on the shift below (2s * 2^16 already dwarfs
/// MAX_DELAY) — the counter itself is otherwise allowed to keep climbing for
/// as long as an attacker keeps retrying.
const MAX_EXPONENT: u32 = 16;

fn failure_key(flow: &str, identity: &str) -> String {
    format!("{flow}:{}", identity.trim().to_lowercase())
}

/// Seconds remaining before the next login attempt for this flow+identity is
/// allowed, or `None` if it's clear to proceed right now.
pub fn login_delay_remaining(flow: &str, identity: &str) -> Option<Duration> {
    let now = Instant::now();
    let key = failure_key(flow, identity);
    let failures = FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    let state = failures.get(&key)?;
    (state.locked_until > now).then(|| state.locked_until - now)
}

/// Record a failed login attempt (wrong password, or an unknown email —
/// treated identically so a guess against a nonexistent account can't be used
/// to distinguish "wrong password" from "no such account"), escalating the
/// delay before the next attempt.
pub fn record_login_failure(flow: &str, identity: &str) {
    let now = Instant::now();
    let key = failure_key(flow, identity);
    let mut failures = FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    if failures.len() >= 10_000 {
        failures.retain(|_, s| s.locked_until > now);
        if failures.len() >= 10_000 && !failures.contains_key(&key) {
            return;
        }
    }
    let state = failures.entry(key).or_insert(FailureState {
        consecutive: 0,
        locked_until: now,
    });
    state.consecutive += 1;
    if state.consecutive > FREE_ATTEMPTS {
        let exponent = (state.consecutive - FREE_ATTEMPTS - 1).min(MAX_EXPONENT);
        let delay = Duration::from_secs(BASE_DELAY_SECS.saturating_mul(1u64 << exponent)).min(MAX_DELAY);
        state.locked_until = now + delay;
    }
}

/// Clear the failure history for this flow+identity — call on a successful
/// login (specifically: once the password itself checks out, even if a later
/// authorization step like role/site access still rejects the request — a
/// wrong-form/wrong-domain mistake isn't a credential-guessing signal).
pub fn record_login_success(flow: &str, identity: &str) {
    let key = failure_key(flow, identity);
    let mut failures = FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    failures.remove(&key);
}

#[cfg(test)]
mod tests {
    use super::{
        consume, login_delay_remaining, record_login_failure, record_login_success, Duration,
        Instant, FREE_ATTEMPTS,
    };

    #[test]
    fn bucket_rejects_after_limit() {
        let now = Instant::now();
        let key = format!("test:{}", uuid::Uuid::new_v4());
        assert!(consume(key.clone(), 2, Duration::from_secs(60), now));
        assert!(consume(key.clone(), 2, Duration::from_secs(60), now));
        assert!(!consume(key, 2, Duration::from_secs(60), now));
    }

    #[test]
    fn login_failures_under_the_free_threshold_add_no_delay() {
        let flow = "test-login";
        let identity = format!("free-{}@example.com", uuid::Uuid::new_v4());
        for _ in 0..FREE_ATTEMPTS {
            record_login_failure(flow, &identity);
            assert!(login_delay_remaining(flow, &identity).is_none());
        }
    }

    #[test]
    fn login_failure_past_the_free_threshold_adds_a_delay() {
        let flow = "test-login";
        let identity = format!("over-{}@example.com", uuid::Uuid::new_v4());
        for _ in 0..FREE_ATTEMPTS {
            record_login_failure(flow, &identity);
        }
        assert!(login_delay_remaining(flow, &identity).is_none());
        record_login_failure(flow, &identity);
        assert!(login_delay_remaining(flow, &identity).is_some());
    }

    #[test]
    fn login_delay_escalates_with_more_failures() {
        let flow = "test-login";
        let identity = format!("escalate-{}@example.com", uuid::Uuid::new_v4());
        for _ in 0..=FREE_ATTEMPTS {
            record_login_failure(flow, &identity);
        }
        let first_delay = login_delay_remaining(flow, &identity).unwrap();
        record_login_failure(flow, &identity);
        let second_delay = login_delay_remaining(flow, &identity).unwrap();
        assert!(
            second_delay > first_delay,
            "expected delay to grow: {first_delay:?} -> {second_delay:?}"
        );
    }

    #[test]
    fn login_success_clears_the_delay() {
        let flow = "test-login";
        let identity = format!("clears-{}@example.com", uuid::Uuid::new_v4());
        for _ in 0..=FREE_ATTEMPTS {
            record_login_failure(flow, &identity);
        }
        assert!(login_delay_remaining(flow, &identity).is_some());
        record_login_success(flow, &identity);
        assert!(login_delay_remaining(flow, &identity).is_none());
        // And the free-attempt count reset with it — one more failure alone
        // shouldn't re-trigger a delay.
        record_login_failure(flow, &identity);
        assert!(login_delay_remaining(flow, &identity).is_none());
    }
}
