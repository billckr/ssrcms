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
}

/// GET /admin/login — render login page.
pub async fn login_form() -> impl IntoResponse {
    Html(admin::pages::login::render(None))
}

/// POST /admin/login — verify credentials, create session.
pub async fn login_post(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Look up user by email.
    let user = match crate::models::user::get_by_email(&state.db, &form.email).await {
        Ok(u) => u,
        Err(_) => {
            return Html(admin::pages::login::render(Some("Invalid email or password."))).into_response();
        }
    };

    // Verify password.
    if !user.verify_password(&form.password) {
        return Html(admin::pages::login::render(Some("Invalid email or password."))).into_response();
    }

    // Check role — staff only. Subscribers must use /login.
    match user.role.as_str() {
        "super_admin" | "site_admin" | "editor" | "author" => {}
        "subscriber" => {
            return Html(admin::pages::login::render(
                Some("Subscriber accounts sign in at /login."),
            )).into_response();
        }
        _ => {
            return Html(admin::pages::login::render(
                Some("Your account does not have admin access."),
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
                            Some("Your account does not have access to this site."),
                        )).into_response();
                    }
                }
            }
            None => {
                return Html(admin::pages::login::render(
                    Some("No site found for this domain."),
                )).into_response();
            }
        }
    }

    // Store user ID in session.
    if let Err(e) = session.insert(SESSION_USER_ID_KEY, user.id.to_string()).await {
        tracing::error!("session insert error: {}", e);
        return Html(admin::pages::login::render(Some("Session error. Please try again."))).into_response();
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
    Query(q): Query<RedirectQuery>,
) -> impl IntoResponse {
    let redirect = q.redirect.as_deref();
    Html(admin::pages::login::render_public(None, redirect))
}

/// POST /login — subscriber login only.
/// Staff who try here get a message pointing them to /admin/login.
pub async fn public_login_post(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Preserve the redirect path through error re-renders.
    let redirect_val = if form.redirect.is_empty() { None } else { Some(form.redirect.as_str()) };

    let user = match crate::models::user::get_by_email(&state.db, &form.email).await {
        Ok(u) => u,
        Err(_) => return Html(admin::pages::login::render_public(Some("Invalid email or password."), redirect_val)).into_response(),
    };
    if !user.verify_password(&form.password) {
        return Html(admin::pages::login::render_public(Some("Invalid email or password."), redirect_val)).into_response();
    }
    match user.role.as_str() {
        "subscriber" => {}
        "super_admin" | "site_admin" | "editor" | "author" => {
            return Html(admin::pages::login::render_public(
                Some("Staff accounts sign in at /admin/login."),
                redirect_val,
            )).into_response();
        }
        _ => return Html(admin::pages::login::render_public(Some("Invalid email or password."), redirect_val)).into_response(),
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
                    redirect_val,
                )).into_response(),
            }
        }
        None => return Html(admin::pages::login::render_public(
            Some("No site found for this domain."),
            redirect_val,
        )).into_response(),
    }

    if let Err(e) = session.insert(SESSION_ACCOUNT_USER_ID_KEY, user.id.to_string()).await {
        tracing::error!("account login session insert error: {}", e);
        return Html(admin::pages::login::render_public(Some("Session error. Please try again."), redirect_val)).into_response();
    }

    // Redirect back to the page that sent the user to login, or fall back to /account.
    let destination = match redirect_val {
        Some(r) if r.starts_with('/') => r,
        _ => "/account",
    };
    Redirect::to(destination).into_response()
}

/// GET /admin/logout — clear admin session key only, redirect to admin login.
pub async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.remove::<String>(SESSION_USER_ID_KEY).await;
    let _ = session.remove::<String>(SESSION_CURRENT_SITE_KEY).await;
    Redirect::to("/admin/login")
}

/// GET /account/logout — clear account session key only, redirect to /login.
pub async fn account_logout(session: Session) -> impl IntoResponse {
    let _ = session.remove::<String>(SESSION_ACCOUNT_USER_ID_KEY).await;
    Redirect::to("/login")
}
