//! Account area handlers — profile, saved posts, my comments.
//! All routes require an `AccountUser` (any logged-in role).

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::account_auth::AccountUser;
use admin::pages::account::{AccountContext, MyCommentRow, ProfileData};

fn build_ctx(account: &AccountUser) -> AccountContext {
    AccountContext {
        user_email:        account.user.email.clone(),
        user_role:         account.user.role.clone(),
        user_display_name: account.user.display_name.clone(),
        site_name:         account.site_name.clone(),
        site_base_url:     account.site_base_url.clone(),
    }
}

// ── Dashboard ────────────────────────────────────────────────────────

/// GET /account — dashboard (default landing page).
pub async fn dashboard(account: AccountUser) -> Html<String> {
    let ctx = build_ctx(&account);
    Html(admin::pages::account::render_dashboard(&ctx))
}

// ── Profile ────────────────────────────────────────────────────────

/// GET /account/profile — profile view.
pub async fn profile_view(account: AccountUser) -> Html<String> {
    let ctx = build_ctx(&account);
    let data = ProfileData {
        email:        account.user.email.clone(),
        display_name: account.user.display_name.clone(),
    };
    Html(admin::pages::account::render_profile(&data, None, &ctx))
}

#[derive(Deserialize)]
pub struct UpdateForm {
    pub email:        String,
    pub display_name: Option<String>,
}

/// POST /account/profile/update
pub async fn profile_update(
    State(state): State<AppState>,
    account: AccountUser,
    Form(form): Form<UpdateForm>,
) -> Html<String> {
    let ctx = build_ctx(&account);
    let data = ProfileData {
        email:        form.email.clone(),
        display_name: form.display_name.clone().unwrap_or_default(),
    };

    use crate::models::user::UpdateUser;
    let update = UpdateUser {
        username:      None,
        email:         Some(form.email),
        display_name:  form.display_name.filter(|s| !s.is_empty()),
        password_hash: None,
        role:          None,
        bio:           None,
    };

    let flash = match crate::models::user::update(&state.db, account.user.id, &update).await {
        Ok(_)  => "Profile updated successfully!",
        Err(_) => "Error saving profile. Please try again.",
    };

    Html(admin::pages::account::render_profile(&data, Some(flash), &ctx))
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password:  String,
    pub new_password:      String,
    pub confirm_password:  String,
}

/// POST /account/profile/change-password
pub async fn profile_change_password(
    State(state): State<AppState>,
    account: AccountUser,
    Form(form): Form<ChangePasswordForm>,
) -> Html<String> {
    let ctx = build_ctx(&account);
    let data = ProfileData {
        email:        account.user.email.clone(),
        display_name: account.user.display_name.clone(),
    };

    if form.new_password != form.confirm_password {
        return Html(admin::pages::account::render_profile(
            &data, Some("New passwords do not match."), &ctx,
        ));
    }
    if !account.user.verify_password(&form.current_password) {
        return Html(admin::pages::account::render_profile(
            &data, Some("Current password is incorrect."), &ctx,
        ));
    }
    if let Err(msg) = crate::models::user::validate_password(&form.new_password) {
        return Html(admin::pages::account::render_profile(&data, Some(msg), &ctx));
    }

    let new_hash = match crate::models::user::hash_password(&form.new_password) {
        Ok(h) => h,
        Err(_) => return Html(admin::pages::account::render_profile(
            &data, Some("Password hashing error. Please try again."), &ctx,
        )),
    };

    use crate::models::user::UpdateUser;
    let update = UpdateUser {
        username: None, email: None, display_name: None,
        password_hash: Some(new_hash), role: None, bio: None,
    };

    let flash = match crate::models::user::update(&state.db, account.user.id, &update).await {
        Ok(_)  => "Password changed successfully!",
        Err(_) => "Error changing password. Please try again.",
    };

    Html(admin::pages::account::render_profile(&data, Some(flash), &ctx))
}

// ── Saved Posts (stub) ───────────────────────────────────────────────────────

/// GET /account/saved-posts
pub async fn saved_posts(account: AccountUser) -> Html<String> {
    let ctx = build_ctx(&account);
    Html(admin::pages::account::render_saved_posts(&ctx))
}

// ── My Comments ──────────────────────────────────────────────────────────────

/// GET /account/my-comments
pub async fn my_comments(
    State(state): State<AppState>,
    account: AccountUser,
) -> Html<String> {
    let ctx = build_ctx(&account);
    let window = chrono::Duration::minutes(15);
    let now = chrono::Utc::now();
    let records = if let Some(site_id) = account.site_id {
        crate::models::comment::list_for_user(&state.db, account.user.id, site_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("failed to fetch comments for user {}: {:?}", account.user.id, e);
                vec![]
            })
    } else {
        vec![]
    };
    let rows: Vec<MyCommentRow> = records.into_iter().map(|r| {
        let can_delete = (now - r.created_at) < window;
        let body_preview = {
            let mut chars = r.body.chars();
            let s: String = chars.by_ref().take(35).collect();
            if chars.next().is_some() { format!("{s}…") } else { s }
        };
        let post_title = {
            let mut chars = r.post_title.chars();
            let s: String = chars.by_ref().take(25).collect();
            if chars.next().is_some() { format!("{s}…") } else { s }
        };
        MyCommentRow {
            id:            r.id.to_string(),
            body_preview,
            post_title,
            post_slug:     r.post_slug,
            site_hostname: r.site_hostname,
            created_at:    r.created_at.format("%B %-d, %Y at %-I:%M %p").to_string(),
            can_delete,
        }
    }).collect();
    Html(admin::pages::account::render_my_comments(&rows, &ctx))
}

/// POST /account/comments/{id}/delete — soft-delete within the 15-minute window.
pub async fn delete_comment(
    State(state): State<AppState>,
    account: AccountUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Fetch the comment first so we can enforce the time window server-side.
    match crate::models::comment::get_by_id(&state.db, id).await {
        Ok(comment) => {
            let elapsed = chrono::Utc::now() - comment.created_at;
            let within_window = elapsed < chrono::Duration::minutes(15);
            let is_owner = comment.author_id == account.user.id;

            if is_owner && within_window && comment.deleted_at.is_none() {
                if let Err(e) = crate::models::comment::soft_delete(&state.db, id, account.user.id).await {
                    tracing::warn!("soft-delete failed for comment {}: {:?}", id, e);
                }
            }
        }
        Err(_) => {}
    }
    Redirect::to("/account/my-comments")
}
