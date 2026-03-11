use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use chrono::Local;
use serde::Serialize;
use std::net::SocketAddr;
use tower_sessions::Session;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::post;
use crate::templates::context::{ContextBuilder, NavContext, RequestContext, SessionContext};

#[derive(Serialize)]
struct CommentPaginationContext {
    current_page: usize,
    total_pages:  usize,
    total_count:  usize,
    prev_page:    Option<usize>,
    next_page:    Option<usize>,
    post_url:     String,
}

use super::home::{build_post_context, build_site_context, render_error_page};

/// `GET /blog/:slug` — render a single post.
pub async fn single_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    jar: SignedCookieJar,
    session: Session,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
    let cpage: usize = params.get("cpage").and_then(|v| v.parse().ok()).unwrap_or(1);

    // Password gate: check before full render.
    if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
        if let Some(ref hash) = post_record.post_password {
            if !super::post_unlock::is_unlocked(&jar, post_record.id, hash) {
                return super::post_unlock::gate_response(
                    &post_record.title,
                    &format!("/blog/{}/unlock", slug),
                    None,
                );
            }
        }
    }

    // Resolve subscriber session (optional — never fails).
    let session_ctx = super::resolve_session(&state, &session).await;

    // Record a unique view (skips bots and logged-in account users).
    if !session_ctx.is_logged_in {
        let ua = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !is_bot(ua) {
            let client_ip = real_ip(&headers, addr);
            let ip_hash = anonymize_ip(&client_ip);
            // Resolve post_id without a second DB query by re-fetching only if needed.
            // We use a lightweight slug→id lookup path here.
            if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
                let today = Local::now().date_naive();
                // Fire-and-forget: send() on an UnboundedSender is non-blocking
                // and lock-free — it enqueues immediately regardless of how many
                // other requests are doing the same thing concurrently.
                // The background flush task deduplicates and persists every 60 s.
                let _ = state.view_buffer.send((post_record.id, ip_hash, today));
            }
        }
    }

    match render_post(state.clone(), slug, uri, site_id, &base_url, session_ctx, cpage).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path, Some(current_site.site.id)).await,
    }
}

/// Return the real client IP, preferring the X-Real-IP header set by Caddy,
/// then X-Forwarded-For, finally the socket address.
fn real_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    if let Some(v) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return v.trim().to_string();
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            return first.trim().to_string();
        }
    }
    addr.ip().to_string()
}

/// Anonymize an IP address: zero the last IPv4 octet or the last 80 bits of
/// an IPv6 address. No new crate required — pure string manipulation.
fn anonymize_ip(ip: &str) -> String {
    if let Ok(parsed) = ip.parse::<std::net::IpAddr>() {
        match parsed {
            std::net::IpAddr::V4(v4) => {
                let o = v4.octets();
                return format!("{}.{}.{}.0", o[0], o[1], o[2]);
            }
            std::net::IpAddr::V6(v6) => {
                let mut segs = v6.segments();
                // Zero the last 5 segments (80 bits).
                for s in segs.iter_mut().skip(3) {
                    *s = 0;
                }
                return std::net::Ipv6Addr::from(segs).to_string();
            }
        }
    }
    // Fallback: return as-is (shouldn't happen in practice).
    ip.to_string()
}

/// Returns true if the User-Agent looks like a bot/crawler.
fn is_bot(ua: &str) -> bool {
    if ua.is_empty() {
        return true;
    }
    let lower = ua.to_lowercase();
    ["bot", "crawler", "spider", "slurp", "curl", "wget", "python", "go-http", "java/", "libwww"]
        .iter()
        .any(|kw| lower.contains(kw))
}

