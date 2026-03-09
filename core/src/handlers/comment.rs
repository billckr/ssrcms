//! Public comment submission handler — POST /blog/:slug/comment.

use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::account_auth::SESSION_ACCOUNT_USER_ID_KEY;
use crate::middleware::site::CurrentSite;
use crate::models::comment::{CreateComment};
use crate::models::post;

#[derive(Deserialize)]
pub struct CommentForm {
    pub body: String,
    #[serde(default)]
    pub parent_id: String,
    /// Present (any value) when the "I'm human" checkbox is checked; absent when unchecked.
    #[serde(default)]
    pub human_check: Option<String>,
}

/// POST /blog/:slug/comment
///
/// Requires an account session (subscriber or above).
/// Validates body length, verifies post has comments enabled,
/// then inserts the comment and redirects back.
pub async fn submit(
    State(state): State<AppState>,
    current_site: CurrentSite,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    session: Session,
    Path(slug): Path<String>,
    Form(form): Form<CommentForm>,
) -> impl IntoResponse {
    let post_url = format!("/blog/{}", slug);

    // Session check — redirect to login if not authenticated.
    let user_id_str: Option<String> = session
        .get(SESSION_ACCOUNT_USER_ID_KEY)
        .await
        .unwrap_or(None);
    let user_id = match user_id_str.and_then(|s| s.parse::<Uuid>().ok()) {
        Some(id) => id,
        None => {
            return Redirect::to(&format!("/login?redirect={}", post_url)).into_response();
        }
    };

    // Human check — reject if checkbox was not ticked.
    if form.human_check.is_none() {
        return Redirect::to(&format!("{}#comments", post_url)).into_response();
    }

    // Validate body.
    let body = form.body.trim().to_string();
    if body.is_empty() || body.len() > 400 {
        return Redirect::to(&format!("{}#comments", post_url)).into_response();
    }

    // Fetch post and verify comments are enabled.
    let post_record = match post::get_published_by_slug(&state.db, Some(current_site.site.id), &slug).await {
        Ok(p) => p,
        Err(_) => return Redirect::to(&post_url).into_response(),
    };
    if !post_record.comments_enabled {
        return Redirect::to(&post_url).into_response();
    }

    // Rate limit: max 2 comments (top-level or reply) per 10 minutes per user.
    let recent: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM comments \
         WHERE author_id = $1 AND created_at > NOW() - INTERVAL '10 minutes'",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if recent >= 2 {
        return Redirect::to(&format!("{}?comment_error=rate_limited#comments", post_url)).into_response();
    }

    // Optional parent_id (only one level of threading — replies to top-level only).
    let parent_id = form.parent_id.trim().parse::<Uuid>().ok();

    let ip_address = headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| Some(peer_addr.ip().to_string()));

    let data = CreateComment {
        post_id:    post_record.id,
        site_id:    Some(current_site.site.id),
        author_id:  user_id,
        parent_id,
        body,
        ip_address,
    };

    if let Err(e) = crate::models::comment::create(&state.db, &data).await {
        tracing::warn!("failed to create comment on post {}: {:?}", post_record.id, e);
    }

    Redirect::to(&format!("{}#comments", post_url)).into_response()
}
