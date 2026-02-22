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
    _admin: AdminUser,
) -> Html<String> {
    let raw = crate::models::user::list(&state.db).await.unwrap_or_default();
    let rows: Vec<UserRow> = raw.iter().map(|u| UserRow {
        id: u.id.to_string(),
        username: u.username.clone(),
        email: u.email.clone(),
        role: u.role.clone(),
        display_name: u.display_name.clone(),
    }).collect();
    Html(admin::pages::users::render_list(&rows, None))
}

pub async fn new_user(
    State(_state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    let edit = UserEdit {
        id: None,
        username: String::new(),
        email: String::new(),
        display_name: String::new(),
        role: "author".into(),
        bio: String::new(),
    };
    Html(admin::pages::users::render_editor(&edit, None))
}

pub async fn edit_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let user = match crate::models::user::get_by_id(&state.db, id).await {
        Ok(u) => u,
        Err(_) => return Redirect::to("/admin/users").into_response(),
    };
    let edit = UserEdit {
        id: Some(user.id.to_string()),
        username: user.username.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: user.role.clone(),
        bio: user.bio.clone(),
    };
    Html(admin::pages::users::render_editor(&edit, None)).into_response()
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
    _admin: AdminUser,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    let password = match form.password.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => return Html("<p>Password is required for new users.</p>".to_string()).into_response(),
    };

    let role = parse_role(&form.role);
    let create = CreateUser {
        username: form.username,
        email: form.email,
        display_name: form.display_name.filter(|s| !s.is_empty()).unwrap_or_default(),
        password,
        role,
    };

    match crate::models::user::create(&state.db, &create).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(e) => Html(format!("<p>Error: {}</p>", e)).into_response(),
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    let new_password_hash = if let Some(pw) = form.password.filter(|p| !p.is_empty()) {
        match crate::models::user::hash_password(&pw) {
            Ok(h) => Some(h),
            Err(_) => return Html("<p>Password hashing error.</p>".to_string()).into_response(),
        }
    } else {
        None
    };

    let update = UpdateUser {
        username: Some(form.username),
        email: Some(form.email),
        display_name: form.display_name,
        password_hash: new_password_hash,
        role: Some(parse_role(&form.role)),
        bio: form.bio,
    };

    match crate::models::user::update(&state.db, id, &update).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(e) => Html(format!("<p>Error: {}</p>", e)).into_response(),
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
