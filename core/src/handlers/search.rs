use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
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
    current_site: CurrentSite,
    Query(params): Query<SearchQuery>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let path = uri.path().to_string();
    let site_id = current_site.site.id;
    let base_url = current_site.base_url.clone();
    match render_search(state.clone(), params.q, uri, site_id, &base_url).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => render_error_page(e, &state, &path, Some(current_site.site.id)).await,
    }
}

async fn render_search(
    state: AppState,
    query: String,
    uri: axum::http::Uri,
    site_id: Uuid,
    base_url: &str,
) -> crate::errors::Result<String> {
    // Enforce 25-character query limit server-side (mirrors maxlength on the HTML input).
    let query = if query.chars().count() > 25 {
        query.chars().take(25).collect::<String>()
    } else {
        query
    };

    let site_id_str = site_id.to_string();
    // Query Tantivy for matching post IDs, then fetch full records from DB.
    let search_results = if query.is_empty() {
        Vec::new()
    } else {
        metrics::counter!("synaptic_search_queries_total").increment(1);
        state.search_index.search(&query, Some(&site_id_str), 20)?
    };

    // Fetch full Post records from DB by ID so we can build PostContext.
    let mut results = Vec::with_capacity(search_results.len());
    for hit in &search_results {
        if let Ok(id) = hit.id.parse::<Uuid>() {
            if let Ok(post) = crate::models::post::get_by_id(&state.db, id).await {
                if post.status == "published" {
                    if let Ok(ctx) = build_post_context(&state, &post, base_url).await {
                        results.push(ctx);
                    }
                }
            }
        }
    }

    let result_count = results.len() as i64;
    let site_ctx = build_site_context(&state, Some(site_id), base_url).await?;

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
            url: format!("{}{}", base_url, uri.path()),
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

    state.templates.render_for_theme(&theme, Some(site_id), "search.html", &ctx)
}
