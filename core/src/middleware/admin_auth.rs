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

/// Session key where the currently selected site UUID is stored.
pub const SESSION_CURRENT_SITE_KEY: &str = "current_site_id";

/// Capabilities derived once at the authentication boundary.
/// Computed from global role + site role; passed downstream — never recomputed.
#[derive(Debug, Clone)]
pub struct AdminCaps {
    /// Agency-level super-admin with unrestricted cross-site access.
    pub is_global_admin: bool,
    /// Super-admin viewing a site they do not own.
    pub visiting_foreign_site: bool,
    /// Can view, create, edit, and delete users.
    pub can_manage_users: bool,
    /// Can create new sites and edit site-level settings.
    pub can_manage_sites: bool,
    /// Can activate, configure, and remove plugins.
    pub can_manage_plugins: bool,
    /// Can access system-level settings (super_admin on the default site only).
    pub can_manage_settings: bool,
    /// Can create, edit, publish, and delete content.
    pub can_manage_content: bool,
    /// Can manage themes (appearance).
    pub can_manage_appearance: bool,
    /// Can create, edit, and delete categories and tags.
    pub can_manage_taxonomies: bool,
    /// Can view, export, and delete form submissions.
    pub can_manage_forms: bool,
}

impl AdminCaps {
    /// Derive capabilities from the user's global role, their role on the current
    /// site, and whether a super-admin is visiting a foreign site.
    /// `is_on_default_site` must be true for `can_manage_settings` to be granted —
    /// system settings are restricted to super_admin on the system default site only.
    pub fn from_roles(
        global_role: &str,
        site_role: &str,
        visiting_foreign: bool,
        is_on_default_site: bool,
    ) -> Self {
        let is_global_admin = global_role == "super_admin";
        let is_admin = is_global_admin || site_role == "admin";
        let is_editor_or_above = is_admin || site_role == "editor";
        Self {
            is_global_admin,
            visiting_foreign_site: visiting_foreign,
            can_manage_users: is_admin,
            can_manage_sites: is_admin,
            can_manage_plugins: is_global_admin,
            can_manage_settings: is_global_admin && is_on_default_site,
            can_manage_content: true,
            can_manage_appearance: is_admin,
            can_manage_taxonomies: is_editor_or_above,
            can_manage_forms: is_admin,
        }
    }
}

/// An authenticated admin user extracted from the session.
/// Add this as a parameter to any admin handler to require authentication.
pub struct AdminUser {
    pub user: User,
    /// UUID of the currently selected site.  `None` when no sites are configured
    /// (single-site backward-compatibility mode).
    pub site_id: Option<Uuid>,
    /// The user's role on the current site, or their global role as fallback.
    /// Still needed for business logic (e.g. creating site_users rows).
    pub site_role: String,
    /// Derived capabilities — use these for all permission checks.
    pub caps: AdminCaps,
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

        let user_id_str = user_id_str.ok_or_else(|| {
            tracing::warn!("admin_auth: no user_id in session — redirecting to login");
            AdminAuthError::NotAuthenticated
        })?;

        let user_id: Uuid = user_id_str
            .parse()
            .map_err(|_| AdminAuthError::NotAuthenticated)?;

        // Fetch user from DB.
        let user = crate::models::user::get_by_id(&state.db, user_id)
            .await
            .map_err(|_| AdminAuthError::NotAuthenticated)?;

        // Super admin, site_admin, editor, and author roles can access the admin.
        match user.role.as_str() {
            "super_admin" | "site_admin" | "editor" | "author" => {}
            _ => return Err(AdminAuthError::Forbidden),
        }

        let is_global_admin = user.role.as_str() == "super_admin";

        // ── Site resolution ────────────────────────────────────────────────────

