//! Context builder: constructs the Tera context for each route type.
//! Every variable in the context must match the API surface documented in docs/plugin-api-v1.md.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::post::PostContext;
use crate::models::taxonomy::TermContext;

/// The global site context available in every template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteContext {
    pub name: String,
    pub description: String,
    pub url: String,
    pub language: String,
    pub theme: String,
    pub post_count: i64,
    pub page_count: i64,
}

/// The request context available in every template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub url: String,
    pub path: String,
    pub query: HashMap<String, String>,
}

/// Session context: what is exposed about the logged-in user (if any).
/// NEVER includes password_hash or any secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub is_logged_in: bool,
    pub user: Option<SessionUserContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUserContext {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

/// Navigation menu context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NavContext {
    pub primary: NavMenuContext,
    pub footer: NavMenuContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NavMenuContext {
    pub items: Vec<NavItemContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItemContext {
    pub label: String,
    pub url: String,
    pub target: String,
    pub is_current: bool,
    pub children: Vec<NavItemContext>,
}

/// Pagination context for archive/home pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationContext {
    pub current_page: i64,
    pub total_pages: i64,
    pub per_page: i64,
    pub total_items: i64,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
}

impl PaginationContext {
    pub fn new(
        current_page: i64,
        per_page: i64,
        total_items: i64,
        base_url: &str,
        extra_params: &str,
    ) -> Self {
        let total_pages = ((total_items as f64) / (per_page as f64)).ceil() as i64;
        let total_pages = total_pages.max(1);

        let prev_url = if current_page > 1 {
            Some(format!(
                "{}?page={}{}",
                base_url,
                current_page - 1,
                extra_params
            ))
        } else {
            None
        };

        let next_url = if current_page < total_pages {
            Some(format!(
                "{}?page={}{}",
                base_url,
                current_page + 1,
                extra_params
            ))
        } else {
            None
        };

        PaginationContext {
            current_page,
            total_pages,
            per_page,
            total_items,
            prev_url,
            next_url,
        }
    }
}

/// Archive context additional data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveContext {
    pub archive_type: String,
    pub archive_term: Option<TermContext>,
    pub posts: Vec<PostContext>,
    pub pagination: Option<PaginationContext>,
}

/// Search results context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SearchContext {
    pub query: String,
    pub results: Vec<PostContext>,
    pub result_count: i64,
}

/// The full Tera context, built per-request and serialized to serde_json::Value.
pub struct ContextBuilder {
    pub site: SiteContext,
    pub request: RequestContext,
    pub session: SessionContext,
    pub nav: NavContext,
}

impl ContextBuilder {
    /// Convert the builder into a tera::Context.
    pub fn into_tera_context(self) -> tera::Context {
        let mut ctx = tera::Context::new();
        ctx.insert("site", &self.site);
        ctx.insert("request", &self.request);
        ctx.insert("session", &self.session);
        ctx.insert("nav", &self.nav);
        ctx
    }

    /// Add pre-rendered hook outputs to the context.
    /// Hook output keys follow the pattern "__hook_output__<hook_name>".
    pub fn add_hook_outputs(ctx: &mut tera::Context, hook_outputs: &HashMap<String, String>) {
        for (hook_name, html) in hook_outputs {
            ctx.insert(format!("__hook_output__{}", hook_name), html);
        }
    }
}