async fn render_post(
    state: AppState,
    slug: String,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
    session_ctx: SessionContext,
    cpage: usize,
) -> crate::errors::Result<String> {
    let post_record = post::get_published_by_slug(&state.db, Some(site_id), &slug).await?;

    let post_ctx = build_post_context(&state, &post_record, base_url).await?;

    let prev = if let Some(pub_at) = post_record.published_at {
        match post::get_prev(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p, base_url).await?),
            None => None,
        }
    } else {
        None
    };

    let next = if let Some(pub_at) = post_record.published_at {
        match post::get_next(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p, base_url).await?),
            None => None,
        }
    } else {
        None
    };

    let related_raw = post::get_related(&state.db, post_record.site_id, post_record.id, 5).await?;
    let mut related = Vec::with_capacity(related_raw.len());
    for p in &related_raw {
        related.push(build_post_context(&state, p, base_url).await?);
    }

    // Fetch comments if enabled.
    const PER_PAGE: usize = 10;
    let comment_page = if post_record.comments_enabled {
        crate::models::comment::list_for_post(&state.db, post_record.id, cpage, PER_PAGE)
            .await
            .unwrap_or_else(|_| crate::models::comment::CommentPage {
                comments:     vec![],
                current_page: 1,
                total_pages:  1,
                total_count:  0,
            })
    } else {
        crate::models::comment::CommentPage {
            comments:     vec![],
            current_page: 1,
            total_pages:  1,
            total_count:  0,
        }
    };

    let comment_pagination = CommentPaginationContext {
        current_page: comment_page.current_page,
        total_pages:  comment_page.total_pages,
        total_count:  comment_page.total_count,
        prev_page:    if comment_page.current_page > 1 { Some(comment_page.current_page - 1) } else { None },
        next_page:    if comment_page.current_page < comment_page.total_pages { Some(comment_page.current_page + 1) } else { None },
        post_url:     format!("/blog/{}", slug),
    };

    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;

    // Check whether the logged-in subscriber has saved this post (before session_ctx is moved).
    let is_saved = if let Some(ref u) = session_ctx.user {
        if let Ok(uid) = u.id.parse::<Uuid>() {
            crate::models::saved_post::is_saved(&state.db, uid, post_record.id)
                .await
                .unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    let query_params: std::collections::HashMap<String, String> = uri.query()
        .map(|q| q.split('&').filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            Some((k, v))
        }).collect())
        .unwrap_or_default();

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, uri.path()),
            path: uri.path().to_string(),
            query: query_params,
        },
        session: session_ctx,
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("post", &post_ctx);
    ctx.insert("is_saved", &is_saved);
    ctx.insert("prev_post", &prev);
    ctx.insert("next_post", &next);
    ctx.insert("related_posts", &related);
    ctx.insert("comments", &comment_page.comments);
    ctx.insert("comment_pagination", &comment_pagination);

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
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render_for_theme(&theme, Some(site_id), "single.html", &ctx)
}

// ── Save / Unsave post ────────────────────────────────────────────────────────

/// `POST /blog/:slug/save` — save a post to the subscriber's reading list.
pub async fn save_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    session: Session,
) -> Response {
    let redirect = axum::response::Redirect::to(&format!("/blog/{}", slug));
    let session_ctx = super::resolve_session(&state, &session).await;
    let Some(ref u) = session_ctx.user else {
        return redirect.into_response();
    };
    let Ok(uid) = u.id.parse::<Uuid>() else {
        return redirect.into_response();
    };
    let site_id = current_site.site.id;
    if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
        let _ = crate::models::saved_post::save(&state.db, uid, post_record.id, Some(site_id)).await;
    }
    redirect.into_response()
}

/// `POST /blog/:slug/unsave` — remove a post from the subscriber's reading list.
pub async fn unsave_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    session: Session,
) -> Response {
    let redirect = axum::response::Redirect::to(&format!("/blog/{}", slug));
    let session_ctx = super::resolve_session(&state, &session).await;
    let Some(ref u) = session_ctx.user else {
        return redirect.into_response();
    };
    let Ok(uid) = u.id.parse::<Uuid>() else {
        return redirect.into_response();
    };
    let site_id = current_site.site.id;
    if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
        let _ = crate::models::saved_post::unsave(&state.db, uid, post_record.id).await;
    }
    redirect.into_response()
}
