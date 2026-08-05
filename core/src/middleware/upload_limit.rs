//! Dynamic upload-size gate for multipart upload routes.
//!
//! `DefaultBodyLimit` is a static tower layer fixed at router-build time, so it
//! can't reflect the admin-configurable, DB-backed max_upload_mb setting
//! (`state.app_settings`, saved from /admin/settings) without a restart. This
//! gate re-checks the request's Content-Length against the live setting on
//! every request instead, so raising/lowering the limit takes effect
//! immediately. It's paired in the router with a large fixed DefaultBodyLimit
//! that acts as an absolute safety net against unbounded bodies.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::app_state::AppState;

pub async fn gate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let max_bytes = {
        let s = state.app_settings.read().unwrap();
        (s.max_upload_mb.max(1) as u64).saturating_mul(1024 * 1024)
    };

    let content_length = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    if let Some(len) = content_length {
        if len > max_bytes {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Upload too large").into_response();
        }
    }

    next.run(req).await
}
