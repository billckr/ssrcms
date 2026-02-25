use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::user::{CreateUser, UpdateUser, UserRole};
use admin::pages::users::{SiteOption, UserEdit, UserRow};

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let rows: Vec<UserRow> = if admin.caps.is_global_admin {
        // For super_admin: fetch all users, but show their role in the current
        // site context (site_users) when available, else fall back to users.role.
        let users = crate::models::user::list(&state.db).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list users: {:?}", e);
            vec![]
        });
        // Build a site_users map for the current site if one is set.
        let site_role_map: std::collections::HashMap<uuid::Uuid, String> =
            if let Some(sid) = admin.site_id {
                crate::models::site_user::list_for_site(&state.db, sid)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(u, r)| (u.id, r))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };
        users.iter().map(|u| UserRow {
            id: u.id.to_string(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: site_role_map.get(&u.id).cloned().unwrap_or_else(|| u.role.clone()),
            display_name: u.display_name.clone(),
            is_protected: u.is_protected,
            is_super_admin: u.role == "super_admin",
        }).collect()
    } else if let Some(site_id) = admin.site_id {
        crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list site users: {:?}", e);
            vec![]
        }).into_iter().filter(|(u, _)| u.role != "super_admin").map(|(u, site_role)| UserRow {
            id: u.id.to_string(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: site_role.clone(),
            display_name: u.display_name.clone(),
            is_protected: u.is_protected,
            is_super_admin: false,
        }).collect()
    } else {
        vec![]
    };
    let current_user_id = admin.user.id.to_string();
    let can_manage_access = admin.caps.can_manage_users;
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::users::render_list(&rows, None, &current_user_id, can_manage_access, &ctx)).into_response()
}

pub async fn new_user(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let sites = if admin.caps.is_global_admin {
        fetch_site_options(&state).await
    } else {
        vec![]
    };
    let edit = UserEdit {
        id: None,
        username: String::new(),
        email: String::new(),
        display_name: String::new(),
        role: "author".into(),
        bio: String::new(),
        sites,
        is_super_admin_target: false,
    };
    Html(admin::pages::users::render_editor(&edit, None, &ctx)).into_response()
}

pub async fn edit_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::get_role(&state.db, sid, id)
                .await.ok().flatten().is_some(),
            None => false,
        };
        if !allowed {
            return Redirect::to("/admin/users").into_response();
        }
    }

    let user = match crate::models::user::get_by_id(&state.db, id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("user {} not found for editing: {:?}", id, e);
            return Redirect::to("/admin/users").into_response();
        }
    };

    // Site admins may not edit super_admin accounts.
    if !admin.caps.is_global_admin && user.role == "super_admin" {
        return Redirect::to("/admin/users").into_response();
    }

    let is_super_admin_target = user.role.as_str() == "super_admin";

    // For non-super-admin targets, show site role (admin/editor/author/subscriber) in the form.
    let display_role = if is_super_admin_target {
        user.role.clone()
    } else if let Some(sid) = admin.site_id {
        crate::models::site_user::get_role(&state.db, sid, id)
            .await.ok().flatten().unwrap_or_else(|| user.role.clone())
    } else {
        user.role.clone()
    };

    let edit = UserEdit {
        id: Some(user.id.to_string()),
        username: user.username.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: display_role,
        bio: user.bio.clone(),
        sites: vec![],
        is_super_admin_target,
    };
    Html(admin::pages::users::render_editor(&edit, None, &ctx)).into_response()
}

