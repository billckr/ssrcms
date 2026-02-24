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
    if admin.site_role.as_str() != "admin" && !admin.is_global_admin {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let rows: Vec<UserRow> = if admin.is_global_admin {
        crate::models::user::list(&state.db).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list users: {:?}", e);
            vec![]
        }).iter().map(|u| UserRow {
            id: u.id.to_string(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: u.role.clone(),
            display_name: u.display_name.clone(),
            is_protected: u.is_protected,
        }).collect()
    } else if let Some(site_id) = admin.site_id {
        crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list site users: {:?}", e);
            vec![]
        }).iter().map(|(u, site_role)| UserRow {
            id: u.id.to_string(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: site_role.clone(),
            display_name: u.display_name.clone(),
            is_protected: u.is_protected,
        }).collect()
    } else {
        vec![]
    };
    let current_user_id = admin.user.id.to_string();
    Html(admin::pages::users::render_list(&rows, None, &cs, &current_user_id, admin.is_global_admin, &admin.user.email)).into_response()
}

pub async fn new_user(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if admin.site_role.as_str() != "admin" && !admin.is_global_admin {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let sites = if admin.is_global_admin {
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
    Html(admin::pages::users::render_editor(&edit, None, &cs, admin.is_global_admin, &admin.user.email)).into_response()
}

pub async fn edit_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.is_global_admin {
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
    Html(admin::pages::users::render_editor(&edit, None, &cs, admin.is_global_admin, &admin.user.email)).into_response()
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
    if admin.site_role.as_str() != "admin" && !admin.is_global_admin {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let password = match form.password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => {
            let sites = if admin.is_global_admin { fetch_site_options(&state).await } else { vec![] };
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
                &cs,
                admin.is_global_admin,
                &admin.user.email,
            )).into_response();
        }
    };

    // site_role: what goes into site_users.role (admin/editor/author/subscriber).
    // Site admins cannot assign super_admin; cap to editor.
    let site_role = if !admin.is_global_admin && form.role == "super_admin" {
        "editor"
    } else {
        form.role.as_str()
    };
    // users_role: what goes into users.role. "admin" is a site_users concept; "super_admin"
    // is CLI-only. Both map to "editor" in the users table.
    let users_role_str = match site_role {
        "admin" | "super_admin" => "editor",
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
            if admin.is_global_admin {
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
                }
            } else {
                // Site admin: auto-scope to their site, record who invited.
                if let Some(site_id) = admin.site_id {
                    if let Err(e) = crate::models::site_user::add(&state.db, site_id, new_user.id, site_role, Some(admin.user.id)).await {
                        tracing::warn!("failed to add new user {} to site {}: {:?}", new_user.id, site_id, e);
                    }
                }
            }
            Redirect::to("/admin/users").into_response()
        }
        Err(e) => {
            tracing::error!("create user error: {:?}", e);
            let sites = if admin.is_global_admin { fetch_site_options(&state).await } else { vec![] };
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
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &cs, admin.is_global_admin, &admin.user.email)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    if admin.site_role.as_str() != "admin" && !admin.is_global_admin {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.is_global_admin {
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
                    &cs,
                    admin.is_global_admin,
                    &admin.user.email,
                )).into_response();
            }
        }
    } else {
        None
    };

    // Determine users.role to write:
    // - super_admin targets keep their role (never downgraded via form)
    // - "admin" is a site_users role concept; map to "editor" in users table
    // - "super_admin" cannot be set via form (CLI-only)
    let new_users_role = if is_super_admin_target {
        parse_role("super_admin")
    } else {
        match form.role.as_str() {
            "admin" | "super_admin" => parse_role("editor"),
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
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &cs, admin.is_global_admin, &admin.user.email)).into_response()
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

    macro_rules! deny {
        ($msg:expr) => {{
            tracing::warn!("delete_user denied for target={} actor={}: {}", id, admin.user.id, $msg);
            let rows: Vec<UserRow> = if admin.is_global_admin {
                crate::models::user::list(&state.db).await.unwrap_or_default()
                    .iter().map(|u| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: u.role.clone(),
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                    }).collect()
            } else if let Some(site_id) = admin.site_id {
                crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_default()
                    .into_iter().map(|(u, site_role)| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: site_role,
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                    }).collect()
            } else {
                vec![]
            };
            return Html(admin::pages::users::render_list(
                &rows,
                Some($msg),
                &cs,
                &current_user_id,
                admin.is_global_admin,
                &admin.user.email,
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
        if t.role == "super_admin" && !admin.is_global_admin {
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
