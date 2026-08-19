pub mod account;
pub mod admin;
pub mod archive;
pub mod auth;
pub mod comment;
pub mod form;
pub mod poll;
pub mod home;
pub mod metrics;
pub mod page;
pub mod plugin_route;
pub mod post;
pub mod post_unlock;
pub mod recover;
pub mod search;
pub mod subscribe;
pub mod theme_static;
pub mod uploads;

use axum::http::HeaderMap;
use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::account_auth::SESSION_ACCOUNT_USER_ID_KEY;
use crate::middleware::admin_auth::{ADMIN_SESSION_COOKIE_NAME, SESSION_USER_ID_KEY};
use crate::templates::context::{SessionContext, SessionUserContext};

/// Resolve the subscriber session into a `SessionContext` for Tera templates.
/// Never redirects — returns an anonymous context if the session is missing or invalid.
pub(super) async fn resolve_session(state: &AppState, session: &Session) -> SessionContext {
    let user_id_str: Option<String> = session
        .get(SESSION_ACCOUNT_USER_ID_KEY)
        .await
        .unwrap_or(None);
    if let Some(id_str) = user_id_str {
        if let Ok(uid) = id_str.parse::<Uuid>() {
            if let Ok(user) = crate::models::user::get_by_id(&state.db, uid).await {
                return SessionContext {
                    is_logged_in: true,
                    user: Some(SessionUserContext {
                        id: user.id.to_string(),
                        username: user.username.clone(),
                        display_name: user.display_name.clone(),
                        role: user.role.as_str().to_string(),
                    }),
                };
            }
        }
    }
    SessionContext { is_logged_in: false, user: None }
}

/// Resolve this site's active theme's customizer layout options (see
/// `models::theme_options`) and insert them into the Tera context as
/// `theme_options` — a plain `{key: bool}` map templates branch on with
/// `{% if theme_options.some_key %}` — and `theme_option_lists` — a
/// `{key: [item_key, ...]}` map for reorderable option groups, looped over
/// with `{% for item in theme_option_lists.some_key %}` — and
/// `theme_option_texts` — a `{key: string}` map for free-form text options,
/// read with `{{ theme_option_texts.some_key }}` — and `theme_option_images` —
/// a `{key: url}` map for image-picker options (empty string means "use the
/// theme's own default image"), read with `{{ theme_option_images.some_key }}`.
/// A theme that declares none of these (or isn't customizer-enabled) just
/// gets empty maps; never an error.
pub(super) async fn insert_theme_options(ctx: &mut tera::Context, state: &AppState, site_id: Uuid) {
    let theme_name = state.active_theme_for_site(Some(site_id));
    let theme_dir = state.templates.resolve_theme_dir_for_site(&theme_name, Some(site_id));
    let theme_options = crate::models::theme_options::build_theme_options_context(
        &state.db,
        theme_dir.as_deref(),
        site_id,
        &theme_name,
    )
    .await;
    ctx.insert("theme_options", &theme_options);
    let theme_option_lists = crate::models::theme_options::build_theme_option_lists_context(
        &state.db,
        theme_dir.as_deref(),
        site_id,
        &theme_name,
    )
    .await;
    ctx.insert("theme_option_lists", &theme_option_lists);
    let theme_option_choices = crate::models::theme_options::build_theme_option_choices_context(
        &state.db,
        theme_dir.as_deref(),
        site_id,
        &theme_name,
    )
    .await;
    ctx.insert("theme_option_choices", &theme_option_choices);
    let theme_option_texts = crate::models::theme_options::build_theme_option_texts_context(
        &state.db,
        theme_dir.as_deref(),
        site_id,
        &theme_name,
    )
    .await;
    ctx.insert("theme_option_texts", &theme_option_texts);
    let theme_option_images = crate::models::theme_options::build_theme_option_images_context(
        &state.db,
        theme_dir.as_deref(),
        site_id,
        &theme_name,
    )
    .await;
    ctx.insert("theme_option_images", &theme_option_images);
}

/// Whether the current request is an authenticated admin/editor/author staff
/// session with access to `site_id` — used to let logged-in system users
/// preview draft/pending/scheduled posts that the public and subscribers
/// cannot see. Never redirects; a false result just means "no preview access",
/// falling through to the normal published-only lookup.
pub(super) async fn can_preview_site(state: &AppState, headers: &HeaderMap, site_id: Uuid) -> bool {
    let Some(user_id) = admin_user_id_from_cookie(state, headers).await else { return false };
    let Ok(user) = crate::models::user::get_by_id(&state.db, user_id).await else { return false };

    match user.role.as_str() {
        "super_admin" => true,
        "site_admin" | "editor" | "author" => {
            crate::models::site_user::has_any_role(&state.db, site_id, user_id)
                .await
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Reads the *admin* session's user id directly off the `admin_session`
/// cookie, bypassing the `Session` extractor entirely.
///
/// The public/front-end routes (where this is called from) are wired to the
/// separate *account* `SessionManagerLayer` (cookie name `"session"`, see
/// `main.rs`) — an admin logged into `/admin` has no `SESSION_USER_ID_KEY`
/// in that session at all, since admin login only ever writes it into the
/// `admin_session` cookie's session. Without this, `can_preview_site` could
/// never return `true` for anyone who only logged into `/admin`, which is
/// the normal (only) way to log in as staff — the preview feature was
/// silently dead. Both session layers share one Postgres-backed store (just
/// different cookie names), so a session loaded this way is the exact same
/// data an admin route would see via the normal extractor.
async fn admin_user_id_from_cookie(state: &AppState, headers: &HeaderMap) -> Option<Uuid> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    let session_id_str = cookie_header.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(ADMIN_SESSION_COOKIE_NAME)?.strip_prefix('=')
    })?;
    let session_id: tower_sessions::session::Id = session_id_str.parse().ok()?;

    let store = std::sync::Arc::new(tower_sessions_sqlx_store::PostgresStore::new(state.db.clone()));
    let admin_session = Session::new(Some(session_id), store, None);

    let user_id_str: String = admin_session.get(SESSION_USER_ID_KEY).await.ok().flatten()?;
    user_id_str.parse().ok()
}
