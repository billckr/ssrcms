use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::user::{CreateUser, UpdateUser, UserRole};
use admin::pages::users::{SiteOption, UserEdit, UserRow};

#[derive(Deserialize, Default)]
pub struct UsersTabQuery {
    #[serde(default)]
    pub tab: String,
    /// Optional site UUID to filter the user list (super_admin only).
    #[serde(default)]
    pub site: String,
}

/// Split a flat list of UserRows into (staff, subscribers).
/// Staff = any role that is not "subscriber".
fn split_by_role(rows: Vec<UserRow>) -> (Vec<UserRow>, Vec<UserRow>) {
    let mut staff = Vec::new();
    let mut subs  = Vec::new();
    for r in rows {
        if r.role == "subscriber" { subs.push(r); } else { staff.push(r); }
    }
    (staff, subs)
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<UsersTabQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);

    // Fetch available sites for the filter dropdown (global admin only).
    let available_sites = if admin.caps.is_global_admin {
        fetch_site_options(&state).await
    } else {
        vec![]
    };

    let rows: Vec<UserRow> = if admin.caps.is_global_admin {
        let filter_site = q.site.parse::<Uuid>().ok();
        if let Some(filter_site_id) = filter_site {
            // Filtered: show only users assigned to this specific site.
            crate::models::site_user::list_for_site(&state.db, filter_site_id)
                .await.unwrap_or_else(|e| {
                    tracing::warn!("failed to list site users for filter: {:?}", e);
                    vec![]
                })
                .into_iter()
                .map(|(u, site_role)| UserRow {
                    id: u.id.to_string(),
                    username: u.username.clone(),
                    email: u.email.clone(),
                    role: site_role,
                    display_name: u.display_name.clone(),
                    is_protected: u.is_protected,
                    is_super_admin: u.role == "super_admin",
                    site_hostnames: vec![],
                })
                .collect()
        } else {
            // All sites: show every user, with site-context role when available.
            let users = crate::models::user::list(&state.db).await.unwrap_or_else(|e| {
                tracing::warn!("failed to list users: {:?}", e);
                vec![]
            });
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
                site_hostnames: vec![],
            }).collect()
        }
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
            site_hostnames: vec![],
        }).collect()
    } else {
        vec![]
    };

    let current_user_id = admin.user.id.to_string();
    // Exclude the currently logged-in user — they manage their own account via /admin/profile.
    let rows: Vec<_> = rows.into_iter().filter(|u| u.id != current_user_id).collect();
    let can_manage_access = admin.caps.can_manage_users;
    let active_tab = if q.tab == "subscribers" { "subscribers" } else { "site-users" };
    let (staff, mut subscribers) = split_by_role(rows);

    // Populate site_hostnames for subscribers: fetch all site memberships in one query.
    if !subscribers.is_empty() {
        let sub_ids: Vec<Uuid> = subscribers.iter()
            .filter_map(|u| u.id.parse::<Uuid>().ok())
            .collect();
        let hostname_rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT su.user_id, s.hostname \
             FROM site_users su \
             JOIN sites s ON s.id = su.site_id \
             WHERE su.user_id = ANY($1) \
             ORDER BY s.created_at ASC",
        )
        .bind(&sub_ids)
        .fetch_all(&state.db)
        .await
        .unwrap_or_else(|e| { tracing::warn!("failed to fetch subscriber sites: {:?}", e); vec![] });

        let mut hostname_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (uid, hostname) in hostname_rows {
            hostname_map.entry(uid.to_string()).or_default().push(hostname);
        }
        for sub in &mut subscribers {
            if let Some(hostnames) = hostname_map.remove(&sub.id) {
                sub.site_hostnames = hostnames;
            }
        }
    }

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::users::render_list(
        &staff, &subscribers, None, &current_user_id,
        can_manage_access, active_tab, &available_sites, &q.site, &ctx,
    )).into_response()
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

    // Validate password requirements.
    if let Err(msg) = crate::models::user::validate_password(&password) {
        let sites = if admin.caps.is_global_admin { fetch_site_options(&state).await } else { vec![] };
        let edit = UserEdit {
            id: None,
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites,
            is_super_admin_target: false,
        };
        return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
    }

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
                    Some("none") | None => None,
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
                        // "existing" — use the selected site id.
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

    // Validate password requirements if a new password was supplied.
    if let Some(pw) = form.password.as_deref().filter(|p| !p.is_empty()) {
        if let Err(msg) = crate::models::user::validate_password(pw) {
            let edit = UserEdit {
                id: Some(id.to_string()),
                username: form.username.clone(),
                email: form.email.clone(),
                display_name: form.display_name.clone().unwrap_or_default(),
                role: form.role.clone(),
                bio: form.bio.clone().unwrap_or_default(),
                sites: vec![],
                is_super_admin_target,
            };
            return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
        }
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
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let tab = form.get("tab").map(|s| s.as_str()).unwrap_or("site-users");
    let redirect_url = format!("/admin/users?tab={}", tab);
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
                        site_hostnames: vec![],
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
                        site_hostnames: vec![],
                    }).collect()
            } else {
                vec![]
            };
            let can_manage_access = admin.caps.can_manage_users;
            let (staff, subscribers) = split_by_role(rows);
            return Html(admin::pages::users::render_list(
                &staff,
                &subscribers,
                Some($msg),
                &current_user_id,
                can_manage_access,
                "site-users",
                &[],
                "",
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
        deny!("Failed to delete user. Please try again.");
    }
    Redirect::to(&redirect_url).into_response()
}

