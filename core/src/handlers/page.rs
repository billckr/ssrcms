use axum::{
    extract::{ConnectInfo, Query, State},
    response::{Html, IntoResponse, Response, Redirect},
    http::{header, HeaderMap},
};
use axum_extra::extract::cookie::SignedCookieJar;
use std::net::SocketAddr;
use tower_sessions::Session;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::post::{self, ListFilter, PostStatus, PostType};
use crate::templates::context::{ContextBuilder, RequestContext, SessionContext};

use super::home::{build_post_context, build_site_context, render_error_page};

/// Fallback handler — render a static page, supporting nested paths like /services/service-1.
/// Also the entry point for decorated post permalinks (e.g.
/// `/2026/08/my-post`) — any multi-segment path that doesn't resolve to a
/// page hierarchy is retried as a post permalink by its LAST segment; see
/// `SiteSettings::permalink_structure`'s doc comment for why.
#[allow(clippy::too_many_arguments)]
pub async fn single_page(
    State(state): State<AppState>,
    current_site: CurrentSite,
    session: Session,
    jar: SignedCookieJar,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let request_path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
    let cpage: usize = params.get("cpage").and_then(|v| v.parse().ok()).unwrap_or(1);

    // Split URI path into segments, filtering empty parts from leading/trailing slashes
    let path = uri.path().trim_start_matches('/').to_string();
    let segments: Vec<&str> = path.split('/').filter(|s: &&str| !s.is_empty()).collect();
    let slug = segments.first().copied().unwrap_or("");

    // Password gate: only applies to top-level pages (no parent).
    // Nested pages skip the password gate in MVP.
    if segments.len() == 1 {
        if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), slug).await {
            if let Some(ref hash) = post_record.post_password {
                if post_record.parent_id.is_none()
                    && !super::post_unlock::is_unlocked(&jar, post_record.id, hash)
                {
                    let default_theme = state.app_settings.read().unwrap().default_theme.clone();
                    return super::post_unlock::gate_response(
                        &post_record.title,
                        &format!("/{}/unlock", slug),
                        None,
                        &default_theme,
                    );
                }
            }
        }
    }

    // Detect feed template early so we can set the correct Content-Type.
    let is_feed = if segments.len() == 1 {
        post::get_published_by_slug(&state.db, Some(site_id), slug)
            .await
            .ok()
            .and_then(|p| p.template)
            .map(|t| t == "feed")
            .unwrap_or(false)
    } else {
        false
    };

    let session_ctx = super::resolve_session(&state, &session).await;
    let preview_allowed = super::can_preview_site(&state, &session, site_id).await;
    let page_result = render_page(state.clone(), segments.clone(), uri.clone(), site_id, &base_url, session_ctx, preview_allowed).await;

    match page_result {
        Ok(xml) if is_feed => (
            [(header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")],
            xml,
        ).into_response(),
        Ok(html) => Html(html).into_response(),
        Err(crate::errors::AppError::NotFound(_)) if segments.len() > 1 => {
            match try_post_permalink(&state, site_id, &base_url, &segments, &request_path, &uri, &jar, &session, addr, &headers, cpage).await {
                Some(resp) => resp,
                None => render_error_page(
                    crate::errors::AppError::NotFound(request_path.clone()),
                    &state,
                    &request_path,
                    Some(site_id),
                ).await,
            }
        }
        Err(e) => render_error_page(e, &state, &request_path, Some(current_site.site.id)).await,
    }
}

/// Try resolving an unmatched multi-segment path as a decorated post
/// permalink by its LAST segment. Returns `None` when the last segment
/// doesn't match any published post (caller renders the normal 404).
///
/// A match always redirects to the post's current canonical URL rather than
/// rendering in place — this self-corrects any mismatched decorative
/// segments (stale dates, a renamed category, `permalink_structure` having
/// changed since the link was published) onto one true URL rather than
/// serving the same content at infinitely many path variations, which is
/// bad for SEO. The one exception is when the request already exactly
/// matches the canonical path, to avoid a pointless redirect-to-self.
#[allow(clippy::too_many_arguments)]
async fn try_post_permalink(
    state: &AppState,
    site_id: Uuid,
    base_url: &str,
    segments: &[&str],
    request_path: &str,
    uri: &axum::http::Uri,
    jar: &SignedCookieJar,
    session: &Session,
    addr: SocketAddr,
    headers: &HeaderMap,
    cpage: usize,
) -> Option<Response> {
    let slug = segments.last().copied()?;
    let post_record = post::get_published_by_slug(&state.db, Some(site_id), slug).await.ok()?;
    if post_record.post_type != PostType::Post.as_str() {
        return None;
    }

    let structure = crate::app_state::get_site_setting(&state.db, site_id, "permalink_structure")
        .await
        .unwrap_or_else(|| "/%postname%".to_string());
    let category_slug = crate::models::taxonomy::for_post(&state.db, post_record.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|t| t.taxonomy == "category")
        .map(|t| t.slug);
    let canonical_path = post::build_permalink(&structure, &post_record, category_slug.as_deref());

    // Compare ignoring a trailing slash either side — both shapes are the
    // "same" URL, no need to redirect one to the other.
    let normalize = |p: &str| p.trim_end_matches('/').to_string();
    if normalize(request_path) == normalize(&canonical_path) {
        let slug = post_record.slug.clone();
        return Some(super::post::render_single_post_response(
            state, site_id, base_url, slug, uri.clone(), jar, session, addr, headers, cpage,
        ).await);
    }

    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    Some(Redirect::permanent(&format!("{canonical_path}{query}")).into_response())
}

pub(super) async fn render_page(
    state: AppState,
    segments: Vec<&str>,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
    session_ctx: SessionContext,
    preview_allowed: bool,
) -> crate::errors::Result<String> {
    // Look up the page: single segment = slug lookup, multiple = hierarchical path.
    // Preview-allowed staff sessions fall back to an any-status lookup when the
    // published-only lookup finds nothing, so draft/pending pages resolve too.
    let post_record = if segments.len() == 1 {
        match post::get_published_by_slug(&state.db, Some(site_id), segments[0]).await {
            Ok(p) => p,
            Err(e) => {
                if preview_allowed {
                    post::get_by_slug(&state.db, Some(site_id), segments[0]).await?
                } else {
                    return Err(e);
                }
            }
        }
    } else {
        match post::get_page_by_path(&state.db, Some(site_id), &segments).await {
            Ok(p) => p,
            Err(e) => {
                if preview_allowed {
                    post::get_page_by_path_any_status(&state.db, Some(site_id), &segments).await?
                } else {
                    return Err(e);
                }
            }
        }
    };

    // Verify it is actually a page
    if post_record.post_type != PostType::Page.as_str() {
        return Err(crate::errors::AppError::NotFound(format!(
            "page '{}'",
            segments.join("/")
        )));
    }

    let page_ctx = build_post_context(&state, &post_record, base_url).await?;
    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;
    let nav = crate::models::nav_menu::build_nav_context(&state.db, site_id, uri.path()).await;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, uri.path()),
            path: uri.path().to_string(),
            query: uri.query()
                .map(parse_query_string)
                .unwrap_or_default(),
        },
        session: session_ctx,
        nav,
    }
    .into_tera_context();
    super::insert_theme_options(&mut ctx, &state, site_id).await;

    ctx.insert("page", &page_ctx);

    // For the RSS feed template, inject the 20 most recent published posts.
    let template_name_raw = post_record
        .template
        .as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or("page");
    if template_name_raw == "feed" {
        let feed_posts = post::list(&state.db, &ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            limit: 20,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
        let mut feed_post_ctxs = Vec::with_capacity(feed_posts.len());
        for p in &feed_posts {
            if let Ok(pctx) = build_post_context(&state, p, base_url).await {
                feed_post_ctxs.push(pctx);
            }
        }
        ctx.insert("posts", &feed_post_ctxs);
    }

    let active_plugins = crate::models::site_plugin::active_plugin_names(&state.db, site_id)
        .await
        .unwrap_or_default();
    let theme = state.active_theme_for_site(Some(site_id));
    let hook_outputs = state.templates.render_hooks_for_theme(
        &theme,
        Some(site_id),
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    Some(&active_plugins));
    crate::templates::context::ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    // Use the page-specific template if set, otherwise fall back to page.html
    let template_name = post_record
        .template
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| format!("{}.html", t))
        .unwrap_or_else(|| "page.html".to_string());

    state.templates.render_for_theme(&theme, Some(site_id), &template_name, &ctx)
}
/// Parse `key=value&key2=value2` query strings into a HashMap.
/// Percent-decoding is intentionally minimal (+ → space, %XX → char).
fn parse_query_string(raw: &str) -> std::collections::HashMap<String, String> {
    raw.split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().filter(|k| !k.is_empty())?;
            let val = parts.next().unwrap_or("");
            Some((url_decode(key), url_decode(val)))
        })
        .collect()
}

/// Minimal percent-decode: replaces `+` with space and `%XX` hex pairs.
fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(a), Some(b)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", a, b), 16) {
                    out.push(byte as char);
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}
