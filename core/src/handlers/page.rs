use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Response},
};

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
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
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
            query: std::collections::HashMap::new(),
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("page", &page_ctx);

    let theme = state.active_theme_for_site(Some(site_id));
    let hook_outputs = state.templates.render_hooks_for_theme(
        &theme,
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    crate::templates::context::ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render_for_theme(&theme, "page.html", &ctx)
}