#[derive(Deserialize)]
pub struct BulkDeleteUsersForm {
    pub ids: String,
    pub tab: Option<String>,
}

pub async fn bulk_delete_users(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<BulkDeleteUsersForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Redirect::to("/admin/users").into_response();
    }
    let tab = form.tab.as_deref().unwrap_or("site-users");
    let redirect_url = format!("/admin/users?tab={}", tab);
    let ids: Vec<String> = form.ids.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for raw_id in &ids {
        let id = match raw_id.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => continue,
        };
        // Never self-delete.
        if id == admin.user.id { continue; }
        let target = match crate::models::user::get_by_id(&state.db, id).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        if target.is_protected { continue; }
        if target.role == "super_admin" && !admin.caps.is_global_admin { continue; }
        if target.role == "super_admin" {
            let remaining = crate::models::user::count_global_admins(&state.db).await.unwrap_or(2);
            if remaining <= 1 { continue; }
        }
        if let Err(e) = crate::models::user::delete_and_reassign(&state.db, id, admin.user.id).await {
            tracing::error!("bulk delete users: failed to delete {}: {:?}", id, e);
        }
    }
    Redirect::to(&redirect_url).into_response()
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
    // Left-join with users to surface any existing non-super_admin site owner.
    // If owner_user_id points to a super_admin we treat the site as having no
    // dedicated site admin yet (the slot is open for a real site_admin).
    let rows: Vec<(uuid::Uuid, String, Option<uuid::Uuid>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT s.id, s.hostname,
               u.id            AS owner_id,
               u.display_name  AS owner_name
        FROM   sites s
        LEFT JOIN users u
               ON u.id = s.owner_user_id
              AND u.role != 'super_admin'
              AND u.deleted_at IS NULL
        ORDER BY s.created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_else(|e| { tracing::warn!("failed to list sites for user form: {:?}", e); vec![] });

    rows.into_iter()
        .map(|(id, hostname, owner_id, owner_name)| SiteOption {
            id: id.to_string(),
            hostname,
            existing_admin_id:   owner_id.map(|uid| uid.to_string()),
            existing_admin_name: owner_name,
        })
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
            .map(|s| SiteOption {
                id: s.id.to_string(),
                hostname: s.hostname,
                // site_admin can't assign the site_admin role so the modal never fires.
                existing_admin_id:   None,
                existing_admin_name: None,
            })
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
    /// "remove" or "demote_author" — sent by the displacement modal when
    /// the target site already has a non-super_admin site admin.
    pub displaced_action: Option<String>,
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
            // Check if site already has a non-super_admin owner.
            let existing_owner: Option<uuid::Uuid> = sqlx::query_scalar(
                r#"SELECT s.owner_user_id
                   FROM sites s
                   JOIN users u ON u.id = s.owner_user_id
                   WHERE s.id = $1
                     AND u.role != 'super_admin'
                     AND u.deleted_at IS NULL"#,
            )
            .bind(site_uuid)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some(old_owner_id) = existing_owner {
                // An existing site admin must be displaced. Require the modal action.
                match form.displaced_action.as_deref() {
                    Some("remove") => {
                        // Remove displaced admin from this site entirely.
                        let _ = crate::models::site_user::remove(&state.db, site_uuid, old_owner_id).await;
                    }
                    Some("demote_author") => {
                        // Demote displaced admin to author on this site.
                        let _ = sqlx::query(
                            "UPDATE site_users SET role = 'author' WHERE site_id = $1 AND user_id = $2"
                        )
                        .bind(site_uuid)
                        .bind(old_owner_id)
                        .execute(&state.db)
                        .await;
                    }
                    _ => {
                        // Modal was bypassed somehow — refuse.
                        return Redirect::to(&format!(
                            "/admin/users/{}/site-access?error=site_admin_exists", user_id
                        )).into_response();
                    }
                }
                // Clear the old owner so it can be reassigned below.
                let _ = sqlx::query(
                    "UPDATE sites SET owner_user_id = NULL, updated_at = NOW() WHERE id = $1"
                )
                .bind(site_uuid)
                .execute(&state.db)
                .await;
            }

            // Set the new owner and promote user's global role.
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
