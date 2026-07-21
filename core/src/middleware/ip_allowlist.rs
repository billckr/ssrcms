//! Per-site IP allowlist gate — blocks all traffic to a site except from a
//! configured set of IPs/CIDRs.
//!
//! Checked live (no cache) on every request, same as maintenance mode, so
//! `synap-cli site allow-ip on/off` takes effect immediately with no
//! restart. Unlike maintenance mode, nothing is exempt: if this is on and
//! the caller's IP isn't on the list, `/admin` is blocked too. This is meant
//! for hard isolation during testing (e.g. a VPS test deploy nobody but you
//! should be able to reach), not a "the operator can still log in remotely"
//! gate — if you lock yourself out, you need shell/SSH access to the box to
//! turn it back off.

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};

use crate::app_state::AppState;

/// Real client IP: prefer X-Real-IP (set by Caddy), then the first hop of
/// X-Forwarded-For, finally the raw socket address. Trustworthy here because
/// Axum only binds to a private interface — Caddy is the only thing that can
/// ever reach it, so these headers can't be forged by an outside caller.
///
/// Shared with `ip_denylist` — same trust model, same header-parsing logic.
pub(crate) fn real_ip(req: &Request, addr: SocketAddr) -> IpAddr {
    if let Some(v) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Ok(ip) = v.trim().parse::<IpAddr>() {
            return ip;
        }
    }
    if let Some(v) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            if let Ok(ip) = first.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    addr.ip()
}

/// Parse an allowlist/denylist entry ("1.2.3.4" or "1.2.3.0/24", IPv4 or
/// IPv6) and test whether `ip` falls inside it. Bare IPs are treated as a
/// /32 or /128. Shared with `ip_denylist`.
pub(crate) fn matches_entry(ip: IpAddr, entry: &str) -> bool {
    let entry = entry.trim();
    if entry.is_empty() {
        return false;
    }
    let (net_str, bits) = match entry.split_once('/') {
        Some((n, b)) => (n, b.parse::<u32>().ok()),
        None => (entry, None),
    };
    let Ok(net_ip) = net_str.parse::<IpAddr>() else {
        return false;
    };

    match (ip, net_ip) {
        (IpAddr::V4(ip4), IpAddr::V4(net4)) => {
            let prefix = bits.unwrap_or(32).min(32);
            let mask: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
            (u32::from(ip4) & mask) == (u32::from(net4) & mask)
        }
        (IpAddr::V6(ip6), IpAddr::V6(net6)) => {
            let prefix = bits.unwrap_or(128).min(128);
            let mask: u128 = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
            (u128::from(ip6) & mask) == (u128::from(net6) & mask)
        }
        _ => false,
    }
}

pub async fn gate(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let hostname = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or(h).to_string());

    let Some(hostname) = hostname else {
        return next.run(req).await;
    };

    let Some((site, _)) = state.resolve_site(&hostname) else {
        return next.run(req).await;
    };

    let enabled: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'ip_allowlist_enabled'",
    )
    .bind(site.id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if enabled.as_deref() != Some("true") {
        return next.run(req).await;
    }

    let list: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'ip_allowlist'",
    )
    .bind(site.id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();

    let client_ip = real_ip(&req, addr);

    if list.split(',').any(|entry| matches_entry(client_ip, entry)) {
        return next.run(req).await;
    }

    render()
}

fn render() -> Response {
    let body = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Access Restricted</title>
<style>
  html, body { height: 100%; margin: 0; }
  body {
    display: flex; align-items: center; justify-content: center;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #f1f1f1; color: #23282d;
  }
  .box {
    max-width: 30rem; margin: 1.5rem; padding: 2rem 2.5rem;
    background: #fff; border-radius: 6px; box-shadow: 0 1px 3px rgba(0,0,0,0.13);
    text-align: center;
  }
  h1 { font-size: 1.3rem; font-weight: 600; margin: 0 0 0.75rem; }
  p { font-size: 1rem; line-height: 1.5; color: #555; margin: 0; }
</style>
</head>
<body>
  <div class="box">
    <h1>Access Restricted</h1>
    <p>This site is not available from your location.</p>
  </div>
</body>
</html>"#;

    let mut resp = Html(body).into_response();
    *resp.status_mut() = StatusCode::FORBIDDEN;
    resp
}
