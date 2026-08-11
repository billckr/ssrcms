use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::profile::ProfileForm;

#[derive(Deserialize)]
pub struct ProfileQuery {
    pub success: Option<String>,
    pub error: Option<String>,
}

fn flash_for(q: &ProfileQuery) -> Option<&'static str> {
    match q.error.as_deref() {
        Some("update_failed") => Some("Error updating profile. Please try again."),
        Some("password_mismatch") => Some("New passwords do not match."),
        Some("wrong_password") => Some("Current password is incorrect."),
        Some("weak_password") => Some("Password must be 8-12 characters, with at least one uppercase letter, one number, and one symbol (! @ # $ % &)."),
        Some("password_hash_failed") => Some("Password hashing error. Please try again."),
        Some("password_update_failed") => Some("Error changing password. Please try again."),
        _ => match q.success.as_deref() {
            Some("profile_updated") => Some("Profile updated successfully!"),
            Some("password_changed") => Some("Password changed successfully!"),
            _ => None,
        },
    }
}

pub async fn view(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<ProfileQuery>,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let profile = ProfileForm {
        username: admin.user.username.clone(),
        email: admin.user.email.clone(),
        display_name: admin.user.display_name.clone(),
        bio: admin.user.bio.clone(),
    };
    Html(admin::pages::profile::render_profile(&profile, flash_for(&q), &ctx))
}

#[derive(Deserialize)]
pub struct UpdateProfileForm {
    pub email: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
}

pub async fn update_profile(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<UpdateProfileForm>,
) -> impl IntoResponse {
    use crate::models::user::UpdateUser;

    // Always Some(...) — including Some("") — so clearing display name/bio to
    // empty actually persists instead of update() silently falling back to
    // the current DB value (its None means "leave untouched", not "clear").
    let display_name = form.display_name.clone().unwrap_or_default();
    let bio = form.bio.clone().unwrap_or_default();

    let update = UpdateUser {
        username: None,
        email: Some(form.email),
        display_name: Some(display_name),
        password_hash: None,
        role: None,
        bio: Some(bio),
    };

    match crate::models::user::update(&state.db, admin.user.id, &update).await {
        Ok(_) => Redirect::to("/admin/profile?success=profile_updated").into_response(),
        Err(e) => {
            tracing::error!("profile update failed: {e}");
            Redirect::to("/admin/profile?error=update_failed").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<ChangePasswordForm>,
) -> impl IntoResponse {
    if form.new_password != form.confirm_password {
        return Redirect::to("/admin/profile?error=password_mismatch").into_response();
    }

    if !admin.user.verify_password(&form.current_password) {
        return Redirect::to("/admin/profile?error=wrong_password").into_response();
    }

    if crate::models::user::validate_password(&form.new_password).is_err() {
        return Redirect::to("/admin/profile?error=weak_password").into_response();
    }

    let new_password_hash = match crate::models::user::hash_password(&form.new_password) {
        Ok(h) => h,
        Err(_) => return Redirect::to("/admin/profile?error=password_hash_failed").into_response(),
    };

    use crate::models::user::UpdateUser;
    let update = UpdateUser {
        username: None,
        email: None,
        display_name: None,
        password_hash: Some(new_password_hash),
        role: None,
        bio: None,
    };

    match crate::models::user::update(&state.db, admin.user.id, &update).await {
        Ok(_) => Redirect::to("/admin/profile?success=password_changed").into_response(),
        Err(e) => {
            tracing::error!("password change failed: {e}");
            Redirect::to("/admin/profile?error=password_update_failed").into_response()
        }
    }
}