#[derive(Deserialize)]
pub struct UserForm {
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub password: Option<String>,
    pub role: String,
    pub bio: Option<String>,
    /// "existing" or "new" — only present on the new-user form for global admins.
    pub site_assignment: Option<String>,
    pub existing_site_id: Option<String>,
    pub new_hostname: Option<String>,
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let password = match form.password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => {
            let sites = if admin.caps.is_global_admin { fetch_site_options(&state).await } else { vec![] };
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites,
                is_super_admin_target: false,
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("Password is required for new users."),
                &ctx,
            )).into_response();
        }
    };

    // site_role: what goes into site_users.role (admin/editor/author/subscriber).
    // Site admins cannot assign super_admin; cap to editor.
    let site_role = if !admin.caps.is_global_admin && form.role == "super_admin" {
        "editor"
    } else {
        form.role.as_str()
    };
    // users_role: what goes into users.role. "admin" is a site_users concept, stored
    // as "site_admin" in users.role. "super_admin" is CLI-only.
    let users_role_str = match site_role {
        "admin" => "site_admin",
        "super_admin" => "site_admin",
        other => other,
    };
    let role = parse_role(users_role_str);
    let create = CreateUser {
        username: form.username.clone(),
        email: form.email.clone(),
        display_name: form.display_name.clone().filter(|s| !s.is_empty()).unwrap_or_default(),
        password,
        role,
    };

    match crate::models::user::create(&state.db, &create).await {
        Ok(new_user) => {
            if admin.caps.is_global_admin {
                // Resolve target site: create new or use existing.
                let site_id = match form.site_assignment.as_deref() {
                    Some("new") => {
                        let hostname = form.new_hostname.as_deref().unwrap_or("").trim().to_lowercase();
                        if hostname.is_empty() {
                            tracing::warn!("new user {} created but no hostname provided for new site", new_user.id);
                            None
                        } else {
                            match crate::models::site::create(&state.db, &hostname).await {
                                Ok(site) => {
                                    if let Err(e) = state.reload_site_cache().await {
                                        tracing::warn!("site cache reload failed: {:?}", e);
                                    }
                                    // If assigning as admin, claim ownership of the new site.
                                    if site_role == "admin" {
                                        let _ = sqlx::query(
                                            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL",
                                        )
                                        .bind(new_user.id)
                                        .bind(site.id)
                                        .execute(&state.db)
                                        .await;
                                    }
                                    Some(site.id)
                                }
                                Err(e) => {
                                    tracing::error!("failed to create site '{}': {:?}", hostname, e);
                                    None
                                }
                            }
                        }
                    }
                    _ => {
                        // "existing" or unset — use the selected site id.
                        form.existing_site_id
                            .as_deref()
                            .and_then(|s| s.parse::<Uuid>().ok())
                    }
                };
                if let Some(sid) = site_id {
                    if let Err(e) = crate::models::site_user::add(&state.db, sid, new_user.id, site_role, None).await {
                        tracing::warn!("failed to add user {} to site {}: {:?}", new_user.id, sid, e);
                    }
                    // If assigning as admin and the site has no owner yet, claim ownership.
                    if site_role == "admin" {
                        let _ = sqlx::query(
                            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL",
                        )
                        .bind(new_user.id)
                        .bind(sid)
                        .execute(&state.db)
                        .await;
                        // Set the new user's default site.
                        let _ = crate::models::user::set_default_site(&state.db, new_user.id, Some(sid)).await;
                    }
                }
            } else {
                // Site admin: auto-scope to their site, record who invited.
                if let Some(site_id) = admin.site_id {
                    if let Err(e) = crate::models::site_user::add(&state.db, site_id, new_user.id, site_role, Some(admin.user.id)).await {
                        tracing::warn!("failed to add new user {} to site {}: {:?}", new_user.id, site_id, e);
                    }
                    // If new user is an admin, set their default site.
                    if site_role == "admin" {
                        let _ = crate::models::user::set_default_site(&state.db, new_user.id, Some(site_id)).await;
                    }
                }
            }
            Redirect::to("/admin/users").into_response()
        }
        Err(e) => {
            tracing::error!("create user error: {:?}", e);
            let sites = if admin.caps.is_global_admin { fetch_site_options(&state).await } else { vec![] };
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites,
                is_super_admin_target: false,
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::get_role(&state.db, sid, id)
                .await.ok().flatten().is_some(),
            None => false,
        };
        if !allowed {
            return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
        }
    }

    // Fetch target to know their current role (preserve super_admin, update site_users.role).
    let target_role = crate::models::user::get_by_id(&state.db, id).await
        .map(|u| u.role.clone()).unwrap_or_default();
    let is_super_admin_target = target_role == "super_admin";

    // Site admins may not edit super_admin accounts.
    if !admin.caps.is_global_admin && is_super_admin_target {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let new_password_hash = if let Some(pw) = form.password.as_deref().filter(|p| !p.is_empty()) {
        match crate::models::user::hash_password(pw) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::error!("password hashing error for user {}: {:?}", id, e);
                let edit = UserEdit {
                    id: Some(id.to_string()),
                    username: form.username,
                    email: form.email,
                    display_name: form.display_name.unwrap_or_default(),
                    role: form.role,
                    bio: form.bio.unwrap_or_default(),
                    sites: vec![],
                    is_super_admin_target,
                };
                return Html(admin::pages::users::render_editor(
                    &edit,
                    Some("Failed to process password. Please try again."),
                    &ctx,
                )).into_response();
            }
        }
    } else {
        None
    };

    // Determine users.role to write:
    // - super_admin targets keep their role (never downgraded via form)
    // - "admin" is a site_users role concept; map to "site_admin" in users table
    // - "super_admin" cannot be set via form (CLI-only)
    let new_users_role = if is_super_admin_target {
        parse_role("super_admin")
    } else {
        match form.role.as_str() {
            "admin" | "super_admin" => parse_role("site_admin"),
            other => parse_role(other),
        }
    };

    let update = UpdateUser {
        username: Some(form.username.clone()),
        email: Some(form.email.clone()),
        display_name: form.display_name.clone(),
        password_hash: new_password_hash,
        role: Some(new_users_role),
        bio: form.bio.clone(),
    };

    // Also sync the site_users.role for non-super-admin users.
    if !is_super_admin_target {
        if let Some(site_id) = admin.site_id {
            let site_role = if form.role == "super_admin" { "editor" } else { &form.role };
            let _ = crate::models::site_user::update_role(&state.db, site_id, id, site_role).await;
        }
    }

    match crate::models::user::update(&state.db, id, &update).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(e) => {
            tracing::error!("update user {} error: {:?}", id, e);
            let edit = UserEdit {
                id: Some(id.to_string()),
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites: vec![],
                is_super_admin_target,
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let current_user_id = admin.user.id.to_string();
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! deny {
        ($msg:expr) => {{
            tracing::warn!("delete_user denied for target={} actor={}: {}", id, admin.user.id, $msg);
            let rows: Vec<UserRow> = if admin.caps.is_global_admin {
                crate::models::user::list(&state.db).await.unwrap_or_default()
                    .iter().map(|u| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: u.role.clone(),
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                        is_super_admin: u.role == "super_admin",
                    }).collect()
            } else if let Some(site_id) = admin.site_id {
                crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_default()
                    .into_iter().filter(|(u, _)| u.role != "super_admin").map(|(u, site_role)| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: site_role,
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                        is_super_admin: false,
                    }).collect()
            } else {
                vec![]
            };
            let can_manage_access = admin.caps.can_manage_users;
            return Html(admin::pages::users::render_list(
                &rows,
                Some($msg),
                &current_user_id,
                can_manage_access,
                &ctx,
            )).into_response();
        }};
    }

    // Guard 1: no self-deletion.
    if id == admin.user.id {
        deny!("You cannot delete your own account.");
    }

    // Guard 2: cannot delete a protected account.
    let target = crate::models::user::get_by_id(&state.db, id).await;
    if let Ok(ref t) = target {
        if t.is_protected {
            deny!("This account is protected and cannot be deleted.");
        }
    }

    // Guard 3: only a global admin may delete another global admin.
    if let Ok(ref t) = target {
        if t.role == "super_admin" && !admin.caps.is_global_admin {
            deny!("Only a global admin can delete another global admin account.");
        }
    }

    // Guard 4: never delete the last global admin.
    if let Ok(ref t) = target {
        if t.role == "super_admin" {
            let remaining = crate::models::user::count_global_admins(&state.db)
                .await
                .unwrap_or(2);
            if remaining <= 1 {
                deny!("Cannot delete the last global admin account.");
            }
        }
    }

    if let Err(e) = crate::models::user::delete_and_reassign(&state.db, id, admin.user.id).await {
        tracing::error!("delete user {} error: {:?}", id, e);
    }
    Redirect::to("/admin/users").into_response()
}

