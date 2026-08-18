//! Login-time role picker, shown when a user holds more than one role on
//! the current site. Deliberately does NOT use the `AdminUser` extractor —
//! that extractor itself redirects here when a role pick is required, so
//! using it on this route would recurse.

use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::middleware::admin_auth::{PickRoleUser, SESSION_CURRENT_ROLE_KEY};
use crate::models::site_user::SiteRole;

/// GET /admin/pick-role
pub async fn show(
    State(state): State<AppState>,
    picker: PickRoleUser,
) -> impl IntoResponse {
    let Some(site_id) = picker.site_id else {
        return Redirect::to("/admin").into_response();
    };

    let roles = crate::models::site_user::list_roles_for_user_and_site(&state.db, site_id, picker.user.id)
        .await
        .unwrap_or_default();

    // Nothing to pick (0 or 1 role) — nothing for this page to do, send them on.
    if roles.len() < 2 {
        return Redirect::to("/admin").into_response();
    }

    let hostname = state.site_hostname(Some(site_id));
    let role_strs: Vec<&str> = roles.iter().map(|r| r.as_str()).collect();
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    Html(admin::pages::role_picker::render(&role_strs, &hostname, &default_theme)).into_response()
}

#[derive(Deserialize)]
pub struct PickRoleForm {
    pub role: String,
}

/// POST /admin/pick-role
pub async fn submit(
    State(state): State<AppState>,
    picker: PickRoleUser,
    session: Session,
    Form(form): Form<PickRoleForm>,
) -> impl IntoResponse {
    let Some(site_id) = picker.site_id else {
        return Redirect::to("/admin");
    };

    // Never trust the posted value blindly — re-validate against the roles
    // the user actually holds on this site before pinning it.
    let roles = crate::models::site_user::list_roles_for_user_and_site(&state.db, site_id, picker.user.id)
        .await
        .unwrap_or_default();

    if let Some(role) = SiteRole::from_str(&form.role) {
        if roles.contains(&role) {
            let _ = session.insert(SESSION_CURRENT_ROLE_KEY, role.as_str()).await;
        } else {
            tracing::warn!(
                "pick-role: user {} attempted to pin role {:?} they do not hold on site {}",
                picker.user.id, form.role, site_id
            );
        }
    }

    Redirect::to("/admin")
}
