use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::templates::context::{ContextBuilder, NavContext, RequestContext, SessionContext};

use super::home::{build_post_context, build_site_context, render_error_page};

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// `GET /search?q=...`
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    match render_search(state.clone(), params.q, uri).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path).await,
    }
}

async fn render_search(
    state: AppState,
    query: String,
    uri: axum::http::Uri,
) -> crate::errors::Result<String> {
    // Query Tantivy for matching post IDs, then fetch full records from DB.
    let search_results = if query.is_empty() {
        Vec::new()
    } else {
        state.search_index.search(&query, 20)?
    };

    // Fetch full Post records from DB by ID so we can build PostContext.
    let mut results = Vec::with_capacity(search_results.len());
    for hit in &search_results {
        if let Ok(id) = hit.id.parse::<Uuid>() {
            if let Ok(post) = crate::models::post::get_by_id(&state.db, id).await {
                if post.status == "published" {
                    if let Ok(ctx) = build_post_context(&state, &post).await {
                        results.push(ctx);
                    }
                }
            }
        }
    }

    let result_count = results.len() as i64;
    let site_ctx = build_site_context(&state).await?;

    let query_params: HashMap<String, String> = uri
        .query()
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            Some((k, v))
        })
        .collect();

    let mut ctx = ContextBuilder {
        site: site_ctx,
        request: RequestContext {
            url: format!("{}{}", state.settings.base_url, uri.path()),
            path: uri.path().to_string(),
            query: query_params,
        },
        session: SessionContext { is_logged_in: false, user: None },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("query", &query);
    ctx.insert("results", &results);
    ctx.insert("result_count", &result_count);

    let hook_outputs = state.templates.render_hooks(
        &["head_start", "head_end", "body_start", "body_end", "before_content", "after_content", "footer"],
        &ctx,
    );
    ContextBuilder::add_hook_outputs(&mut ctx, &hook_outputs);

    state.templates.render("search.html", &ctx)
}
