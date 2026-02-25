//! Public form submission handler.
//!
//! `POST /form/{name}` — accepts any HTML form, stores fields as JSONB,
//! and redirects back with `?submitted=1`.
//!
//! Fields whose name starts with `_` (e.g. `_honeypot`) are stripped before
//! storage so they never persist.

use std::collections::HashMap;

use axum::{
    extract::{Form, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect},
};

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::form_submission::{create, CreateFormSubmission};

/// `POST /form/{name}` — store a form submission and redirect.
pub async fn submit(
    State(state): State<AppState>,
    current_site: CurrentSite,
    headers: HeaderMap,
    Path(name): Path<String>,
    Form(fields): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    // Honeypot / internal field stripping — drop any key starting with `_`
    let data: HashMap<String, String> = fields
        .into_iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .collect();

    // Skip storing empty submissions (all fields blank after stripping)
    let is_empty = data.values().all(|v| v.trim().is_empty());

    if !is_empty {
        // Best-effort IP extraction; Caddy sets X-Real-IP
        let ip = headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

        let input = CreateFormSubmission {
            site_id: current_site.site.id,
            form_name: name.clone(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Object(Default::default())),
            ip_address: ip,
        };

        if let Err(e) = create(&state.db, input).await {
            tracing::error!("form submit '{}' error: {:?}", name, e);
        }
    }

    // Redirect back to the page that submitted the form, appending ?submitted=1.
    // Fall back to "/" if the Referer header is missing or unparseable.
    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");

    // Strip any existing query string from the referer before appending ours.
    let base = referer.split('?').next().unwrap_or(referer);
    Redirect::to(&format!("{}?submitted=1", base))
}
