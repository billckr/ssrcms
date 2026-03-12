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
    /// Base URL for this request: uses configured `site_url` if set in DB,
    /// otherwise derived from the Host header (e.g. "http://beth.com:3000").
    pub base_url: String,
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
        // Extract the Host header value, keeping the raw value (with port) for URL building.
        let raw_host = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost")
            .to_string();

        // Strip port for site lookup: "example.com:3000" → "example.com"
        let hostname = {
            if let Some(pos) = raw_host.rfind(':') {
                if raw_host[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                    raw_host[..pos].to_string()
                } else {
                    raw_host.clone()
                }
            } else {
                raw_host.clone()
            }
        };

        // Derive a base URL from the Host header. Use the configured site_url from
        // settings if it has been explicitly set (i.e. not the localhost default),
        // otherwise build from the raw host so dev just works without any DB config.
        let make_base_url = |settings: &SiteSettings| -> String {
            if settings.base_url != "http://localhost:3000" {
                settings.base_url.clone()
            } else {
                format!("http://{}", raw_host)
            }
        };

        // Fast path: look up in the site_cache.
        // Validate the cached entry against the DB so that a stale cache (e.g.
        // after `dev reset` without a server restart) doesn't serve ghost sites.
        if let Some((site, settings)) = state.resolve_site(&hostname) {
            match crate::models::site::get_by_hostname(&state.db, &hostname).await {
                Ok(_) => {
                    let base_url = make_base_url(&settings);
                    return Ok(CurrentSite { site, settings, base_url });
                }
                Err(_) => {
                    // Cache is stale — reload it and retry once.
                    tracing::info!("site_cache stale for '{}' — reloading", hostname);
                    let _ = state.reload_site_cache().await;
                    if let Some((site, settings)) = state.resolve_site(&hostname) {
                        let base_url = make_base_url(&settings);
                        return Ok(CurrentSite { site, settings, base_url });
                    }
                    // Hostname is genuinely gone (e.g. after dev reset).
                    // Return 404 directly — do NOT fall into the empty-cache
                    // fallback, which would serve the default theme.
                    return Err(SiteResolutionError::UnknownHostname(hostname));
                }
            }
        }

        // No site matched this hostname — return 404.
        Err(SiteResolutionError::UnknownHostname(hostname))
    }
}
