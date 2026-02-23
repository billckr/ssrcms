//! GET /metrics — Prometheus metrics endpoint.
//!
//! Returns metrics in the Prometheus text exposition format (version 0.0.4).
//! Compatible with Prometheus, Grafana, Datadog, CloudWatch, and most other
//! metrics aggregators.
//!
//! Access control: if `METRICS_TOKEN` is configured, the request must include
//! `Authorization: Bearer <token>`. If unset the endpoint is open — restrict
//! access at the network or Caddy level in that case.

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::app_state::AppState;

pub async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = &state.metrics_token {
        let provided = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        if provided != Some(token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "Unauthorized\n").into_response();
        }
    }

    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics_handle.render(),
    )
        .into_response()
}
