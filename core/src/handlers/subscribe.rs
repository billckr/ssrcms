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
use crate::models::user::{validate_display_name, validate_username, CreateUser, UserRole};

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
    State(state): State<AppState>,
    Query(q): Query<SubscribeQuery>,
    site: CurrentSite,
) -> Response {
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
    if q.subscribed.as_deref() == Some("1") {
        Html(admin::pages::subscribe::render_success(&site.settings.site_name, &default_theme)).into_response()
    } else if !site.settings.allow_registration {
        Html(admin::pages::subscribe::render_closed(&site.settings.site_name, &default_theme)).into_response()
    } else {
        Html(admin::pages::subscribe::render(None, &site.settings.site_name, &default_theme)).into_response()
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
    let default_theme = state.app_settings.read().unwrap().default_theme.clone();

    // Re-checked here, not just on the GET form — a direct POST (bypassing
    // the UI) must not be able to create an account when registration is off.
    if !site.settings.allow_registration {
        return Html(admin::pages::subscribe::render_closed(&site_name, &default_theme)).into_response();
    }

    macro_rules! err {
        ($msg:expr) => {
            return Html(admin::pages::subscribe::render(Some($msg), &site_name, &default_theme)).into_response()
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
    if let Err(msg) = validate_display_name(form.display_name.trim()) {
        err!(msg);
    }
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        err!("A valid email address is required.");
    }
    if form.password != form.confirm_password {
        err!("Passwords do not match.");
    }
    if let Err(msg) = crate::models::user::validate_password(&form.password) {
        return Html(admin::pages::subscribe::render(Some(msg), &site_name, &default_theme)).into_response();
    }

    // ── Email already exists? ─────────────────────────────────────────────────
    match crate::models::user::get_by_email(&state.db, &email).await {
        Ok(existing) => {
            // Known user — ensure they have a site_users row for this site.
            match crate::models::site_user::has_any_role(&state.db, site_id, existing.id).await {
                Ok(true) => {
                    err!("This email address is already subscribed to this site.");
                }
                _ => {
                    // Not yet linked to this site — add the row.
                    if let Err(e) = crate::models::site_user::add(
                        &state.db,
                        site_id,
                        existing.id,
                        crate::models::site_user::SiteRole::Subscriber,
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
            let username = generate_username(&state.db, site_id, form.display_name.trim()).await;
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
                    crate::models::site_user::SiteRole::Subscriber,
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

/// Derive a unique username from a display name that also satisfies
/// [`validate_username`] (5–15 chars, lowercase/digits/hyphens, no leading or
/// trailing hyphen) — since this username is never shown to or confirmed by
/// the user, it must be generated valid rather than relying on them to fix it.
/// e.g. "Steve Miller" → "steve-miller", then "steve-miller2" if taken;
/// "Bo" → too short alone, padded with hex to clear the 5-char minimum.
/// Uniqueness is checked against `site_id` only (usernames aren't globally
/// unique — see `user::username_available`), since that's the only site this
/// new subscriber is about to be linked to. `site_id == Uuid::nil()` (the
/// single-site-mode fallback) skips the check entirely, same as the caller's
/// own nil check before linking `site_users`.
async fn generate_username(pool: &sqlx::PgPool, site_id: Uuid, display_name: &str) -> String {
    const MIN: usize = 5;
    const MAX: usize = 15;

    // slug::slugify already lowercases and restricts to [a-z0-9-], collapsing
    // repeats and trimming edges — matches validate_username's character
    // rules directly; only length still needs enforcing.
    let mut base = slug::slugify(display_name);
    if base.len() > MAX {
        base.truncate(MAX);
        base = base.trim_end_matches('-').to_string();
    }
    if base.len() < MIN {
        // Too short (or empty) to stand alone — pad with hex from a fresh
        // UUID so the base itself clears the 5-char floor before any
        // uniqueness suffix is appended below.
        base.push_str(&Uuid::new_v4().simple().to_string());
        base.truncate(MAX);
        base = base.trim_end_matches('-').to_string();
    }

    if validate_username(&base).is_ok() && !username_taken(pool, site_id, &base).await {
        return base;
    }

    // Try sequential suffixes: steve-miller2, steve-miller3, … — trimming the
    // base as needed so the total stays within MAX.
    for n in 2u32..=9999 {
        let suffix = n.to_string();
        let keep = MAX.saturating_sub(suffix.len());
        let mut candidate: String = base.chars().take(keep).collect();
        candidate = candidate.trim_end_matches('-').to_string();
        candidate.push_str(&suffix);
        if validate_username(&candidate).is_ok() && !username_taken(pool, site_id, &candidate).await {
            return candidate;
        }
    }

    // Last resort: guaranteed-valid (4 + 11 = 15 chars), effectively-guaranteed-unique.
    format!("user{}", &Uuid::new_v4().simple().to_string()[..11])
}

async fn username_taken(pool: &sqlx::PgPool, site_id: Uuid, username: &str) -> bool {
    if site_id == Uuid::nil() {
        return false;
    }
    !crate::models::user::username_available(pool, username, &[site_id], None)
        .await
        .unwrap_or(false)
}
