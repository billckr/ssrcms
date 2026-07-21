//! WordPress-style maintenance mode gate.
//!
//! Checked live (a single indexed query, no cache) on every public request so
//! `synap-cli site maintenance on/off` takes effect immediately with no
//! restart and no reload signal. `/admin/*` is always exempt so an operator
//! can still log in to turn it back off; static asset routes are exempt too
//! since blocking them would break the maintenance page's own styling and
//! any in-flight admin session's assets.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{Html, IntoResponse, Response},
};

use crate::app_state::AppState;

fn is_exempt(path: &str) -> bool {
    path.starts_with("/admin")
        || path.starts_with("/theme/static")
        || path.starts_with("/uploads")
        || path.starts_with("/metrics")
}

pub async fn gate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if is_exempt(&path) {
        return next.run(req).await;
    }

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

    let mode: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'maintenance_mode'",
    )
    .bind(site.id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if mode.as_deref() != Some("true") {
        return next.run(req).await;
    }

    let message: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE site_id = $1 AND key = 'maintenance_message'",
    )
    .bind(site.id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "This site is currently undergoing scheduled maintenance. Please check back soon.".to_string());

    render(&message)
}

fn render(message: &str) -> Response {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Maintenance</title>
<style>
  html, body {{ height: 100%; margin: 0; }}
  body {{
    display: flex; align-items: center; justify-content: center;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #f1f1f1; color: #23282d;
  }}
  .box {{
    max-width: 30rem; margin: 1.5rem; padding: 2rem 2.5rem;
    background: #fff; border-radius: 6px; box-shadow: 0 1px 3px rgba(0,0,0,0.13);
    text-align: center;
  }}
  h1 {{ font-size: 1.3rem; font-weight: 600; margin: 0 0 0.75rem; }}
  p {{ font-size: 1rem; line-height: 1.5; color: #555; margin: 0; }}
</style>
</head>
<body>
  <div class="box">
    <h1>Under Maintenance</h1>
    <p>{escaped}</p>
  </div>
</body>
</html>"#
    );

    let mut resp = Html(body).into_response();
    *resp.status_mut() = axum::http::StatusCode::SERVICE_UNAVAILABLE;
    resp.headers_mut()
        .insert(header::RETRY_AFTER, axum::http::HeaderValue::from_static("3600"));
    resp
}
