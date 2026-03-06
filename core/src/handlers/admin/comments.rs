//! Admin comment management — delete individual comments.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

#[derive(Deserialize, Default)]
pub struct DeleteCommentForm {
    /// URL to redirect back to after deletion (e.g. the post URL).
    #[serde(default)]
    pub redirect: String,
}

/// POST /admin/comments/:id/delete
///
/// Editors and site admins (and above) may delete any comment.
/// Authors may not delete comments.
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<DeleteCommentForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_content {
        return Redirect::to("/admin").into_response();
    }

    if let Err(e) = crate::models::comment::delete(&state.db, id).await {
        tracing::warn!("failed to delete comment {}: {:?}", id, e);
    }

    let destination = if form.redirect.is_empty() { "/admin".to_string() } else { form.redirect };
    Redirect::to(&destination).into_response()
}
