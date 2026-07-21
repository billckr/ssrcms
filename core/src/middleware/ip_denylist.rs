//! Per-site IP denylist gate — the inverse of `ip_allowlist`: everyone can
//! reach the site *except* the configured IPs/CIDRs.
//!
//! Checked live (no cache) on every request, same pattern as `ip_allowlist`
//! and `maintenance`, so `synap-cli site block-ip on/off` takes effect
//! immediately with no restart. Nothing is exempt — a blocked IP is blocked
//! from `/admin` too.

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use std::net::SocketAddr;

use super::ip_allowlist::{matches_entry, real_ip};
use crate::app_state::AppState;

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
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'ip_denylist_enabled'",
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
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'ip_denylist'",
    )
    .bind(site.id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();

    let client_ip = real_ip(&req, addr);

    if list.split(',').any(|entry| matches_entry(client_ip, entry)) {
        return render();
    }

    next.run(req).await
}

fn render() -> Response {
    let body = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Access Denied</title>
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
    <h1>Access Denied</h1>
    <p>Your IP address has been blocked from accessing this site.</p>
  </div>
</body>
</html>"#;

    let mut resp = Html(body).into_response();
    *resp.status_mut() = StatusCode::FORBIDDEN;
    resp
}
