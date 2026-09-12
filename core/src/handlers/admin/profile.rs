use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

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
        Some("weak_password") => Some("Password must be 12-128 characters."),
        Some("password_hash_failed") => Some("Password hashing error. Please try again."),
        Some("password_update_failed") => Some("Error changing password. Please try again."),
        Some("sign_out_other_devices_failed") => {
            Some("Error signing out other sessions. Please try again.")
        }
        Some("mfa_wrong_password") => Some("Current password is incorrect."),
        Some("mfa_setup_failed") => {
            Some("Error starting two-factor authentication setup. Please try again.")
        }
        Some("mfa_disable_failed") => {
            Some("Error disabling two-factor authentication. Please try again.")
        }
        Some("mfa_regenerate_failed") => {
            Some("Error regenerating recovery codes. Please try again.")
        }
        _ => match q.success.as_deref() {
            Some("profile_updated") => Some("Profile updated successfully!"),
            Some("password_changed") => Some("Password changed successfully!"),
            Some("signed_out_other_devices") => Some("Signed out of every other session."),
            Some("mfa_disabled") => Some("Two-factor authentication disabled."),
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
    let mfa_enabled_at = crate::models::user_totp::enabled_at(&state.db, admin.user.id)
        .await
        .unwrap_or(None);
    let mfa_recovery_codes_remaining = if mfa_enabled_at.is_some() {
        crate::models::mfa_recovery_code::count_remaining(&state.db, admin.user.id)
            .await
            .unwrap_or(0)
    } else {
        0
    };
    let profile = ProfileForm {
        username: admin.user.username.clone(),
        email: admin.user.email.clone(),
        display_name: admin.user.display_name.clone(),
        bio: admin.user.bio.clone(),
        mfa_enabled: mfa_enabled_at.is_some(),
        mfa_enabled_at: mfa_enabled_at.map(|d| d.format("%Y-%m-%d").to_string()),
        mfa_recovery_codes_remaining,
    };
    Html(admin::pages::profile::render_profile(
        &profile,
        flash_for(&q),
        &ctx,
    ))
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
        // Email is an authentication identifier. Self-service changes stay
        // disabled until a pending-address verification flow is available.
        email: None,
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
    session: Session,
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
        Ok(updated) => {
            let _ = session
                .insert(
                    crate::middleware::admin_auth::SESSION_CREDENTIAL_VERSION_KEY,
                    updated.credential_version(),
                )
                .await;
            Redirect::to("/admin/profile?success=password_changed").into_response()
        }
        Err(e) => {
            tracing::error!("password change failed: {e}");
            Redirect::to("/admin/profile?error=password_update_failed").into_response()
        }
    }
}

/// POST /admin/profile/sign-out-other-devices — invalidate every other
/// session for this account. The current session is kept alive by
/// re-inserting the fresh `credential_version()` right after the bump.
pub async fn sign_out_other_devices(
    State(state): State<AppState>,
    admin: AdminUser,
    session: Session,
) -> impl IntoResponse {
    match crate::models::user::regenerate_session_nonce(&state.db, admin.user.id).await {
        Ok(updated) => {
            let _ = session
                .insert(
                    crate::middleware::admin_auth::SESSION_CREDENTIAL_VERSION_KEY,
                    updated.credential_version(),
                )
                .await;
            Redirect::to("/admin/profile?success=signed_out_other_devices").into_response()
        }
        Err(e) => {
            tracing::error!("sign-out-other-devices failed: {e}");
            Redirect::to("/admin/profile?error=sign_out_other_devices_failed").into_response()
        }
    }
}

// ── Two-factor authentication (TOTP) ────────────────────────────────────

/// Sends a best-effort notification email about an MFA state change — so a
/// hijacked-session attacker silently enabling MFA as a persistence
/// mechanism gets caught by the real account owner. Resolves a site to
/// send through as `admin.site_id` (if scoped) or the user's own
/// `default_site_id` (a global super_admin can have neither pinned nor a
/// default), same fallback used elsewhere for a super_admin without a
/// current site. Never blocks the caller — a missing/unreachable provider
/// only logs a warning, matching `mail::send_for_site`'s own "opt-in, not
/// required" behavior.
fn notify_mfa_change(state: &AppState, admin: &AdminUser, subject: &'static str, text: String) {
    let Some(site_id) = admin.site_id.or(admin.user.default_site_id) else {
        tracing::warn!(
            "no site available to send MFA notification to user {} — skipping",
            admin.user.id
        );
        return;
    };
    let task_state = state.clone();
    let to = admin.user.email.clone();
    tokio::spawn(async move {
        let _ = crate::mail::send_for_site(
            &task_state,
            site_id,
            crate::mail::EmailMessage {
                to: &to,
                subject,
                text: &text,
                form_id: None,
                provider_id: None,
            },
        )
        .await;
    });
}

#[derive(Deserialize)]
pub struct MfaPasswordConfirmForm {
    pub current_password: String,
}

/// POST /admin/profile/2fa/setup/start — confirm current password, then
/// generate a fresh pending TOTP secret and send the browser to the
/// QR/confirm page.
pub async fn mfa_setup_start(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<MfaPasswordConfirmForm>,
) -> impl IntoResponse {
    if !admin.user.verify_password(&form.current_password) {
        return Redirect::to("/admin/profile?error=mfa_wrong_password").into_response();
    }
    if let Err(e) = crate::models::user_totp::start_enrollment(
        &state.db,
        &state.config.secret_key,
        admin.user.id,
    )
    .await
    {
        tracing::error!("mfa setup start failed: {e}");
        return Redirect::to("/admin/profile?error=mfa_setup_failed").into_response();
    }
    Redirect::to("/admin/profile/2fa/setup").into_response()
}