fn friendly_user_error(e: &crate::errors::AppError) -> String {
    let s = e.to_string();
    if s.contains("duplicate key") || s.contains("unique") {
        "A user with that username or email already exists.".to_string()
    } else {
        "Failed to save user. Please try again.".to_string()
    }
}

fn parse_role(s: &str) -> UserRole {
    match s {
        "super_admin" => UserRole::SuperAdmin,
        "site_admin" => UserRole::SiteAdmin,
        "editor" => UserRole::Editor,
        "author" => UserRole::Author,
        _ => UserRole::Subscriber,
    }
}

async fn fetch_site_options(state: &AppState) -> Vec<SiteOption> {
    crate::models::site::list(&state.db).await
        .unwrap_or_else(|e| { tracing::warn!("failed to list sites for user form: {:?}", e); vec![] })
        .into_iter()
        .map(|s| SiteOption { id: s.id.to_string(), hostname: s.hostname })
        .collect()
}

// ── Site access management ────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct SiteAccessQuery {
    pub error: Option<String>,
}

/// GET /admin/users/:id/site-access — manage which sites a user can access.
/// Accessible to super_admin and site_admin only.
pub async fn site_access_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<SiteAccessQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let target_user = match crate::models::user::get_by_id(&state.db, user_id).await {
        Ok(u) => u,
        Err(_) => return Html("<h1>User not found</h1>".to_string()).into_response(),
    };

    // Super admin cannot be assigned to individual sites.
    if target_user.role == "super_admin" {
        return Html("<h1>Super admins have global access and cannot be assigned to individual sites.</h1>".to_string()).into_response();
    }

    // Current site assignments for the target user.
    let assignments = crate::models::site_user::list_for_user(&state.db, user_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(s, role)| admin::pages::users::SiteAssignmentRow {
            site_id: s.id.to_string(),
            hostname: s.hostname.clone(),
            role,
        })
        .collect::<Vec<_>>();

    // Available sites for this admin to assign to: all for super_admin, owned for site_admin.
    let available_sites: Vec<SiteOption> = if admin.caps.is_global_admin {
        fetch_site_options(&state).await
    } else {
        crate::models::site::list_by_owner(&state.db, admin.user.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| SiteOption { id: s.id.to_string(), hostname: s.hostname })
            .collect()
    };

    let data = admin::pages::users::SiteAccessData {
        user_id: user_id.to_string(),
        display_name: target_user.display_name.clone(),
        email: target_user.email.clone(),
        assignments,
        available_sites,
    };

    let flash = match query.error.as_deref() {
        Some("site_admin_exists") => Some("This site already has a Site Admin. Remove the existing Site Admin first."),
        _ => None,
    };

    Html(admin::pages::users::render_site_access(
        &data,
        flash,
        &ctx,
    )).into_response()
}

