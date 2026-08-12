//! Public password-recovery flow: request a link by email, click the link,
//! set a new password. Delivery uses each site's own Mailgun account (or the
//! install-wide fallback) via `crate::mail`.

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::mail::{send_for_site, EmailMessage};
use crate::middleware::site::CurrentSite;
use crate::models::{password_reset, user};

#[derive(Deserialize)]
pub struct RequestForm {
    pub email: String,
    /// Honeypot: hidden field must stay empty. Bots that auto-fill forms populate it.
    #[serde(default)]
    pub website: String,
}

#[derive(Deserialize)]
pub struct ResetForm {
    pub password: String,
    pub confirm_password: String,
}

/// GET /recover — show the "type your email" request form.
pub async fn request_form(State(state): State<AppState>) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    Html(admin::pages::recover::render_request(None, false, &default_theme)).into_response()
}

/// POST /recover — always shows the same "check your email" message
/// regardless of whether the address is registered, so the form can't be
/// used to enumerate accounts.
pub async fn request_post(
    State(state): State<AppState>,
    site: CurrentSite,
    Form(form): Form<RequestForm>,
) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    if !form.website.trim().is_empty() {
        // Bot caught by the honeypot — pretend success, don't tip it off.
        return Html(admin::pages::recover::render_request(None, true, &default_theme)).into_response();
    }

    let email = form.email.trim().to_lowercase();
    let target = user::get_by_email(&state.db, &email).await.ok();
    // Staff accounts (super_admin/site_admin/editor/author) are excluded the
    // same way they're excluded from public /login — self-service recovery
    // is subscriber-only; staff password resets stay CLI-only
    // (`synap user reset-password`). Silently no-op rather than error,
    // same as an unregistered email, so the form can't be used to fingerprint
    // which addresses belong to staff.
    if let Some(target) = target.filter(|u| u.role == "subscriber") {
        match password_reset::create(&state.db, target.id).await {
            Ok(token) => {
                let link = format!("{}/recover/{}", site.base_url, token);
                let text = format!(
                    "Hi {},\n\n\
                     We received a request to reset the password for your account on {}.\n\n\
                     Reset your password: {}\n\n\
                     This link expires in 1 hour. If you didn't request this, you can ignore this email.",
                    target.display_name, site.settings.site_name, link,
                );
                if let Err(e) = send_for_site(
                    &state,
                    site.site.id,
                    EmailMessage {
                        to: &target.email,
                        subject: "Reset your password",
                        text: &text,
                        form_id: None,
                    },
                )
                .await
                {
                    tracing::error!("recover: failed to send reset email to {}: {:?}", target.email, e);
                }
            }
            Err(e) => tracing::error!("recover: failed to create reset token for {}: {:?}", target.email, e),
        }
    }

    Html(admin::pages::recover::render_request(None, true, &default_theme)).into_response()
}

/// GET /recover/{token} — show the "set a new password" form if the token
/// is still valid (unexpired, unused).
pub async fn reset_form(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    let valid = password_reset::find_valid_user_id(&state.db, &token).await.is_some();
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    Html(admin::pages::recover::render_reset(&token, valid, None, &default_theme)).into_response()
}

/// POST /recover/{token} — validate the new password, consume the token,
/// update the account, and send the user to sign in.
pub async fn reset_post(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Form(form): Form<ResetForm>,
) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    macro_rules! invalid_form {
        ($msg:expr) => {
            return Html(admin::pages::recover::render_reset(&token, true, Some($msg), &default_theme)).into_response()
        };
    }

    if form.password != form.confirm_password {
        invalid_form!("Passwords do not match.");
    }
    if let Err(msg) = user::validate_password(&form.password) {
        invalid_form!(msg);
    }

    let Some(user_id) = password_reset::consume(&state.db, &token).await else {
        // Expired/used/invalid by the time of submission.
        return Html(admin::pages::recover::render_reset(&token, false, None, &default_theme)).into_response();
    };

    let password_hash = match user::hash_password(&form.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("recover: password hashing failed for user {}: {:?}", user_id, e);
            invalid_form!("Something went wrong. Please try again.");
        }
    };

    let update = user::UpdateUser {
        username: None,
        email: None,
        display_name: None,
        password_hash: Some(password_hash),
        role: None,
        bio: None,
    };

    match user::update(&state.db, user_id, &update).await {
        Ok(_) => Redirect::to("/login?flash=Password+reset.+You+can+now+sign+in.").into_response(),
        Err(e) => {
            tracing::error!("recover: failed to update password for user {}: {:?}", user_id, e);
            invalid_form!("Something went wrong. Please try again.");
        }
    }
}
