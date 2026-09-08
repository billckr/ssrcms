//! Account session guard — accepts any authenticated user (subscriber or above).
//!
//! Uses its own session key (`account_user_id`) entirely separate from the
//! admin session key (`admin_user_id`). This means logging in as a subscriber
//! via /login never touches the admin session, and vice-versa — two different
//! users can be "logged in" in different contexts in the same browser without
//! interfering with each other.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, Uri},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::user::User;

/// Session key for the account area — kept separate from SESSION_USER_ID_KEY.
pub const SESSION_ACCOUNT_USER_ID_KEY: &str = "account_user_id";
pub const SESSION_ACCOUNT_CREDENTIAL_VERSION_KEY: &str = "account_credential_version";
pub const SESSION_ACCOUNT_LOGIN_AT_KEY: &str = "account_login_at";
const ACCOUNT_ABSOLUTE_SESSION_SECONDS: i64 = 7 * 24 * 60 * 60;

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
    NotAuthenticated(String),
    Internal { message: String, login_url: String },
}

impl IntoResponse for AccountAuthError {
    fn into_response(self) -> Response {
        match self {
            AccountAuthError::NotAuthenticated(login_url) => {
                Redirect::to(&login_url).into_response()
            }
            AccountAuthError::Internal { message, login_url } => {
                tracing::error!("account auth error: {}", message);
                Redirect::to(&login_url).into_response()
            }
        }
    }
}

fn login_url_for_uri(uri: &Uri) -> String {
    let return_to = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    login_url_for_return_to(return_to)
}

pub(crate) fn login_url_for_return_to(return_to: &str) -> String {
    format!("/login?redirect={}", encode_query_value(return_to))
}

fn encode_query_value(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }

    encoded
}

/// Resolve and fully validate an account session without redirecting.
///
/// Public pages and public mutations use this same boundary as `AccountUser`
/// so absolute expiry and password-change revocation cannot be bypassed by a
/// handler that only needs an optional identity.
pub(crate) async fn validated_account_user(
    state: &AppState,
    session: &Session,
) -> Result<Option<User>, String> {
    let user_id_str: Option<String> = session
        .get(SESSION_ACCOUNT_USER_ID_KEY)
        .await
        .map_err(|e| format!("session get error: {e}"))?;
    let Some(user_id_str) = user_id_str else {
        return Ok(None);
    };

    let login_at: Option<i64> = session
        .get(SESSION_ACCOUNT_LOGIN_AT_KEY)
        .await
        .map_err(|e| format!("session login-time error: {e}"))?;
    if login_at
        .map(|at| {
            chrono::Utc::now().timestamp().saturating_sub(at) > ACCOUNT_ABSOLUTE_SESSION_SECONDS
        })
        .unwrap_or(true)
    {
        let _ = session.flush().await;
        return Ok(None);
    }

    let Ok(user_id) = user_id_str.parse::<Uuid>() else {
        let _ = session.flush().await;
        return Ok(None);
    };
    let Ok(user) = crate::models::user::get_by_id(&state.db, user_id).await else {
        let _ = session.flush().await;
        return Ok(None);
    };

    let session_credential_version: Option<String> = session
        .get(SESSION_ACCOUNT_CREDENTIAL_VERSION_KEY)
        .await
        .map_err(|e| format!("session credential check error: {e}"))?;
    if session_credential_version.as_deref() != Some(user.credential_version().as_str()) {
        let _ = session.flush().await;
        return Ok(None);
    }

    Ok(Some(user))
}

/// Validate both the account session and the user's continuing membership in
/// the site receiving the request. Removing a user from a site takes effect on
/// their next request instead of waiting for the session to expire.
pub(crate) async fn validated_account_user_for_site(
    state: &AppState,
    session: &Session,
    site_id: Uuid,
) -> Result<Option<User>, String> {
    let Some(user) = validated_account_user(state, session).await? else {
        return Ok(None);
    };
    let has_access = crate::models::site_user::has_any_role(&state.db, site_id, user.id)
        .await
        .map_err(|e| format!("account site access check error: {e}"))?;
    if !has_access {
        let _ = session.flush().await;
        return Ok(None);
    }
    Ok(Some(user))
}

impl FromRequestParts<AppState> for AccountUser {
    type Rejection = AccountAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let login_url = login_url_for_uri(&parts.uri);
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or_else(|| AccountAuthError::Internal {
                message: "session not found".into(),
                login_url: login_url.clone(),
            })?
            .clone();

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

        let user = match site_id {
            Some(site_id) => validated_account_user_for_site(state, &session, site_id).await,
            None => validated_account_user(state, &session).await,
        }
        .map_err(|message| AccountAuthError::Internal {
            message,
            login_url: login_url.clone(),
        })?
        .ok_or_else(|| AccountAuthError::NotAuthenticated(login_url))?;

        Ok(AccountUser {
            user,
            site_id,
            site_name,
            site_base_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_query_value, login_url_for_uri};

    #[test]
    fn login_url_preserves_and_encodes_the_original_path() {
        let uri = "/account/profile?tab=security&from=email"
            .parse()
            .expect("valid URI");

        assert_eq!(
            login_url_for_uri(&uri),
            "/login?redirect=%2Faccount%2Fprofile%3Ftab%3Dsecurity%26from%3Demail"
        );
    }

    #[test]
    fn query_value_encoding_handles_unicode_and_reserved_characters() {
        assert_eq!(encode_query_value("/saved café"), "%2Fsaved%20caf%C3%A9");
    }
}
