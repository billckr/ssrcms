//! WordPress-style maintenance mode gate.
//!
//! Checked live (a single indexed query, no cache) on every public request so
//! `synap site maintenance on/off` takes effect immediately with no
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

    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    render(&message, &default_theme)
}

fn render(message: &str, default_theme: &str) -> Response {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    // Already validated to one of these three literals when saved — safe to
    // splice into JS, but re-checked here since callers pass it through untrusted.
    let default_theme = match default_theme {
        "light" | "dark" => default_theme,
        _ => "system",
    };

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Maintenance</title>
<script>
  // Applies the saved/system theme before first paint (same key + logic as
  // the admin login page) so this public-facing page matches whatever
  // theme was last chosen instead of always rendering light.
  (function() {{
    try {{
      var pref = localStorage.getItem('admin-theme') || '{default_theme}';
      var dark = pref === 'dark' || (pref === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
      if (dark) {{
        document.documentElement.setAttribute('data-theme', 'dark');
      }}
    }} catch (e) {{}}
  }})();
</script>
<style>
  :root {{
    --bg: #f1f1f1;
    --surface: #fff;
    --border: #e2e8f0;
    --text: #23282d;
    --muted: #555;
    --shadow: 0 1px 3px rgba(0,0,0,0.13);
  }}
  :root[data-theme="dark"] {{
    --bg: #2d2d31;
    --surface: #232326;
    --border: #48484f;
    --text: #e4e4e7;
    --muted: #a1a1aa;
    --shadow: 0 1px 3px rgba(0,0,0,.4);
  }}
  html, body {{ height: 100%; margin: 0; }}
  body {{
    display: flex; align-items: center; justify-content: center;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: var(--bg); color: var(--text);
  }}
  .box {{
    max-width: 30rem; margin: 1.5rem; padding: 2rem 2.5rem;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 6px; box-shadow: var(--shadow);
    text-align: center;
  }}
  h1 {{ font-size: 1.3rem; font-weight: 600; margin: 0 0 0.75rem; }}
  p {{ font-size: 1rem; line-height: 1.5; color: var(--muted); margin: 0; }}
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
