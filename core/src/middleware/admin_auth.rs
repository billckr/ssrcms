//! Admin session guard.
//!
//! `AdminUser` is an Axum extractor that reads the session, validates the admin
//! user_id stored in it, and returns `Err(Redirect to /admin/login)` if not found.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::user::User;

/// Session key where the logged-in user's UUID is stored.
pub const SESSION_USER_ID_KEY: &str = "admin_user_id";

/// An authenticated admin user extracted from the session.
/// Add this as a parameter to any admin handler to require authentication.
pub struct AdminUser {
    pub user: User,
}

pub enum AdminAuthError {
    NotAuthenticated,
    Forbidden,
    Internal(String),
}

impl IntoResponse for AdminAuthError {
    fn into_response(self) -> Response {
        match self {
            AdminAuthError::NotAuthenticated => {
                Redirect::to("/admin/login").into_response()
            }
            AdminAuthError::Forbidden => {
                (StatusCode::FORBIDDEN, "Forbidden").into_response()
            }
            AdminAuthError::Internal(e) => {
                tracing::error!("admin auth error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AdminAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        // Extract the session from request extensions.
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or_else(|| AdminAuthError::Internal("session not found in extensions — is SessionManagerLayer installed?".into()))?
            .clone();

        // Read the user ID from the session.
        let user_id_str: Option<String> = session
            .get(SESSION_USER_ID_KEY)
            .await
            .map_err(|e| AdminAuthError::Internal(format!("session get error: {e}")))?;

        let user_id_str = user_id_str.ok_or(AdminAuthError::NotAuthenticated)?;

        let user_id: Uuid = user_id_str
            .parse()
            .map_err(|_| AdminAuthError::NotAuthenticated)?;

        // Fetch user from DB.
        let user = crate::models::user::get_by_id(&state.db, user_id)
            .await
            .map_err(|_| AdminAuthError::NotAuthenticated)?;

        // Only Admin and Editor roles can access the admin.
        match user.role.as_str() {
            "admin" | "editor" => Ok(AdminUser { user }),
            _ => Err(AdminAuthError::Forbidden),
        }
    }
}
