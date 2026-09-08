//! Verified self-service email change for subscribers: request a link,
//! click it, and the candidate address is committed.
//!
//! Kept separate from `account.rs` because the confirm routes must work for
//! a browser that isn't authenticated in the current session — the user may
//! open the confirmation link on a different device than the one that
//! requested the change, so unlike the rest of `account.rs` these routes
//! don't require `AccountUser`.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::mail::{send_for_site, EmailMessage};
use crate::middleware::account_auth::{
    AccountUser, SESSION_ACCOUNT_CREDENTIAL_VERSION_KEY, SESSION_ACCOUNT_USER_ID_KEY,
};
use crate::middleware::site::CurrentSite;
use crate::models::{email_change, user};

#[derive(Deserialize)]
pub struct RequestEmailChangeForm {
    pub new_email: String,
    pub current_password: String,
}

fn redirect_to_profile(flash: &str) -> Redirect {
    Redirect::to(&format!(
        "/account/profile?flash={}",
        flash.replace(' ', "+")
    ))
}

/// POST /account/email/change — request a verified email change. Requires
/// re-entering the current password: email is a recovery identity, the same
/// sensitivity bar as changing a password.
pub async fn request_change(
    State(state): State<AppState>,
    account: AccountUser,
    site: CurrentSite,
    headers: HeaderMap,
    Form(form): Form<RequestEmailChangeForm>,
) -> Redirect {
    if !account.user.verify_password(&form.current_password) {
        return redirect_to_profile("Current password is incorrect.");
    }

    let new_email = user::normalize_email(&form.new_email);
    if new_email.is_empty() || !new_email.contains('@') || new_email.len() > 254 {
        return redirect_to_profile("A valid email address is required.");
    }
    if new_email == account.user.email {
        return redirect_to_profile("That's already your email address.");
    }
    if !crate::middleware::auth_security::allow("email-change", &headers, &new_email) {
        return redirect_to_profile("Too many requests. Please try again later.");
    }
    // UX-only availability check — this is an authenticated, rate-limited
    // context (unlike anonymous /subscribe), so revealing "already in use"
    // here isn't an account-enumeration concern.
    if let Ok(existing) = user::get_by_email(&state.db, &new_email).await {
        if existing.id != account.user.id {
            return redirect_to_profile("That email address is already in use.");
        }
    }

    // Keep token creation and mail-provider latency off the response path.
    let task_state = state.clone();
    let user_id = account.user.id;
    let display_name = account.user.display_name.clone();
    let site_id = site.site.id;
    let site_name = site.settings.site_name.clone();
    let site_base_url = site.base_url.clone();
    let target_email = new_email.clone();
    tokio::spawn(async move {
        match email_change::create(&task_state.db, user_id, &target_email).await {
            Ok(token) => {
                let link = format!("{site_base_url}/account/email/confirm/{token}");
                let text = format!(
                    "Hi {display_name},\n\n\
                     We received a request to change the sign-in email for your account on {site_name} to this address.\n\n\
                     Confirm the change: {link}\n\n\
                     This link expires in 1 hour. If you didn't request this, you can ignore this email.",
                );
                if let Err(e) = send_for_site(
                    &task_state,
                    site_id,
                    EmailMessage {
                        to: &target_email,
                        subject: "Confirm your new email address",
                        text: &text,
                        form_id: None,
                        provider_id: None,
                    },
                )
                .await
                {
                    tracing::error!(
                        "account_email: failed to send confirmation to {}: {:?}",
                        target_email,
                        e
                    );
                }
            }
            Err(e) => tracing::error!(
                "account_email: failed to create change token for {}: {:?}",
                user_id,
                e
            ),
        }
    });

    redirect_to_profile(&format!(
        "Check {new_email} for a confirmation link. It expires in 1 hour."
    ))
}

/// GET /account/email/confirm/{token} — show a "confirm this change?" page
/// if the token is still valid (unexpired, unused). Unauthenticated.
pub async fn confirm_form(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();

    match email_change::find_valid_by_token(&state.db, &token).await {
        Some(pending) => {
            let old_email = user::get_by_id(&state.db, pending.user_id)
                .await
                .map(|u| u.email)
                .unwrap_or_default();
            Html(admin::pages::account_email::render_confirm(
                &token,
                admin::pages::account_email::ConfirmState::Pending {
                    old_email: &old_email,
                    new_email: &pending.new_email,
                },
                &default_theme,
            ))
            .into_response()
        }
        None => Html(admin::pages::account_email::render_confirm(
            &token,
            admin::pages::account_email::ConfirmState::Invalid,
            &default_theme,
        ))
        .into_response(),
    }
}

/// POST /account/email/confirm/{token} — consume the token and commit the
/// change. Unauthenticated: the link may be opened on a different
/// browser/device than the one that requested it.
pub async fn confirm_post(
    State(state): State<AppState>,
    site: CurrentSite,
    session: Session,
    Path(token): Path<String>,
) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();

    match email_change::consume_and_apply(&state.db, &token).await {
        Ok(Some(applied)) => {
            let current_session_user: Option<String> = session
                .get(SESSION_ACCOUNT_USER_ID_KEY)
                .await
                .ok()
                .flatten();
            let response =
                if current_session_user.as_deref() == Some(applied.user.id.to_string().as_str()) {
                    // This browser is the one that requested the change — refresh
                    // its stored marker so it isn't logged out by the very
                    // credential-version bump it's about to cause.
                    let _ = session
                        .insert(
                            SESSION_ACCOUNT_CREDENTIAL_VERSION_KEY,
                            applied.user.credential_version(),
                        )
                        .await;
                    Redirect::to("/account/profile?flash=Email+updated!").into_response()
                } else {
                    Redirect::to("/login?notice=email-changed").into_response()
                };

            // Notify the old address in the background — best-effort, must
            // not block the redirect.
            let task_state = state.clone();
            let old_email = applied.old_email.clone();
            let new_email = applied.user.email.clone();
            let display_name = applied.user.display_name.clone();
            let site_id = site.site.id;
            let site_name = site.settings.site_name.clone();
            tokio::spawn(async move {
                let text = format!(
                    "Hi {display_name},\n\n\
                     The sign-in email on your {site_name} account was changed to {new_email}.\n\n\
                     If you didn't make this change, contact support immediately.",
                );
                if let Err(e) = send_for_site(
                    &task_state,
                    site_id,
                    EmailMessage {
                        to: &old_email,
                        subject: "Your email address was changed",
                        text: &text,
                        form_id: None,
                        provider_id: None,
                    },
                )
                .await
                {
                    tracing::error!(
                        "account_email: failed to notify old address {}: {:?}",
                        old_email,
                        e
                    );
                }
            });

            response
        }
        Ok(None) => Html(admin::pages::account_email::render_confirm(
            &token,
            admin::pages::account_email::ConfirmState::Invalid,
            &default_theme,
        ))
        .into_response(),
        Err(e) => {
            tracing::error!("account_email: failed to apply change for a token: {:?}", e);
            let msg = if e.to_string().contains("duplicate key") || e.to_string().contains("unique")
            {
                "That email was claimed by another account before you confirmed. Please request the change again."
            } else {
                "Something went wrong. Please try again."
            };
            Html(admin::pages::account_email::render_confirm(
                &token,
                admin::pages::account_email::ConfirmState::Error(msg),
                &default_theme,
            ))
            .into_response()
        }
    }
}
