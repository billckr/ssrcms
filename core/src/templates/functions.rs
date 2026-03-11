//! Core Tera functions registered by the template engine.
//! Documented in docs/plugin-api-v1.md §6.

use std::collections::HashMap;
use std::sync::Arc;
use tera::{Function, Result, Value};

use crate::plugins::HookRegistry;

// ── hook() ───────────────────────────────────────────────────────────────────

/// `{{ hook(name="hook_name") }}`
/// Returns a sentinel string that is resolved to pre-rendered HTML after the main
/// template render completes. See TemplateEngine::resolve_hook_sentinels().
pub struct HookFunction {
    pub registry: Arc<HookRegistry>,
}

impl Function for HookFunction {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        let hook_name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("hook() requires a 'name' argument"))?;

        let handlers = self.registry.handlers_for(hook_name);
        if handlers.is_empty() {
            return Ok(Value::String(String::new()));
        }

        // Return a sentinel that the post-render pass replaces with pre-rendered HTML.
        // Format: [[HOOK:__hook_output__<hook_name>]]
        // The context key "__hook_output__<hook_name>" is populated by TemplateEngine::render_hooks()
        // and injected via ContextBuilder::add_hook_outputs() before the main render.
        Ok(Value::String(format!("[[HOOK:__hook_output__{}]]", hook_name)))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

// ── url_for() ────────────────────────────────────────────────────────────────

/// `{{ url_for(type="post", slug="hello-world") }}`
/// Generates a canonical URL for a named resource.
pub struct UrlForFunction {
    pub base_url: String,
}

impl Function for UrlForFunction {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        let resource_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("url_for() requires a 'type' argument"))?;

        let slug = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("url_for() requires a 'slug' argument"))?;

        let url = match resource_type {
            "post" => format!("{}/blog/{}", self.base_url, slug),
            "page" => format!("{}/{}", self.base_url, slug),
            "category" => format!("{}/category/{}", self.base_url, slug),
            "tag" => format!("{}/tag/{}", self.base_url, slug),
            "author" => format!("{}/author/{}", self.base_url, slug),
            other => {
                return Err(tera::Error::msg(format!(
                    "url_for(): unknown resource type '{other}'"
                )))
            }
        };

        Ok(Value::String(url))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

// ── get_posts() ──────────────────────────────────────────────────────────────

/// `{{ get_posts(limit=5, category="rust") }}`
/// Fetches published posts with optional filters.
/// Runs a blocking DB query on the current Tokio runtime.
pub struct GetPostsFunction {
    pub pool: sqlx::PgPool,
    pub base_url: String,
}

impl Function for GetPostsFunction {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10)
            .min(100); // safety cap
        let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0);
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let tag = args
            .get("tag")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let author = args
            .get("author")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let order = args
            .get("order")
            .and_then(|v| v.as_str())
            .unwrap_or("desc")
            .to_string();

        let pool = self.pool.clone();
        let base_url = self.base_url.clone();

        // Tera functions are synchronous; run the async query on the current Tokio runtime.
        let posts = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                fetch_posts_for_function(&pool, &base_url, limit, offset, category, tag, author, &order).await
            })
        })
        .map_err(|e| tera::Error::msg(format!("get_posts() error: {e}")))?;

        serde_json::to_value(posts)
            .map_err(|e| tera::Error::msg(format!("get_posts() serialization error: {e}")))
    }

    fn is_safe(&self) -> bool {
        false
    }
}

