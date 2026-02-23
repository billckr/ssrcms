//! Site resolution middleware.
//!
//! `CurrentSite` is an Axum extractor that resolves the current site from the
//! Host request header, returning either the matching (Site, SiteSettings) pair
//! from the site_cache or a 404 response if the hostname is unknown.
//!
//! When the site_cache is empty (no sites configured yet), the extractor falls
//! back gracefully to the default settings in AppState.  This preserves
//! single-site backward compatibility.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};

use crate::app_state::{AppState, SiteSettings};
use crate::models::site::Site;

/// The resolved current site, extracted from the Host header.
pub struct CurrentSite {
    pub site: Site,
    pub settings: SiteSettings,
}

pub enum SiteResolutionError {
    UnknownHostname(String),
}

impl IntoResponse for SiteResolutionError {
    fn into_response(self) -> Response {
        match self {
            SiteResolutionError::UnknownHostname(hostname) => {
                tracing::warn!("unknown hostname in request: '{}'", hostname);
                (
                    StatusCode::NOT_FOUND,
                    format!("No site found for hostname '{hostname}'"),
                )
                    .into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for CurrentSite {
    type Rejection = SiteResolutionError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract the Host header value (strip port if present).
        let hostname = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok())
            .map(|h| {
                // Strip port: "example.com:3000" → "example.com"
                if let Some(pos) = h.rfind(':') {
                    // Only strip if what follows looks like a port (all digits)
                    if h[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                        return &h[..pos];
                    }
                }
                h
            })
            .unwrap_or("localhost")
            .to_string();

        // Fast path: look up in the site_cache.
        if let Some((site, settings)) = state.resolve_site(&hostname) {
            return Ok(CurrentSite { site, settings });
        }

        // If site_cache is empty (no sites configured yet), fall back to
        // the default settings so single-site installs keep working.
        if state.site_cache.read().map(|c| c.is_empty()).unwrap_or(true) {
            use uuid::Uuid;
            use chrono::Utc;
            let fallback_site = Site {
                id: Uuid::nil(),
                hostname: hostname.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            return Ok(CurrentSite {
                site: fallback_site,
                settings: (*state.settings).clone(),
            });
        }

        Err(SiteResolutionError::UnknownHostname(hostname))
    }
}
