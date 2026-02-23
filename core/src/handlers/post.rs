use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Response},
};

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::post;
use crate::templates::context::{ContextBuilder, NavContext, RequestContext, SessionContext};

use super::home::{build_post_context, build_site_context, render_error_page};

/// `GET /blog/:slug` — render a single post.
pub async fn single_post(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    match render_post(state.clone(), slug, uri, site_id).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path).await,
    }
}

async fn render_post(
    state: AppState,
    slug: String,
    uri: axum::http::Uri,
    site_id: Uuid,
) -> crate::errors::Result<String> {
    let post_record = post::get_published_by_slug(&state.db, Some(site_id), &slug).await?;

    let post_ctx = build_post_context(&state, &post_record).await?;

    let prev = if let Some(pub_at) = post_record.published_at {
        match post::get_prev(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p).await?),
            None => None,
        }
    } else {
        None
    };

    let next = if let Some(pub_at) = post_record.published_at {
        match post::get_next(&state.db, post_record.site_id, pub_at).await? {
            Some(p) => Some(build_post_context(&state, &p).await?),
            None => None,
        }
    } else {
        None
    };

    let related_raw = post::get_related(&state.db, post_record.site_id, post_record.id, 5).await?;
    let mut related = Vec::with_capacity(related_raw.len());
    for p in &related_raw {
        related.push(build_post_context(&state, p).await?);
    }

    let site_ctx = build_site_context(&state, Some(site_id)).await?;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", state.settings.base_url, uri.path()),
            path: uri.path().to_string(),
            query: std::collections::HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("post", &post_ctx);
    ctx.insert("prev_post", &prev);
    ctx.insert("next_post", &next);
    ctx.insert("related_posts", &related);

    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("single.html", &ctx)
}
