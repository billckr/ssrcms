pub mod admin;
pub mod archive;
pub mod auth;
pub mod home;
pub mod page;
pub mod plugin_route;
pub mod post;
pub mod search;
pub mod theme_static;

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::errors::AppError;

/// Wrap an Html string in a 200 response.
pub fn html_ok(body: String) -> Response {
    Html(body).into_response()
}

/// Convert AppError into a proper HTTP response, rendering the 404 template when possible.
pub fn error_response(_err: AppError, status: StatusCode, body: String) -> Response {
    (status, Html(body)).into_response()
}
