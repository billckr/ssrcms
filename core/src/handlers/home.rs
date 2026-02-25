use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::errors::{AppError, Result};
use crate::middleware::site::CurrentSite;
use crate::models::post::{self, ListFilter, PostContext, PostStatus, PostType};
use crate::models::taxonomy::{self, TaxonomyType};
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
    current_site: CurrentSite,
    Query(query): Query<PageQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
    match render_home(state.clone(), query, uri, site_id, &base_url).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("home handler error: {:?}", e);
            render_error_page(e, &state, &path, Some(current_site.site.id)).await
        }
    }
}

async fn render_home(
    state: AppState,
    query: PageQuery,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
) -> Result<String> {
    let per_page = state.get_site_by_id(site_id)
        .map(|(_, s)| s.posts_per_page)
        .unwrap_or(state.settings.posts_per_page);
    let offset = (query.page - 1) * per_page;

    let total = post::count(&state.db, Some(site_id), Some(PostStatus::Published), Some(PostType::Post)).await?;
    let posts_raw = post::list(
        &state.db,
        &ListFilter {
            site_id: Some(site_id),
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
        let ctx = build_post_context(&state, p, base_url).await?;
        posts.push(ctx);
    }

    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;
    let pagination = PaginationContext::new(query.page, per_page, total, base_url, "");

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, uri.path()),
            path: uri.path().to_string(),
            query: HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    // Build tag cloud for sidebar
    let raw_tags = taxonomy::list(&state.db, Some(site_id), TaxonomyType::Tag).await.unwrap_or_default();
    let mut tag_cloud = Vec::with_capacity(raw_tags.len());
    for t in &raw_tags {
        let count = taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
        if count > 0 {
            tag_cloud.push(taxonomy::TermContext::from_taxonomy(t, base_url, count));
        }
    }

    // Build category cloud for sidebar
    let raw_cats = taxonomy::list(&state.db, Some(site_id), TaxonomyType::Category).await.unwrap_or_default();
    let mut category_cloud = Vec::with_capacity(raw_cats.len());
    for c in &raw_cats {
        let count = taxonomy::post_count(&state.db, c.id).await.unwrap_or(0);
        if count > 0 {
            category_cloud.push(taxonomy::TermContext::from_taxonomy(c, base_url, count));
        }
    }

    ctx.insert("posts", &posts);
    ctx.insert("pagination", &pagination);
    ctx.insert("featured_post", &Option::<PostContext>::None);
    ctx.insert("tag_cloud", &tag_cloud);
    ctx.insert("category_cloud", &category_cloud);

    let theme = state.active_theme_for_site(Some(site_id));

    // Pre-render hooks
    let hook_outputs = state.templates.render_hooks_for_theme(
        &theme,
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render_for_theme(&theme, "index.html", &ctx)
}

/// Render an error response, using the active theme's 404.html for NotFound errors.
/// Falls back to plain HTML if the template engine is unavailable.
pub async fn render_error_page(err: AppError, state: &AppState, path: &str, site_id: Option<uuid::Uuid>) -> Response {
    match err {
        AppError::NotFound(_) => {
            match render_404(state, path, site_id).await {
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

async fn render_404(state: &AppState, path: &str, site_id: Option<uuid::Uuid>) -> Result<String> {
    let base_url = &state.settings.base_url;
    let site_ctx = build_site_context(state, site_id, base_url).await?;

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", base_url, path),
            path: path.to_string(),
            query: HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    let theme = state.active_theme_for_site(site_id);
    let hook_outputs = state.templates.render_hooks_for_theme(
        &theme,
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render_for_theme(&theme, "404.html", &ctx)
}

// ── Shared helpers ──────────────────────────────────────────────────────────

pub(crate) async fn build_post_context(
    state: &AppState,
    p: &crate::models::post::Post,
    base_url: &str,
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
            base_url,
            count,
        ));
    }

    let mut tag_ctxs = Vec::new();
    for t in &tags {
        let count = taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
        tag_ctxs.push(crate::models::taxonomy::TermContext::from_taxonomy(
            t,
            base_url,
            count,
        ));
    }

    let featured_image = if let Some(img_id) = p.featured_image_id {
        match media::get_by_id(&state.db, img_id).await {
            Ok(m) => Some(crate::models::media::MediaContext::from_media(
                &m,
                base_url,
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
        base_url,
    ))
}

pub(crate) async fn build_site_context(state: &AppState, site_id: Option<Uuid>, base_url: &str) -> Result<SiteContext> {
    let post_count =
        post::count(&state.db, site_id, Some(PostStatus::Published), Some(PostType::Post)).await?;
    let page_count =
        post::count(&state.db, site_id, Some(PostStatus::Published), Some(PostType::Page)).await?;

    // Use per-site settings from the cache when a site_id is available,
    // falling back to global settings for single-site / unconfigured installs.
    let settings;
    let s: &crate::app_state::SiteSettings = if let Some(sid) = site_id {
        if let Some((_, per_site)) = state.get_site_by_id(sid) {
            settings = per_site;
            &settings
        } else {
            &state.settings
        }
    } else {
        &state.settings
    };

    Ok(SiteContext {
        name: s.site_name.clone(),
        description: s.site_description.clone(),
        url: base_url.to_string(),
        language: s.language.clone(),
        theme: s.active_theme.clone(),
        post_count,
        page_count,
    })
}
