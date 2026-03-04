//! Password-protection unlock handlers.
//!
//! When a post or page has `post_password` set, the public handlers redirect
//! here instead of rendering content. The visitor must submit the correct
//! plain-text password; on success a browser-session cookie (no Max-Age) is
//! set for the post so the visitor can view the content until they close their
//! browser or use a different browser.
//!
//! Cookie name:  `post_pw_{post_uuid}`
//! Cookie value: `1`  (presence = unlocked)
//! Lifetime: browser session (no Max-Age / no Expires → deleted on browser close)

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::post;

const COOKIE_PREFIX: &str = "post_pw_";

fn unlock_cookie_name(post_id: uuid::Uuid) -> String {
    format!("{}{}", COOKIE_PREFIX, post_id)
}

#[derive(Deserialize)]
pub struct UnlockForm {
    pub post_password: String,
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Check whether the visitor has an active browser-session unlock for `post_id`.
///
/// Returns `true` only when the browser-session cookie is present in `jar`.
/// A fresh browser (no cookies) or a different device will always return `false`.
pub fn is_unlocked(jar: &CookieJar, post_id: uuid::Uuid) -> bool {
    jar.get(&unlock_cookie_name(post_id)).is_some()
}

/// Return a full-page password gate `Response`.
pub fn gate_response(post_title: &str, action: &str, error: Option<&str>) -> Response {
    Html(gate_html(post_title, action, error)).into_response()
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /blog/{slug}/unlock` — verify password for a post.
pub async fn unlock_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<UnlockForm>,
) -> Response {
    unlock_inner(
        &state,
        current_site,
        &slug,
        &form.post_password,
        jar,
        &format!("/blog/{}/unlock", slug),
        &format!("/blog/{}", slug),
    )
    .await
}

/// `POST /{slug}/unlock` — verify password for a page.
pub async fn unlock_page(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<UnlockForm>,
) -> Response {
    unlock_inner(
        &state,
        current_site,
        &slug,
        &form.post_password,
        jar,
        &format!("/{}/unlock", slug),
        &format!("/{}", slug),
    )
    .await
}

// ── Shared logic ──────────────────────────────────────────────────────────────

async fn unlock_inner(
    state: &AppState,
    current_site: CurrentSite,
    slug: &str,
    password: &str,
    jar: CookieJar,
    form_action: &str,
    redirect_to: &str,
) -> Response {
    let site_id = current_site.site.id;

    let post_record = match post::get_published_by_slug(&state.db, Some(site_id), slug).await {
        Ok(p) => p,
        Err(_) => return Redirect::to(redirect_to).into_response(),
    };

    let hash = match &post_record.post_password {
        Some(h) => h.clone(),
        None => return Redirect::to(redirect_to).into_response(), // no longer protected
    };

    if crate::models::user::verify_password(password, &hash) {
        // Set a browser-session cookie (no Max-Age → deleted when browser closes).
        // HttpOnly + SameSite=Lax prevents JS access and CSRF.
        let cookie = Cookie::build((unlock_cookie_name(post_record.id), "1"))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        (jar.add(cookie), Redirect::to(redirect_to)).into_response()
    } else {
        gate_response(&post_record.title, form_action, Some("Incorrect password. Please try again."))
    }
}

// ── Gate HTML ─────────────────────────────────────────────────────────────────

fn gate_html(post_title: &str, action: &str, error: Option<&str>) -> String {
    let error_html = error.map(|e| {
        format!(
            r#"<div class="error">{}</div>"#,
            html_escape(e)
        )
    }).unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Protected Content — {title}</title>
  <style>
    *, *::before, *::after {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #f4f5f7;
      font-family: system-ui, -apple-system, sans-serif;
      color: #222;
    }}
    .gate {{
      background: #fff;
      border-radius: 10px;
      box-shadow: 0 4px 24px rgba(0,0,0,.10);
      padding: 2.5rem 2rem;
      width: 100%;
      max-width: 380px;
      text-align: center;
    }}
    .gate .lock {{ font-size: 2.5rem; margin-bottom: .5rem; }}
    .gate h1 {{ font-size: 1.25rem; margin: 0 0 .4rem; }}
    .gate .subtitle {{ color: #666; font-size: .9rem; margin: 0 0 1.5rem; }}
    .gate .error {{
      color: #b91c1c;
      background: #fef2f2;
      border: 1px solid #fecaca;
      border-radius: 5px;
      padding: .5rem .75rem;
      margin-bottom: 1rem;
      font-size: .875rem;
    }}
    .gate input[type=password] {{
      width: 100%;
      padding: .65rem .9rem;
      border: 1px solid #d1d5db;
      border-radius: 6px;
      font-size: 1rem;
      margin-bottom: .75rem;
      outline: none;
      transition: border-color .15s;
    }}
    .gate input[type=password]:focus {{ border-color: #3b82f6; box-shadow: 0 0 0 3px rgba(59,130,246,.15); }}
    .gate button {{
      width: 100%;
      padding: .65rem;
      background: #3b82f6;
      color: #fff;
      border: none;
      border-radius: 6px;
      font-size: 1rem;
      cursor: pointer;
      font-weight: 500;
      transition: background .15s;
    }}
    .gate button:hover {{ background: #2563eb; }}
  </style>
</head>
<body>
  <div class="gate">
    <div class="lock">&#x1F512;</div>
    <h1>Protected Content</h1>
    <p class="subtitle">Enter the password to view <strong>{title}</strong>.</p>
    {error_html}
    <form method="POST" action="{action}">
      <input type="password" name="post_password" placeholder="Password" autofocus required>
      <button type="submit">View Content</button>
    </form>
  </div>
</body>
</html>"#,
        title = html_escape(post_title),
        action = html_escape(action),
        error_html = error_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
