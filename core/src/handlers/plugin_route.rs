//! Handler for plugin-registered HTTP routes (e.g. /sitemap.xml).
//!
//! When a plugin declares a route in its plugin.toml:
//!
//!   [routes]
//!   "/sitemap.xml" = { template = "seo/sitemap.xml", content_type = "application/xml" }
//!
//! Requests to that path are dispatched here. The handler fetches the appropriate
//! data (all published posts for a sitemap), builds a context, and renders the
//! plugin template.
//!
//! Plugin route handlers are PRESENTATION ONLY — they render a template with data
//! provided by the core. Plugins cannot implement arbitrary request logic.

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

use uuid::Uuid;

use crate::app_state::AppState;
use crate::handlers::home::{build_post_context, build_site_context};
use crate::middleware::site::CurrentSite;
use crate::models::post::{self, ListFilter, PostStatus, PostType};
use crate::templates::context::{ContextBuilder, RequestContext, SessionContext};

/// Hardcoded handler for `/sitemap.xml`.
///
/// Renders `sitemap.xml` from the active theme.  This route is always
/// present regardless of which plugins are installed — no plugin required.
pub async fn sitemap(
    State(state): State<AppState>,
    current_site: CurrentSite,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();

    match render_plugin_route(state, &path, "sitemap.xml", "application/xml", site_id, &base_url).await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(body))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(e) => {
            tracing::error!("sitemap render error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Handler for all plugin-registered routes.
/// The path is resolved against the plugin route registry in AppState.
pub async fn dispatch(
    State(state): State<AppState>,
    current_site: CurrentSite,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();

    let registration = match state.plugin_routes.get(&path) {
        Some(r) => r.clone(),
        None => {
            return (StatusCode::NOT_FOUND, "Not found").into_response();
        }
    };

    match render_plugin_route(state, &path, &registration.template, &registration.content_type, site_id, &base_url).await
    {
        Ok(body) => {
            let content_type = HeaderValue::from_str(&registration.content_type)
                .unwrap_or_else(|_| HeaderValue::from_static("text/plain"));
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            tracing::error!("plugin route '{}' render error: {:?}", path, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn render_plugin_route(
    state: AppState,
    path: &str,
    template_name: &str,
    _content_type: &str,
    site_id: Uuid,
    base_url: &str,
) -> crate::errors::Result<String> {
    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;
    let nav = crate::models::nav_menu::build_nav_context(&state.db, site_id, path).await;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, path),
            path: path.to_string(),
            query: std::collections::HashMap::new(),
        },
        session: SessionContext {
            is_logged_in: false,
            user: None,
        },
        nav,
    }
    .into_tera_context();

    // For sitemap-style routes: inject all published posts and pages for this site.
    // This is the standard context for any route that needs the full content list.
    let all_posts_raw = post::list(
        &state.db,
        &ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            limit: 50_000,
            offset: 0,
            ..Default::default()
        },
    )
    .await?;

    let all_pages_raw = post::list(
        &state.db,
        &ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Page),
            limit: 10_000,
            offset: 0,
            ..Default::default()
        },
    )
    .await?;

    let mut all_posts = Vec::with_capacity(all_posts_raw.len());
    for p in &all_posts_raw {
        all_posts.push(build_post_context(&state, p, base_url).await?);
    }

    let mut all_pages = Vec::with_capacity(all_pages_raw.len());
    for p in &all_pages_raw {
        all_pages.push(build_post_context(&state, p, base_url).await?);
    }

    ctx.insert("all_posts", &all_posts);
    ctx.insert("all_pages", &all_pages);

    let theme = state.active_theme_for_site(Some(site_id));
    state.templates.render_for_theme(&theme, Some(site_id), template_name, &ctx)
}