        // 1. Try to get the current site from the session, validating it still exists.
        //    Stale UUIDs arise when the session store survives a DB reset.
        let site_id_opt: Option<String> = session.get(SESSION_CURRENT_SITE_KEY).await.unwrap_or(None);
        let session_site_id: Option<Uuid> = if let Some(sid_str) = site_id_opt {
            if let Ok(uuid) = sid_str.parse::<Uuid>() {
                match crate::models::site::get_by_id(&state.db, uuid).await {
                    Ok(_) => Some(uuid),
                    Err(_) => {
                        // Site no longer exists — clear stale key and re-resolve below.
                        let _ = session.remove::<String>(SESSION_CURRENT_SITE_KEY).await;
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Extract hostname from the Host header for site resolution fallback.
        let request_hostname: Option<String> = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok())
            .map(|raw| {
                if let Some(pos) = raw.rfind(':') {
                    if raw[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                        return raw[..pos].to_string();
                    }
                }
                raw.to_string()
            });

        let site_id = if let Some(id) = session_site_id {
            Some(id)
        } else if is_global_admin {
            // 2a. Global admin — prefer the site matching the request's Host header
            //     so that logging in from bckr.local lands on the bckr.local site.
            //     Falls back to the first site in the DB for direct/localhost access.
            //
            // NOTE: resolve_site() uses the in-memory cache which can be stale after
            // a `dev reset`. We validate the cached result against the DB; on failure
            // we reload the cache so the next resolve attempt returns current data.
            let cached_site_id = request_hostname
                .as_deref()
                .and_then(|h| state.resolve_site(h))
                .map(|(s, _)| s.id);

            let host_site_id = match cached_site_id {
                Some(id) => {
                    match crate::models::site::get_by_id(&state.db, id).await {
                        Ok(_) => Some(id),
                        Err(_) => {
                            // Cache is stale (e.g. after dev reset) — reload and retry.
                            let _ = state.reload_site_cache().await;
                            request_hostname
                                .as_deref()
                                .and_then(|h| state.resolve_site(h))
                                .map(|(s, _)| s.id)
                        }
                    }
                }
                None => None,
            };

            let resolved = if host_site_id.is_some() {
                host_site_id
            } else {
                match crate::models::site::list(&state.db).await {
                    Ok(sites) if !sites.is_empty() => Some(sites[0].id),
                    _ => None,
                }
            };

            if let Some(id) = resolved {
                let _ = session.insert(SESSION_CURRENT_SITE_KEY, id.to_string()).await;
            }
            resolved
        } else {
            // 2b. Site user — look up their first accessible site.
            match crate::models::site_user::list_for_user(&state.db, user_id).await {
                Ok(sites) if !sites.is_empty() => {
                    let first_id = sites[0].0.id;
                    let _ = session
                        .insert(SESSION_CURRENT_SITE_KEY, first_id.to_string())
                        .await;
                    Some(first_id)
                }
                _ => None,
            }
        };

        // 3. Determine the role for the current site.
        let site_role = if is_global_admin {
            // Global admin always has full admin role on any site.
            "admin".to_string()
        } else if let Some(sid) = site_id {
            match crate::models::site_user::get_role(&state.db, sid, user_id).await {
                Ok(Some(r)) => r,
                _ => user.role.clone(),
            }
        } else {
            user.role.clone()
        };

        // Show the "visiting" badge when a super_admin is browsing any site other
        // than their own default/home site.  Using default_site_id (not owner_user_id)
        // because a super_admin typically creates every client site themselves, so
        // they technically "own" all of them — but they're still visiting as admin.
        let is_visiting_foreign_site = is_global_admin
            && site_id.is_some()
            && site_id != user.default_site_id;

        // System settings are only accessible to super_admin on their default/home site.
        // Uses the same default_site_id as the visiting badge so both stay in sync
        // when the super_admin changes their default site.
        let is_on_default_site = is_global_admin
            && user.default_site_id.is_some()
            && site_id == user.default_site_id;

        let caps = AdminCaps::from_roles(&user.role, &site_role, is_visiting_foreign_site, is_on_default_site);

        Ok(AdminUser { user, site_id, site_role, caps })
    }
}
