use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Form,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::profile::ProfileForm;

pub async fn view(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let profile = ProfileForm {
        username: admin.user.username.clone(),
        email: admin.user.email.clone(),
        display_name: admin.user.display_name.clone(),
        bio: admin.user.bio.clone(),
    };
    Html(admin::pages::profile::render_profile(&profile, None, &cs, admin.is_global_admin, admin.is_visiting_foreign_site, &admin.user.email, admin.is_global_admin || admin.site_role.as_str() == "admin"))
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

    let cs = state.site_hostname(admin.site_id);
    let email = form.email.clone();
    let display_name = form.display_name.clone().filter(|s| !s.is_empty());
    let bio = form.bio.clone().filter(|s| !s.is_empty());

    let update = UpdateUser {
        username: None,
        email: Some(form.email),
        display_name: display_name.clone(),
        password_hash: None,
        role: None,
        bio: bio.clone(),
    };

    let profile = ProfileForm {
        username: admin.user.username.clone(),
        email: email.clone(),
        display_name: display_name.clone().unwrap_or_default(),
        bio: bio.clone().unwrap_or_default(),
    };

    match crate::models::user::update(&state.db, admin.user.id, &update).await {
        Ok(_) => Html(admin::pages::profile::render_profile(
            &profile,
            Some("Profile updated successfully!"),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        )),
        Err(e) => Html(admin::pages::profile::render_profile(
            &profile,
            Some(&format!("Error updating profile: {}", e)),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        )),
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

fn validate_password_requirements(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long");
    }

    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_numeric());

    if !has_upper || !has_lower || !has_digit {
        return Err("Password must contain uppercase and lowercase letters, and at least one number");
    }

    Ok(())
}

pub async fn change_password(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<ChangePasswordForm>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let profile = ProfileForm {
        username: admin.user.username.clone(),
        email: admin.user.email.clone(),
        display_name: admin.user.display_name.clone(),
        bio: admin.user.bio.clone(),
    };

    if form.new_password != form.confirm_password {
        return Html(admin::pages::profile::render_profile(
            &profile,
            Some("New passwords do not match."),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &admin.user.email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        ));
    }

    if !admin.user.verify_password(&form.current_password) {
        return Html(admin::pages::profile::render_profile(
            &profile,
            Some("Current password is incorrect."),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &admin.user.email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        ));
    }

    if let Err(e) = validate_password_requirements(&form.new_password) {
        return Html(admin::pages::profile::render_profile(&profile, Some(e), &cs, admin.is_global_admin, admin.is_visiting_foreign_site, &admin.user.email, admin.is_global_admin || admin.site_role.as_str() == "admin"));
    }

    let new_password_hash = match crate::models::user::hash_password(&form.new_password) {
        Ok(h) => h,
        Err(_) => {
            return Html(admin::pages::profile::render_profile(
                &profile,
                Some("Password hashing error. Please try again."),
                &cs,
                admin.is_global_admin,
                admin.is_visiting_foreign_site,
                &admin.user.email,
                admin.is_global_admin || admin.site_role.as_str() == "admin",
            ));
        }
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
        Ok(_) => Html(admin::pages::profile::render_profile(
            &profile,
            Some("Password changed successfully!"),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &admin.user.email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        )),
        Err(e) => Html(admin::pages::profile::render_profile(
            &profile,
            Some(&format!("Error changing password: {}", e)),
            &cs,
            admin.is_global_admin,
            admin.is_visiting_foreign_site,
            &admin.user.email,
            admin.is_global_admin || admin.site_role.as_str() == "admin",
        )),
    }
}
