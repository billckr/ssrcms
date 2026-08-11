//! Authentication handlers: login form, login POST, logout.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::middleware::account_auth::SESSION_ACCOUNT_USER_ID_KEY;
use crate::middleware::admin_auth::{SESSION_CURRENT_SITE_KEY, SESSION_USER_ID_KEY};

/// Extract bare hostname from a Host header value (strips port if present).
fn host_to_hostname(raw: &str) -> String {
    if let Some(pos) = raw.rfind(':') {
        if raw[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
            return raw[..pos].to_string();
        }
    }
    raw.to_string()
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub redirect: String,
}

#[derive(Deserialize)]
pub struct RedirectQuery {
    pub redirect: Option<String>,
    pub flash: Option<String>,
}

/// GET /admin/login — render login page.
pub async fn login_form(State(state): State<AppState>) -> impl IntoResponse {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    Html(admin::pages::login::render(None, &default_theme))
}

/// POST /admin/login — verify credentials, create session.
pub async fn login_post(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();

    // Look up user by email.
    let user = match crate::models::user::get_by_email(&state.db, &form.email).await {
        Ok(u) => u,
        Err(_) => {
            return Html(admin::pages::login::render(Some("Invalid email or password."), &default_theme)).into_response();
        }
    };

    // Verify password.
    if !user.verify_password(&form.password) {
        return Html(admin::pages::login::render(Some("Invalid email or password."), &default_theme)).into_response();
    }

    // Check role — staff only. Subscribers must use /login.
    match user.role.as_str() {
        "super_admin" | "site_admin" | "editor" | "author" => {}
        "subscriber" => {
            return Html(admin::pages::login::render(
                Some("Subscriber accounts sign in at /login."), &default_theme,
            )).into_response();
        }
        _ => {
            return Html(admin::pages::login::render(
                Some("Your account does not have admin access."), &default_theme,
            )).into_response();
        }
    }

    // ── Site resolution ───────────────────────────────────────────────────────
    // Resolve the site from the Host header so that logging in from
    // bckr.local:3000 lands on the bckr.local site, not whichever site
    // happens to be first in the database.
    let raw_host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost")
        .to_string();
    let hostname = host_to_hostname(&raw_host);

    tracing::info!("login: raw_host='{}' hostname='{}'", raw_host, hostname);
    let resolved_site = state.resolve_site(&hostname);
    tracing::info!("login: site resolved={}", resolved_site.is_some());

    // Non-super-admin users must have an explicit site_users row for this domain.
    if user.role.as_str() != "super_admin" {
        match &resolved_site {
            Some((site, _)) => {
                match crate::models::site_user::get_role(&state.db, site.id, user.id).await {
                    Ok(Some(_)) => {} // has access — continue
                    _ => {
                        return Html(admin::pages::login::render(
                            Some("Your account does not have access to this site."), &default_theme,
                        )).into_response();
                    }
                }
            }
            None => {
                return Html(admin::pages::login::render(
                    Some("No site found for this domain."), &default_theme,
                )).into_response();
            }
        }
    }

    // Store user ID in session.
    if let Err(e) = session.insert(SESSION_USER_ID_KEY, user.id.to_string()).await {
        tracing::error!("session insert error: {}", e);
        return Html(admin::pages::login::render(Some("Session error. Please try again."), &default_theme)).into_response();
    }
    tracing::info!("login: user_id stored in session for {}", form.email);

    // Store the resolved site in the session immediately so the AdminUser
    // extractor doesn't have to re-derive it from scratch on the next request.
    if let Some((site, _)) = resolved_site {
        tracing::info!("login: site_id stored in session: {} ({})", site.hostname, site.id);
        let _ = session.insert(SESSION_CURRENT_SITE_KEY, site.id.to_string()).await;
    } else {
        tracing::warn!("login: no site resolved for hostname '{}' — session will have no site_id", hostname);
    }

    Redirect::to("/admin").into_response()
}

/// GET /login — public-facing login form (for subscribers).
pub async fn public_login_form(
    State(state): State<AppState>,
    Query(q): Query<RedirectQuery>,
) -> impl IntoResponse {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    let redirect = q.redirect.as_deref();
    Html(admin::pages::login::render_public(None, q.flash.as_deref(), redirect, &default_theme))
}

/// POST /login — subscriber login only.
/// Staff who try here get a message pointing them to /admin/login.
pub async fn public_login_post(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();

    // Preserve the redirect path through error re-renders.
    let redirect_val = if form.redirect.is_empty() { None } else { Some(form.redirect.as_str()) };

    let user = match crate::models::user::get_by_email(&state.db, &form.email).await {
        Ok(u) => u,
        Err(_) => return Html(admin::pages::login::render_public(Some("Invalid email or password."), None, redirect_val, &default_theme)).into_response(),
    };
    if !user.verify_password(&form.password) {
        return Html(admin::pages::login::render_public(Some("Invalid email or password."), None, redirect_val, &default_theme)).into_response();
    }
    match user.role.as_str() {
        "subscriber" => {}
        // Deliberately the same generic message as a wrong password/unknown
        // email below, not "Staff accounts sign in at /admin/login." — this
        // branch only runs after a *correct* password match, so a distinct
        // message here would confirm to anyone testing credentials (e.g.
        // credential stuffing) that a given email/password pair belongs to a
        // staff account, which is useful targeting information even though
        // they already have valid creds for it.
        "super_admin" | "site_admin" | "editor" | "author" => {
            return Html(admin::pages::login::render_public(Some("Invalid email or password."), None, redirect_val, &default_theme)).into_response();
        }
        _ => return Html(admin::pages::login::render_public(Some("Invalid email or password."), None, redirect_val, &default_theme)).into_response(),
    }

    let raw_host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost")
        .to_string();
    let hostname = host_to_hostname(&raw_host);
    let resolved_site = state.resolve_site(&hostname);

    match &resolved_site {
        Some((site, _)) => {
            match crate::models::site_user::get_role(&state.db, site.id, user.id).await {
                Ok(Some(_)) => {}
                _ => return Html(admin::pages::login::render_public(
                    Some("Your account does not have access to this site."),
                    None, redirect_val, &default_theme,
                )).into_response(),
            }
        }
        None => return Html(admin::pages::login::render_public(
            Some("No site found for this domain."),
            None, redirect_val, &default_theme,
        )).into_response(),
    }

    if let Err(e) = session.insert(SESSION_ACCOUNT_USER_ID_KEY, user.id.to_string()).await {
        tracing::error!("account login session insert error: {}", e);
        return Html(admin::pages::login::render_public(Some("Session error. Please try again."), None, redirect_val, &default_theme)).into_response();
    }

    // Redirect back to the page that sent the user to login, or fall back to /account.
    let destination = match redirect_val {
        Some(r) if r.starts_with('/') => r,
        _ => "/account",
    };
    Redirect::to(destination).into_response()
}

/// GET /admin/logout — destroy the session (not just the auth key) and redirect to admin login.
///
/// `flush()` (not `remove()`) is required: tower_sessions' cookie-removal check only fires when
/// a request arrives with no session id at all, so merely removing keys leaves the record (and
/// cookie) alive and silently renews its expiry on save. `flush()` deletes the store row and
/// clears the session id, which does trigger cookie removal in the response.
pub async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    Redirect::to("/admin/login")
}

/// GET /account/logout — destroy the session and redirect to /login. See `logout` above for why
/// this uses `flush()` rather than removing the account key.
pub async fn account_logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    Redirect::to("/login")
}
