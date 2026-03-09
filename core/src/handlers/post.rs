use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use tower_sessions::Session;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::account_auth::SESSION_ACCOUNT_USER_ID_KEY;
use crate::middleware::site::CurrentSite;
use crate::models::post;
use crate::templates::context::{ContextBuilder, NavContext, RequestContext, SessionContext, SessionUserContext};

use super::home::{build_post_context, build_site_context, render_error_page};

/// `GET /blog/:slug` — render a single post.
pub async fn single_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    jar: SignedCookieJar,
    session: Session,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
    let cpage: usize = params.get("cpage").and_then(|v| v.parse().ok()).unwrap_or(1);

    // Password gate: check before full render.
    if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
        if let Some(ref hash) = post_record.post_password {
            if !super::post_unlock::is_unlocked(&jar, post_record.id, hash) {
                return super::post_unlock::gate_response(
                    &post_record.title,
                    &format!("/blog/{}/unlock", slug),
                    None,
                );
            }
        }
    }

    // Resolve subscriber session (optional — never fails).
    let session_ctx = resolve_session(&state, &session).await;

    match render_post(state.clone(), slug, uri, site_id, &base_url, session_ctx, cpage).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path, Some(current_site.site.id)).await,
    }
}

/// Read the account session and build a SessionContext — never redirects.
async fn resolve_session(state: &AppState, session: &Session) -> SessionContext {
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

async fn render_post(
    state: AppState,
    slug: String,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
    session_ctx: SessionContext,
    cpage: usize,
) -> crate::errors::Result<String> {
    let post_record = post::get_published_by_slug(&state.db, Some(site_id), &slug).await?;

    let post_ctx = build_post_context(&state, &post_record, base_url).await?;

    let prev = if let Some(pub_at) = post_record.published_at {
        match post::get_prev(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p, base_url).await?),
            None => None,
        }
    } else {
        None
    };

    let next = if let Some(pub_at) = post_record.published_at {
        match post::get_next(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p, base_url).await?),
            None => None,
        }
    } else {
        None
    };

    let related_raw = post::get_related(&state.db, post_record.site_id, post_record.id, 5).await?;
    let mut related = Vec::with_capacity(related_raw.len());
    for p in &related_raw {
        related.push(build_post_context(&state, p, base_url).await?);
    }

    // Fetch comments if enabled.
    const PER_PAGE: usize = 10;
    let comment_page = if post_record.comments_enabled {
        crate::models::comment::list_for_post(&state.db, post_record.id, cpage, PER_PAGE)
            .await
            .unwrap_or_else(|_| crate::models::comment::CommentPage {
                comments:     vec![],
                current_page: 1,
                total_pages:  1,
                total_count:  0,
            })
    } else {
        crate::models::comment::CommentPage {
            comments:     vec![],
            current_page: 1,
            total_pages:  1,
            total_count:  0,
        }
    };

    let post_url = format!("/blog/{}", slug);
    let comment_pagination = serde_json::json!({
        "current_page": comment_page.current_page,
        "total_pages":  comment_page.total_pages,
        "total_count":  comment_page.total_count,
        "prev_page":    if comment_page.current_page > 1 { Some(comment_page.current_page - 1) } else { None },
        "next_page":    if comment_page.current_page < comment_page.total_pages { Some(comment_page.current_page + 1) } else { None },
        "post_url":     post_url,
    });

    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, uri.path()),
            path: uri.path().to_string(),
            query: std::collections::HashMap::new(),
        },
        session: session_ctx,
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("post", &post_ctx);
    ctx.insert("prev_post", &prev);
    ctx.insert("next_post", &next);
    ctx.insert("related_posts", &related);
    ctx.insert("comments", &comment_page.comments);
    ctx.insert("comment_pagination", &comment_pagination);

    let active_plugins = crate::models::site_plugin::active_plugin_names(&state.db, site_id)
        .await
        .unwrap_or_default();
    let theme = state.active_theme_for_site(Some(site_id));
    let hook_outputs = state.templates.render_hooks_for_theme(
        &theme,
        Some(site_id),
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    Some(&active_plugins));
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render_for_theme(&theme, Some(site_id), "single.html", &ctx)
}