#[derive(Deserialize)]
pub struct MfaSetupQuery {
    pub error: Option<String>,
}

/// GET /admin/profile/2fa/setup — show the QR/manual-entry secret for a
/// still-unconfirmed enrollment started by `mfa_setup_start`. Redirects
/// back to the profile page if there's nothing pending (e.g. a stale
/// bookmark after already confirming or never starting).
pub async fn mfa_setup_view(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<MfaSetupQuery>,
) -> impl IntoResponse {
    let Ok(Some(secret)) = crate::models::user_totp::pending_secret(
        &state.db,
        &state.config.secret_key,
        admin.user.id,
    )
    .await
    else {
        return Redirect::to("/admin/profile").into_response();
    };
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let issuer = state.app_settings.read().unwrap().app_name.clone();
    let uri = crate::utils::totp::provisioning_uri(&secret, &admin.user.email, &issuer);
    let qr = crate::utils::totp::qr_svg(&uri);
    let error = match q.error.as_deref() {
        Some("invalid_code") => Some("Invalid code. Please try again."),
        _ => None,
    };
    Html(admin::pages::profile_mfa::render_setup(
        &secret,
        qr.as_deref(),
        error,
        &ctx,
    ))
    .into_response()
}

#[derive(Deserialize)]
pub struct MfaCodeForm {
    pub code: String,
}

/// POST /admin/profile/2fa/setup/confirm — verify the first code from the
/// newly scanned authenticator app. On success, issues recovery codes and
/// shows them once.
pub async fn mfa_setup_confirm(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<MfaCodeForm>,
) -> impl IntoResponse {
    let now = chrono::Utc::now();
    let confirmed = crate::models::user_totp::confirm_enrollment(
        &state.db,
        &state.config.secret_key,
        admin.user.id,
        form.code.trim(),
        now,
    )
    .await
    .unwrap_or(false);

    if !confirmed {
        return Redirect::to("/admin/profile/2fa/setup?error=invalid_code").into_response();
    }

    let codes = crate::models::mfa_recovery_code::generate_batch();
    if let Err(e) =
        crate::models::mfa_recovery_code::replace_all(&state.db, admin.user.id, &codes).await
    {
        tracing::error!("failed to store recovery codes after mfa enable: {e}");
    }

    notify_mfa_change(
        &state,
        &admin,
        "Two-factor authentication was enabled on your account",
        format!(
            "Hi {},\n\nTwo-factor authentication was just enabled on your account. \
             If you didn't do this, sign in and disable it immediately, or contact an administrator.",
            admin.user.display_name
        ),
    );

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::profile_mfa::render_recovery_codes(
        &codes,
        "Two-factor authentication enabled",
        &ctx,
    ))
    .into_response()
}

/// POST /admin/profile/2fa/disable — confirm current password, then remove
/// MFA and every recovery code entirely.
pub async fn mfa_disable(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<MfaPasswordConfirmForm>,
) -> impl IntoResponse {
    if !admin.user.verify_password(&form.current_password) {
        return Redirect::to("/admin/profile?error=mfa_wrong_password").into_response();
    }
    if let Err(e) = crate::models::user_totp::disable(&state.db, admin.user.id).await {
        tracing::error!("mfa disable failed: {e}");
        return Redirect::to("/admin/profile?error=mfa_disable_failed").into_response();
    }
    let _ = crate::models::mfa_recovery_code::delete_all_for_user(&state.db, admin.user.id).await;

    notify_mfa_change(
        &state,
        &admin,
        "Two-factor authentication was disabled on your account",
        format!(
            "Hi {},\n\nTwo-factor authentication was just disabled on your account. \
             If you didn't do this, contact an administrator immediately.",
            admin.user.display_name
        ),
    );

    Redirect::to("/admin/profile?success=mfa_disabled").into_response()
}

/// POST /admin/profile/2fa/recovery-codes/regenerate — confirm current
/// password, then replace every recovery code and show the new set once.
pub async fn mfa_recovery_codes_regenerate(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<MfaPasswordConfirmForm>,
) -> impl IntoResponse {
    if !admin.user.verify_password(&form.current_password) {
        return Redirect::to("/admin/profile?error=mfa_wrong_password").into_response();
    }
    let codes = crate::models::mfa_recovery_code::generate_batch();
    if let Err(e) =
        crate::models::mfa_recovery_code::replace_all(&state.db, admin.user.id, &codes).await
    {
        tracing::error!("mfa recovery code regeneration failed: {e}");
        return Redirect::to("/admin/profile?error=mfa_regenerate_failed").into_response();
    }

    notify_mfa_change(
        &state,
        &admin,
        "Your two-factor recovery codes were regenerated",
        format!(
            "Hi {},\n\nYour two-factor authentication recovery codes were just regenerated — \
             every old code has been invalidated. If you didn't do this, contact an administrator immediately.",
            admin.user.display_name
        ),
    );

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::profile_mfa::render_recovery_codes(
        &codes,
        "Recovery codes regenerated",
        &ctx,
    ))
    .into_response()
}
