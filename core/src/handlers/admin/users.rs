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
use admin::pages::users::{UserEdit, UserRow};

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let raw = crate::models::user::list(&state.db).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list users: {:?}", e);
        vec![]
    });
    let rows: Vec<UserRow> = raw.iter().map(|u| UserRow {
        id: u.id.to_string(),
        username: u.username.clone(),
        email: u.email.clone(),
        role: u.role.clone(),
        display_name: u.display_name.clone(),
    }).collect();
    Html(admin::pages::users::render_list(&rows, None, &cs))
}

pub async fn new_user(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let edit = UserEdit {
        id: None,
        username: String::new(),
        email: String::new(),
        display_name: String::new(),
        role: "author".into(),
        bio: String::new(),
    };
    Html(admin::pages::users::render_editor(&edit, None, &cs))
}

pub async fn edit_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let user = match crate::models::user::get_by_id(&state.db, id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("user {} not found for editing: {:?}", id, e);
            return Redirect::to("/admin/users").into_response();
        }
    };
    let edit = UserEdit {
        id: Some(user.id.to_string()),
        username: user.username.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: user.role.clone(),
        bio: user.bio.clone(),
    };
    Html(admin::pages::users::render_editor(&edit, None, &cs)).into_response()
}

#[derive(Deserialize)]
pub struct UserForm {
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub password: Option<String>,
    pub role: String,
    pub bio: Option<String>,
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let password = match form.password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => {
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("Password is required for new users."),
                &cs,
            )).into_response();
        }
    };

    let role = parse_role(&form.role);
    let create = CreateUser {
        username: form.username.clone(),
        email: form.email.clone(),
        display_name: form.display_name.clone().filter(|s| !s.is_empty()).unwrap_or_default(),
        password,
        role,
    };

    match crate::models::user::create(&state.db, &create).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(e) => {
            tracing::error!("create user error: {:?}", e);
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &cs)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
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
                };
                return Html(admin::pages::users::render_editor(
                    &edit,
                    Some("Failed to process password. Please try again."),
                    &cs,
                )).into_response();
            }
        }
    } else {
        None
    };

    let update = UpdateUser {
        username: Some(form.username.clone()),
        email: Some(form.email.clone()),
        display_name: form.display_name.clone(),
        password_hash: new_password_hash,
        role: Some(parse_role(&form.role)),
        bio: form.bio.clone(),
    };

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
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &cs)).into_response()
        }
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = crate::models::user::delete(&state.db, id).await {
        tracing::error!("delete user {} error: {:?}", id, e);
    }
    Redirect::to("/admin/users")
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
        "admin" => UserRole::Admin,
        "editor" => UserRole::Editor,
        "author" => UserRole::Author,
        _ => UserRole::Subscriber,
    }
}
