use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::app_state::AppState;
use crate::errors::{AppError, Result};
use crate::models::post::{self, ListFilter, PostContext, PostStatus, PostType};
use crate::templates::context::{
    ContextBuilder, NavContext, PaginationContext, RequestContext, SessionContext, SiteContext,
};

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]
    pub page: i64,
}

fn default_page() -> i64 {
    1
}

/// `GET /` — render the home page (paginated post list).
pub async fn home(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    match render_home(state.clone(), query, uri).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("home handler error: {:?}", e);
            render_error_page(e, &state, &path).await
        }
    }
}

async fn render_home(
    state: AppState,
    query: PageQuery,
    uri: axum::http::Uri,
) -> Result<String> {
    let per_page = state.settings.posts_per_page;
    let offset = (query.page - 1) * per_page;

    let total = post::count(&state.db, Some(PostStatus::Published), Some(PostType::Post)).await?;
    let posts_raw = post::list(
        &state.db,
        &ListFilter {
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            limit: per_page,
            offset,
            ..Default::default()
        },
    )
    .await?;

    // Build PostContext for each post
    let mut posts = Vec::with_capacity(posts_raw.len());
    for p in &posts_raw {
        let ctx = build_post_context(&state, p).await?;
        posts.push(ctx);
    }

    let site_ctx = build_site_context(&state).await?;
    let pagination = PaginationContext::new(query.page, per_page, total, &state.settings.base_url, "");

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", state.settings.base_url, uri.path()),
            path: uri.path().to_string(),
            query: HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("posts", &posts);
    ctx.insert("pagination", &pagination);
    ctx.insert("featured_post", &Option::<PostContext>::None);

    // Pre-render hooks
    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("index.html", &ctx)
}

/// Render an error response, using the active theme's 404.html for NotFound errors.
/// Falls back to plain HTML if the template engine is unavailable.
pub async fn render_error_page(err: AppError, state: &AppState, path: &str) -> Response {
    match err {
        AppError::NotFound(_) => {
            match render_404(state, path).await {
                Ok(html) => (axum::http::StatusCode::NOT_FOUND, Html(html)).into_response(),
                Err(e) => {
                    tracing::warn!("could not render theme 404 page: {:?}", e);
                    (
                        axum::http::StatusCode::NOT_FOUND,
                        Html(format!(
                            r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>404 Not Found</title></head><body><h1>404 — Not Found</h1><p>The page <code>{path}</code> could not be found.</p><p><a href="/">← Back to home</a></p></body></html>"#
                        )),
                    ).into_response()
                }
            }
        }
        _ => {
            tracing::error!("unhandled error in handler: {:?}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Html("<h1>500 Internal Server Error</h1>".to_string()),
            ).into_response()
        }
    }
}

async fn render_404(state: &AppState, path: &str) -> Result<String> {
    let site_ctx = build_site_context(state).await?;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", state.settings.base_url, path),
            path: path.to_string(),
            query: HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("404.html", &ctx)
}

// ── Shared helpers ──────────────────────────────────────────────────────────

pub(crate) async fn build_post_context(
    state: &AppState,
    p: &crate::models::post::Post,
) -> Result<PostContext> {
    use crate::models::{media, taxonomy, user};

    let author = user::get_by_id(&state.db, p.author_id).await?;
    let all_terms = taxonomy::for_post(&state.db, p.id).await?;

    let categories: Vec<_> = all_terms
        .iter()
        .filter(|t| t.taxonomy == "category")
        .collect();
    let tags: Vec<_> = all_terms.iter().filter(|t| t.taxonomy == "tag").collect();

    let mut category_ctxs = Vec::new();
    for c in &categories {
        let count = taxonomy::post_count(&state.db, c.id).await.unwrap_or(0);
        category_ctxs.push(crate::models::taxonomy::TermContext::from_taxonomy(
            c,
            &state.settings.base_url,
            count,
        ));
    }

    let mut tag_ctxs = Vec::new();
    for t in &tags {
        let count = taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
        tag_ctxs.push(crate::models::taxonomy::TermContext::from_taxonomy(
            t,
            &state.settings.base_url,
            count,
        ));
    }

    let featured_image = if let Some(img_id) = p.featured_image_id {
        match media::get_by_id(&state.db, img_id).await {
            Ok(m) => Some(crate::models::media::MediaContext::from_media(
                &m,
                &state.settings.base_url,
            )),
            Err(_) => None,
        }
    } else {
        None
    };

    let meta = post::get_meta(&state.db, p.id).await.unwrap_or_default();

    Ok(PostContext::build(
        p,
        &author,
        category_ctxs,
        tag_ctxs,
        featured_image,
        meta,
        0, // comment_count — Phase 3
        &state.settings.base_url,
    ))
}

pub(crate) async fn build_site_context(state: &AppState) -> Result<SiteContext> {
    let post_count =
        post::count(&state.db, Some(PostStatus::Published), Some(PostType::Post)).await?;
    let page_count =
        post::count(&state.db, Some(PostStatus::Published), Some(PostType::Page)).await?;

    Ok(SiteContext {
        name: state.settings.site_name.clone(),
        description: state.settings.site_description.clone(),
        url: state.settings.base_url.clone(),
        language: state.settings.language.clone(),
        theme: state.settings.active_theme.clone(),
        post_count,
        page_count,
    })
}
