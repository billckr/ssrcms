pub mod account;
pub mod admin;
pub mod archive;
pub mod auth;
pub mod comment;
pub mod form;
pub mod home;
pub mod metrics;
pub mod page;
pub mod plugin_route;
pub mod post;
pub mod post_unlock;
pub mod search;
pub mod subscribe;
pub mod theme_static;
pub mod uploads;

use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::account_auth::SESSION_ACCOUNT_USER_ID_KEY;
use crate::templates::context::{SessionContext, SessionUserContext};

/// Resolve the subscriber session into a `SessionContext` for Tera templates.
/// Never redirects — returns an anonymous context if the session is missing or invalid.
pub(super) async fn resolve_session(state: &AppState, session: &Session) -> SessionContext {
    let user_id_str: Option<String> = session
        .get(SESSION_ACCOUNT_USER_ID_KEY)
        .await
        .unwrap_or(None);
    if let Some(id_str) = user_id_str {
        if let Ok(uid) = id_str.parse::<Uuid>() {
            if let Ok(user) = crate::models::user::get_by_id(&state.db, uid).await {
                return SessionContext {
                    is_logged_in: true,
                    user: Some(SessionUserContext {
                        id: user.id.to_string(),
                        username: user.username.clone(),
                        display_name: user.display_name.clone(),
                        role: user.role.as_str().to_string(),
                    }),
                };
            }
        }
    }
    SessionContext { is_logged_in: false, user: None }
}
