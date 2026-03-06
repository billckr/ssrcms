//! Public comment submission handler — POST /blog/:slug/comment.

use axum::{
    extract::{Path, State},
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
}

/// POST /blog/:slug/comment
///
/// Requires an account session (subscriber or above).
/// Validates body length, verifies post has comments enabled,
/// then inserts the comment and redirects back.
pub async fn submit(
    State(state): State<AppState>,
    current_site: CurrentSite,
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

    // Validate body.
    let body = form.body.trim().to_string();
    if body.is_empty() || body.len() > 2000 {
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

    // Optional parent_id (only one level of threading — replies to top-level only).
    let parent_id = form.parent_id.trim().parse::<Uuid>().ok();

    let data = CreateComment {
        post_id: post_record.id,
        site_id: Some(current_site.site.id),
        author_id: user_id,
        parent_id,
        body,
    };

    if let Err(e) = crate::models::comment::create(&state.db, &data).await {
        tracing::warn!("failed to create comment on post {}: {:?}", post_record.id, e);
    }

    Redirect::to(&format!("{}#comments", post_url)).into_response()
}
