//! Public subscriber signup handlers: GET /subscribe and POST /subscribe.
//!
//! Site resolution comes from the Host header via the `CurrentSite` extractor,
//! so posting to bckr.local/subscribe automatically scopes the new subscriber
//! to that site — no extra query params or hidden fields required.

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::user::{CreateUser, UserRole};

#[derive(Deserialize)]
pub struct SubscribeQuery {
    /// Set to "1" after a successful signup to show the success page.
    #[serde(default)]
    pub subscribed: Option<String>,
}

#[derive(Deserialize)]
pub struct SubscribeForm {
    pub display_name: String,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    /// Honeypot: must be absent/empty. Bots fill hidden fields; real users leave this blank.
    #[serde(default)]
    pub website: String,
    /// "I am human" checkbox — must be "on".
    #[serde(default)]
    pub human_check: String,
    /// Terms of Service agreement checkbox — must be "on".
    #[serde(default)]
    pub terms: String,
}

/// GET /subscribe — show the signup form (or success page after redirect).
pub async fn subscribe_form(
    Query(q): Query<SubscribeQuery>,
    site: CurrentSite,
) -> Response {
    if q.subscribed.as_deref() == Some("1") {
        Html(admin::pages::subscribe::render_success(&site.settings.site_name)).into_response()
    } else {
        Html(admin::pages::subscribe::render(None, &site.settings.site_name)).into_response()
    }
}

/// POST /subscribe — validate, create user + site_users row, redirect on success.
pub async fn subscribe_post(
    State(state): State<AppState>,
    site: CurrentSite,
    Form(form): Form<SubscribeForm>,
) -> Response {
    let site_name = site.settings.site_name.clone();
    let site_id = site.site.id;

    macro_rules! err {
        ($msg:expr) => {
            return Html(admin::pages::subscribe::render(Some($msg), &site_name)).into_response()
        };
    }

    // ── Bot / spam checks ─────────────────────────────────────────────────────
    // Honeypot: hidden field must be empty. Bots that auto-fill forms will populate it.
    if !form.website.trim().is_empty() {
        // Silently redirect — don't tell bots they were caught.
        return Redirect::to("/subscribe?subscribed=1").into_response();
    }
    if form.human_check.as_str() != "on" {
        err!("Please confirm you are human.");
    }
    if form.terms.as_str() != "on" {
        err!("You must agree to the Terms of Service to create an account.");
    }

    // ── Validation ────────────────────────────────────────────────────────────
    if form.display_name.trim().is_empty() {
        err!("Name is required.");
    }
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        err!("A valid email address is required.");
    }
    if form.password != form.confirm_password {
        err!("Passwords do not match.");
    }
    if let Err(msg) = crate::models::user::validate_password(&form.password) {
        return Html(admin::pages::subscribe::render(Some(msg), &site_name)).into_response();
    }

    // ── Email already exists? ─────────────────────────────────────────────────
    match crate::models::user::get_by_email(&state.db, &email).await {
        Ok(existing) => {
            // Known user — ensure they have a site_users row for this site.
            match crate::models::site_user::get_role(&state.db, site_id, existing.id).await {
                Ok(Some(_)) => {
                    err!("This email address is already subscribed to this site.");
                }
                _ => {
                    // Not yet linked to this site — add the row.
                    if let Err(e) = crate::models::site_user::add(
                        &state.db,
                        site_id,
                        existing.id,
                        "subscriber",
                        None,
                    )
                    .await
                    {
                        tracing::error!("subscribe: failed to link existing user to site: {:?}", e);
                        err!("Something went wrong. Please try again.");
                    }
                    return Redirect::to("/subscribe?subscribed=1").into_response();
                }
            }
        }
        Err(_) => {
            // New user — generate a username from display name and create the account.
            let username = generate_username(&state.db, form.display_name.trim()).await;
            let create = CreateUser {
                username,
                email: email.clone(),
                display_name: form.display_name.trim().to_string(),
                password: form.password.clone(),
                role: UserRole::Subscriber,
            };

            let new_user = match crate::models::user::create(&state.db, &create).await {
                Ok(u) => u,
                Err(e) => {
                    tracing::error!("subscribe: user creation failed: {:?}", e);
                    err!("Something went wrong. Please try again.");
                }
            };

            // Link to site (skip for nil-UUID fallback used in single-site mode).
            if site_id != Uuid::nil() {
                if let Err(e) = crate::models::site_user::add(
                    &state.db,
                    site_id,
                    new_user.id,
                    "subscriber",
                    None,
                )
                .await
                {
                    tracing::warn!(
                        "subscribe: created user {} but failed to add site_users row: {:?}",
                        new_user.id,
                        e
                    );
                }
            }

            return Redirect::to("/subscribe?subscribed=1").into_response();
        }
    }

    // Unreachable — all code paths above return — but needed to satisfy the
    // compiler's IntoResponse requirement.
    #[allow(unreachable_code)]
    Redirect::to("/subscribe").into_response()
}

/// Derive a unique username from a display name.
/// e.g. "Steve Miller" → "steve-miller", then "steve-miller2" if taken.
async fn generate_username(pool: &sqlx::PgPool, display_name: &str) -> String {
    let base = slug::slugify(display_name);
    let base = if base.is_empty() { "user".to_string() } else { base };

    if !username_taken(pool, &base).await {
        return base;
    }

    // Try sequential suffixes: steve-miller2, steve-miller3, …
    for n in 2u32..=9999 {
        let candidate = format!("{}{}", base, n);
        if !username_taken(pool, &candidate).await {
            return candidate;
        }
    }

    // Last resort: guaranteed-unique UUID suffix.
    format!("{}{}", base, Uuid::new_v4().simple())
}

async fn username_taken(pool: &sqlx::PgPool, username: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap_or(true)
}
