//! Account session guard — accepts any authenticated user (subscriber or above).
//!
//! Uses its own session key (`account_user_id`) entirely separate from the
//! admin session key (`admin_user_id`). This means logging in as a subscriber
//! via /login never touches the admin session, and vice-versa — two different
//! users can be "logged in" in different contexts in the same browser without
//! interfering with each other.

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::user::User;

/// Session key for the account area — kept separate from SESSION_USER_ID_KEY.
pub const SESSION_ACCOUNT_USER_ID_KEY: &str = "account_user_id";

/// An authenticated account user (any role) extracted from the session.
pub struct AccountUser {
    pub user: User,
    /// Site resolved from the Host header — None in single-site fallback mode.
    pub site_id: Option<Uuid>,
    /// Human-readable site name for display (e.g. "Back to Acme Blog").
    pub site_name: String,
    /// Base URL of the current site for "back to site" links.
    pub site_base_url: String,
}

pub enum AccountAuthError {
    NotAuthenticated,
    Internal(String),
}

impl IntoResponse for AccountAuthError {
    fn into_response(self) -> Response {
        match self {
            AccountAuthError::NotAuthenticated => Redirect::to("/login").into_response(),
            AccountAuthError::Internal(e) => {
                tracing::error!("account auth error: {}", e);
                Redirect::to("/login").into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for AccountUser {
    type Rejection = AccountAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or_else(|| AccountAuthError::Internal("session not found".into()))?
            .clone();

        let user_id_str: Option<String> = session
            .get(SESSION_ACCOUNT_USER_ID_KEY)
            .await
            .map_err(|e| AccountAuthError::Internal(format!("session get error: {e}")))?;

        let user_id_str = user_id_str.ok_or(AccountAuthError::NotAuthenticated)?;
        let user_id: Uuid = user_id_str.parse().map_err(|_| AccountAuthError::NotAuthenticated)?;

        let user = crate::models::user::get_by_id(&state.db, user_id)
            .await
            .map_err(|_| AccountAuthError::NotAuthenticated)?;

        // Resolve site from Host header.
        let raw_host = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost")
            .to_string();
        let hostname = {
            if let Some(pos) = raw_host.rfind(':') {
                if raw_host[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                    raw_host[..pos].to_string()
                } else {
                    raw_host.clone()
                }
            } else {
                raw_host.clone()
            }
        };

        let (site_id, site_name, site_base_url) =
            if let Some((site, settings)) = state.resolve_site(&hostname) {
                let base_url = if settings.base_url != "http://localhost:3000" {
                    settings.base_url.clone()
                } else {
                    format!("http://{}", raw_host)
                };
                (Some(site.id), settings.site_name.clone(), base_url)
            } else {
                let base_url = format!("http://{}", raw_host);
                (None, state.settings.site_name.clone(), base_url)
            };

        Ok(AccountUser { user, site_id, site_name, site_base_url })
    }
}
