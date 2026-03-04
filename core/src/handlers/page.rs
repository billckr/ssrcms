use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::post::{self, PostType};
use crate::templates::context::{ContextBuilder, NavContext, RequestContext, SessionContext};

use super::home::{build_post_context, build_site_context, render_error_page};

/// `GET /:slug` — render a static page.
pub async fn single_page(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    jar: CookieJar,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();

    // Password gate: check before full render.
    if let Ok(post_record) = post::get_published_by_slug(&state.db, Some(site_id), &slug).await {
        if post_record.post_password.is_some()
            && !super::post_unlock::is_unlocked(&jar, post_record.id)
        {
            return super::post_unlock::gate_response(
                &post_record.title,
                &format!("/{}/unlock", slug),
                None,
            );
        }
    }

    match render_page(state.clone(), slug, uri, site_id, &base_url).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path, Some(current_site.site.id)).await,
    }
}

async fn render_page(
    state: AppState,
    slug: String,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
) -> crate::errors::Result<String> {
    // Look up a published page (post_type = 'page') by slug
    let post_record = post::get_published_by_slug(&state.db, Some(site_id), &slug).await?;

    // Verify it is actually a page
    if post_record.post_type != PostType::Page.as_str() {
        return Err(crate::errors::AppError::NotFound(format!("page '{slug}'")));
    }

    let page_ctx = build_post_context(&state, &post_record, base_url).await?;
    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, uri.path()),
            path: uri.path().to_string(),
            query: uri.query()
                .map(parse_query_string)
                .unwrap_or_default(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("page", &page_ctx);

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