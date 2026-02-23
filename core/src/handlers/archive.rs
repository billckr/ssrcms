use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;

use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::{post, taxonomy};
use crate::models::post::{ListFilter, PostStatus, PostType};
use crate::models::taxonomy::{TaxonomyType, TermContext};
use crate::templates::context::{
    ArchiveContext, ContextBuilder, NavContext, PaginationContext, RequestContext, SessionContext,
};

use super::home::{build_post_context, build_site_context, render_error_page};

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]
    page: i64,
}

fn default_page() -> i64 {
    1
}

/// `GET /category/:slug`
pub async fn category_archive(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    Query(query): Query<PageQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    match render_taxonomy_archive(state.clone(), slug, TaxonomyType::Category, query.page, uri, site_id).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path).await,
    }
}

/// `GET /tag/:slug`
pub async fn tag_archive(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
    Query(query): Query<PageQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    match render_taxonomy_archive(state.clone(), slug, TaxonomyType::Tag, query.page, uri, site_id).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path).await,
    }
}

/// `GET /author/:username`
pub async fn author_archive(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(username): Path<String>,
    Query(query): Query<PageQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    match render_author_archive(state.clone(), username, query.page, uri, site_id).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path).await,
    }
}

async fn render_taxonomy_archive(
    state: AppState,
    slug: String,
    tax_type: TaxonomyType,
    page: i64,
    uri: axum::http::Uri,
    site_id: Uuid,
) -> crate::errors::Result<String> {
    let term = taxonomy::get_by_slug(&state.db, Some(site_id), &slug, tax_type.clone()).await?;
    let count = taxonomy::post_count(&state.db, term.id).await?;
    let term_ctx = TermContext::from_taxonomy(&term, &state.settings.base_url, count);

    let per_page = state.settings.posts_per_page;
    let offset = (page - 1) * per_page;

    let filter = match tax_type {
        TaxonomyType::Category => ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            category_slug: Some(slug.clone()),
            limit: per_page,
            offset,
            ..Default::default()
        },
        TaxonomyType::Tag => ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            tag_slug: Some(slug.clone()),
            limit: per_page,
            offset,
            ..Default::default()
        },
    };

    let posts_raw = post::list(&state.db, &filter).await?;
    let mut posts = Vec::with_capacity(posts_raw.len());
    for p in &posts_raw {
        posts.push(build_post_context(&state, p).await?);
    }

    let pagination =
        PaginationContext::new(page, per_page, count, &format!("{}{}", state.settings.base_url, uri.path()), "");

    let archive = ArchiveContext {
        archive_type: term.taxonomy.clone(),
        archive_term: Some(term_ctx),
        posts,
        pagination: Some(pagination),
    };

    let site_ctx = build_site_context(&state, Some(site_id)).await?;

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

    ctx.insert("archive_type", &archive.archive_type);
    ctx.insert("archive_term", &archive.archive_term);
    ctx.insert("posts", &archive.posts);
    ctx.insert("pagination", &archive.pagination);

    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("archive.html", &ctx)
}

async fn render_author_archive(
    state: AppState,
    username: String,
    page: i64,
    uri: axum::http::Uri,
    site_id: Uuid,
) -> crate::errors::Result<String> {
    use crate::models::user;

    let author = user::get_by_username(&state.db, &username).await?;

    let per_page = state.settings.posts_per_page;
    let offset = (page - 1) * per_page;

    let posts_raw = post::list(
        &state.db,
        &ListFilter {
            site_id: Some(site_id),
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            author_id: Some(author.id),
            limit: per_page,
            offset,
            ..Default::default()
        },
    )
    .await?;

    let total_posts = post::count(&state.db, Some(site_id), Some(PostStatus::Published), Some(PostType::Post)).await?;

    let mut posts = Vec::with_capacity(posts_raw.len());
    for p in &posts_raw {
        posts.push(build_post_context(&state, p).await?);
    }

    let pagination =
        PaginationContext::new(page, per_page, total_posts, &format!("{}{}", state.settings.base_url, uri.path()), "");

    let author_ctx = crate::models::user::UserContext::from_user(&author, &state.settings.base_url);
    let site_ctx = build_site_context(&state, Some(site_id)).await?;

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

    ctx.insert("archive_type", &"author");
    ctx.insert("archive_term", &Option::<TermContext>::None);
    ctx.insert("archive_author", &author_ctx);
    ctx.insert("posts", &posts);
    ctx.insert("pagination", &pagination);

    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("archive.html", &ctx)
}
