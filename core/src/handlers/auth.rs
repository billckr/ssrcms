//! Authentication handlers: login form, login POST, logout.

use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::middleware::admin_auth::SESSION_USER_ID_KEY;

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

/// GET /admin/login — render login page.
pub async fn login_form() -> impl IntoResponse {
    Html(admin::pages::login::render(None))
}

/// POST /admin/login — verify credentials, create session.
pub async fn login_post(
    State(state): State<AppState>,
    session: Session,
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

    // Check role — super_admin, editor, and author can log in to admin.
    match user.role.as_str() {
        "super_admin" | "editor" | "author" => {}
        _ => {
            return Html(admin::pages::login::render(Some("Your account does not have admin access."))).into_response();
        }
    }

    // Store user ID in session.
    if let Err(e) = session.insert(SESSION_USER_ID_KEY, user.id.to_string()).await {
        tracing::error!("session insert error: {}", e);
        return Html(admin::pages::login::render(Some("Session error. Please try again."))).into_response();
    }

    Redirect::to("/admin").into_response()
}

/// GET /admin/logout — clear session, redirect to login.
pub async fn logout(session: Session) -> impl IntoResponse {
    let _ = session.flush().await;
    Redirect::to("/admin/login")
}