async fn fetch_posts_for_function(
    pool: &sqlx::PgPool,
    base_url: &str,
    limit: i64,
    offset: i64,
    category: Option<String>,
    tag: Option<String>,
    author: Option<String>,
    order: &str,
) -> anyhow::Result<Vec<crate::models::post::PostContext>> {
    use crate::models::{media, post, taxonomy, user};

    let order_clause = if order == "asc" { "ASC" } else { "DESC" };

    let posts_raw: Vec<post::Post> = if let Some(cat_slug) = &category {
        sqlx::query_as::<_, post::Post>(&format!(
            r#"SELECT p.* FROM posts p
               JOIN post_taxonomies pt ON pt.post_id = p.id
               JOIN taxonomies t ON t.id = pt.taxonomy_id
               WHERE t.slug = $1 AND t.taxonomy = 'category' AND p.status = 'published'
               ORDER BY p.published_at {order_clause} NULLS LAST
               LIMIT $2 OFFSET $3"#
        ))
        .bind(cat_slug)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else if let Some(tag_slug) = &tag {
        sqlx::query_as::<_, post::Post>(&format!(
            r#"SELECT p.* FROM posts p
               JOIN post_taxonomies pt ON pt.post_id = p.id
               JOIN taxonomies t ON t.id = pt.taxonomy_id
               WHERE t.slug = $1 AND t.taxonomy = 'tag' AND p.status = 'published'
               ORDER BY p.published_at {order_clause} NULLS LAST
               LIMIT $2 OFFSET $3"#
        ))
        .bind(tag_slug)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else if let Some(username) = &author {
        let author_row = sqlx::query_as::<_, user::User>(
            "SELECT * FROM users WHERE username = $1 AND is_active = TRUE",
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;

        match author_row {
            None => Vec::new(),
            Some(u) => {
                sqlx::query_as::<_, post::Post>(&format!(
                    r#"SELECT * FROM posts
                       WHERE status = 'published' AND post_type = 'post' AND author_id = $1
                       ORDER BY published_at {order_clause} NULLS LAST
                       LIMIT $2 OFFSET $3"#
                ))
                .bind(u.id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
            }
        }
    } else {
        sqlx::query_as::<_, post::Post>(&format!(
            r#"SELECT * FROM posts
               WHERE status = 'published' AND post_type = 'post'
               ORDER BY published_at {order_clause} NULLS LAST
               LIMIT $1 OFFSET $2"#
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::with_capacity(posts_raw.len());
    for p in &posts_raw {
        let author_rec = user::get_by_id(pool, p.author_id).await?;
        let all_terms = taxonomy::for_post(pool, p.id).await?;

        let categories = all_terms
            .iter()
            .filter(|t| t.taxonomy == "category")
            .map(|t| taxonomy::TermContext::from_taxonomy(t, base_url, 0))
            .collect();
        let tags = all_terms
            .iter()
            .filter(|t| t.taxonomy == "tag")
            .map(|t| taxonomy::TermContext::from_taxonomy(t, base_url, 0))
            .collect();

        let featured_image = if let Some(img_id) = p.featured_image_id {
            media::get_by_id(pool, img_id)
                .await
                .ok()
                .map(|m| media::MediaContext::from_media(&m, base_url))
        } else {
            None
        };

        let meta = post::get_meta(pool, p.id).await.unwrap_or_default();

        // For pages, resolve the full hierarchical path.
        let page_path = if p.post_type == "page" {
            Some(post::get_full_page_path(pool, p).await)
        } else {
            None
        };

        result.push(post::PostContext::build(
            p,
            &author_rec,
            categories,
            tags,
            featured_image,
            meta,
            0,
            base_url,
            page_path,
            vec![],
        ));
    }

    Ok(result)
}

// ── get_menu() ───────────────────────────────────────────────────────────────

/// `{% set items = get_menu(name="salemenu") %}`
/// Fetches a menu by name and returns its items as a tree (`Vec<NavItemContext>`).
/// Returns an empty array if no menu with that name exists.
/// The `request_path` argument is optional; when supplied, `is_current` is set
/// on the matching item.  Example:
///   `{% set items = get_menu(name="salemenu", path=request.path) %}`
pub struct GetMenuFunction {
    pub pool: sqlx::PgPool,
}

impl Function for GetMenuFunction {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("get_menu() requires a 'name' argument"))?
            .to_string();

        let request_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pool = self.pool.clone();

        let items = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                fetch_menu_by_name(&pool, &name, &request_path).await
            })
        })
        .map_err(|e| tera::Error::msg(format!("get_menu() error: {e}")))?;

        serde_json::to_value(items)
            .map_err(|e| tera::Error::msg(format!("get_menu() serialization error: {e}")))
    }

    fn is_safe(&self) -> bool {
        false
    }
}

async fn fetch_menu_by_name(
    pool: &sqlx::PgPool,
    name: &str,
    request_path: &str,
) -> anyhow::Result<Vec<crate::templates::context::NavItemContext>> {
    use crate::models::nav_menu;

    // Find menu by name (any site — same limitation as get_posts)
    let menu = sqlx::query_as::<_, nav_menu::NavMenu>(
        "SELECT * FROM nav_menus WHERE name = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;

    let Some(menu) = menu else {
        return Ok(vec![]);
    };

    let items = nav_menu::items_for_menu(pool, menu.id).await?;

    // Resolve page URLs for any page_id references
    let page_ids: Vec<uuid::Uuid> = items
        .iter()
        .filter_map(|i| i.page_id)
        .collect();

    let mut page_urls: std::collections::HashMap<uuid::Uuid, String> =
        std::collections::HashMap::new();
    for pid in page_ids {
        if let Ok(post) = sqlx::query_as::<_, crate::models::post::Post>(
            "SELECT * FROM posts WHERE id = $1",
        )
        .bind(pid)
        .fetch_one(pool)
        .await
        {
            let path = crate::models::post::get_full_page_path(pool, &post).await;
            page_urls.insert(pid, path);
        }
    }

    Ok(nav_menu::build_tree(&items, &page_urls, None, request_path))
}

// ── get_terms() ──────────────────────────────────────────────────────────────

/// `{{ get_terms(taxonomy="category") }}`
/// Fetches all terms for a given taxonomy type, with post counts.
pub struct GetTermsFunction {
    pub pool: sqlx::PgPool,
    pub base_url: String,
}

impl Function for GetTermsFunction {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        let taxonomy = args
            .get("taxonomy")
            .and_then(|v| v.as_str())
            .unwrap_or("category")
            .to_string();

        if taxonomy != "category" && taxonomy != "tag" {
            return Err(tera::Error::msg(format!(
                "get_terms(): taxonomy must be 'category' or 'tag', got '{taxonomy}'"
            )));
        }

        let pool = self.pool.clone();
        let base_url = self.base_url.clone();

        let terms = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                fetch_terms_for_function(&pool, &base_url, &taxonomy).await
            })
        })
        .map_err(|e| tera::Error::msg(format!("get_terms() error: {e}")))?;

        serde_json::to_value(terms)
            .map_err(|e| tera::Error::msg(format!("get_terms() serialization error: {e}")))
    }

    fn is_safe(&self) -> bool {
        false
    }
}

async fn fetch_terms_for_function(
    pool: &sqlx::PgPool,
    base_url: &str,
    taxonomy: &str,
) -> anyhow::Result<Vec<crate::models::taxonomy::TermContext>> {
    use crate::models::taxonomy;

    let tax_type = if taxonomy == "tag" {
        taxonomy::TaxonomyType::Tag
    } else {
        taxonomy::TaxonomyType::Category
    };

    let terms = taxonomy::list(pool, None, tax_type).await?;
    let mut result = Vec::with_capacity(terms.len());
    for t in &terms {
        let count = taxonomy::post_count(pool, t.id).await.unwrap_or(0);
        result.push(taxonomy::TermContext::from_taxonomy(t, base_url, count));
    }
    Ok(result)
}