#[derive(Deserialize)]
pub struct SiteAccessAddForm {
    pub site_id: String,
    pub role: String,
}

/// POST /admin/users/:id/site-access/add — assign a user to a site.
pub async fn add_site_access(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<SiteAccessAddForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Ok(site_uuid) = form.site_id.parse::<Uuid>() else {
        return Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response();
    };

    // For site_admin: verify they own the target site.
    if !admin.caps.is_global_admin {
        let owned = crate::models::site::get_by_id(&state.db, site_uuid).await
            .ok()
            .and_then(|s| s.owner_user_id) == Some(admin.user.id);
        if !owned {
            return (axum::http::StatusCode::FORBIDDEN, "You do not own that site.").into_response();
        }
    }

    // Sanitise role.
    // site_admin may only be assigned by a global_admin and only if the site has no owner yet.
    let role = match form.role.as_str() {
        "site_admin" if admin.caps.is_global_admin => {
            // Enforce one site_admin per site.
            let site = crate::models::site::get_by_id(&state.db, site_uuid).await
                .map_err(|_| ()).ok();
            if site.as_ref().and_then(|s| s.owner_user_id).is_some() {
                // Flash error back to the page.
                return Redirect::to(&format!(
                    "/admin/users/{}/site-access?error=site_admin_exists", user_id
                )).into_response();
            }
            // Update owner_user_id on the site and promote user's global role.
            let _ = sqlx::query(
                "UPDATE sites SET owner_user_id = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(user_id)
            .bind(site_uuid)
            .execute(&state.db)
            .await;
            let _ = sqlx::query(
                "UPDATE users SET role = 'site_admin' WHERE id = $1 AND role NOT IN ('super_admin', 'site_admin')"
            )
            .bind(user_id)
            .execute(&state.db)
            .await;
            "admin" // site_users role for a site_admin is 'admin'
        }
        "editor" | "author" | "subscriber" => form.role.as_str(),
        _ => "editor",
    };

    if let Err(e) = crate::models::site_user::add(&state.db, site_uuid, user_id, role, Some(admin.user.id)).await {
        tracing::warn!("failed to add user {} to site {}: {:?}", user_id, site_uuid, e);
    }

    // Reload cache so ownership change is immediately reflected.
    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after site-access add: {:?}", e);
    }

    Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response()
}

#[derive(Deserialize)]
pub struct SiteAccessRemoveForm {
    pub site_id: String,
}

/// POST /admin/users/:id/site-access/remove — remove a user from a site.
pub async fn remove_site_access(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<SiteAccessRemoveForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Ok(site_uuid) = form.site_id.parse::<Uuid>() else {
        return Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response();
    };

    // For site_admin: verify they own the target site.
    if !admin.caps.is_global_admin {
        let owned = crate::models::site::get_by_id(&state.db, site_uuid).await
            .ok()
            .and_then(|s| s.owner_user_id) == Some(admin.user.id);
        if !owned {
            return (axum::http::StatusCode::FORBIDDEN, "You do not own that site.").into_response();
        }
    }

    if let Err(e) = crate::models::site_user::remove(&state.db, site_uuid, user_id).await {
        tracing::warn!("failed to remove user {} from site {}: {:?}", user_id, site_uuid, e);
    }

    // If this user was the site owner, clear owner_user_id so the site
    // can have a new site_admin assigned.
    let _ = sqlx::query(
        "UPDATE sites SET owner_user_id = NULL, updated_at = NOW() WHERE id = $1 AND owner_user_id = $2"
    )
    .bind(site_uuid)
    .bind(user_id)
    .execute(&state.db)
    .await;

    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after site-access remove: {:?}", e);
    }

    Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response()
}
