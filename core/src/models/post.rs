use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::{AppError, Result};
use crate::models::media::{Media, MediaContext};
use crate::models::taxonomy::{TermContext, TaxonomyType};
use crate::models::user::{User, UserContext};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostStatus {
    Draft,
    Pending,
    Published,
    Scheduled,
    Trashed,
}

impl PostStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostStatus::Draft => "draft",
            PostStatus::Pending => "pending",
            PostStatus::Published => "published",
            PostStatus::Scheduled => "scheduled",
            PostStatus::Trashed => "trashed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostType {
    Post,
    Page,
}

impl PostType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostType::Post => "post",
            PostType::Page => "page",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub site_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub content_format: String,
    pub excerpt: Option<String>,
    pub status: String,
    pub post_type: String,
    pub author_id: Uuid,
    pub featured_image_id: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub template: Option<String>,
    pub post_password: Option<String>,
    pub comments_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent_id: Option<Uuid>,
}

/// A single breadcrumb item for hierarchical page navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadcrumbItem {
    pub label: String,
    pub url: String,
    pub is_current: bool,
}

/// Full context object for a post, as exposed to Tera templates.
/// All IDs are strings (UUID), datetimes are ISO 8601 strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostContext {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: String,
    pub status: String,
    pub post_type: String,
    pub url: String,
    /// UUID string of the parent page, or null for top-level pages/posts.
    pub parent_id: Option<String>,
    /// Breadcrumb trail for hierarchical pages (Home → Parent → Current).
    pub breadcrumbs: Vec<BreadcrumbItem>,
    pub published_at: Option<String>,
    pub updated_at: String,
    pub author: UserContext,
    pub categories: Vec<TermContext>,
    pub tags: Vec<TermContext>,
    pub featured_image: Option<MediaContext>,
    pub reading_time: u32,
    pub comment_count: i64,
    /// Whether readers can post comments on this post.
    pub comments_enabled: bool,
    /// Plugin-registered custom fields, keyed by meta_key.
    pub meta: HashMap<String, String>,
}

impl PostContext {
    /// Build a PostContext from a Post and its related data.
    ///
    /// `page_path` overrides the URL for hierarchical pages (e.g. `/services/service-1`).
    /// For non-page post types, pass `None`.
    /// `breadcrumbs` is populated for pages with ancestors.
    pub fn build(
        post: &Post,
        author: &User,
        categories: Vec<TermContext>,
        tags: Vec<TermContext>,
        featured_image: Option<MediaContext>,
        meta: HashMap<String, String>,
        comment_count: i64,
        base_url: &str,
        page_path: Option<String>,
        breadcrumbs: Vec<BreadcrumbItem>,
    ) -> Self {
        let url = if let Some(ref path) = page_path {
            format!("{}{}", base_url, path)
        } else {
            match post.post_type.as_str() {
                "page" => format!("{}/{}", base_url, post.slug),
                _ => format!("{}/{}", base_url, post.slug),
            }
        };

        let excerpt = post.excerpt.clone().unwrap_or_else(|| {
            // Auto-generate: strip HTML, take first 55 words
            let text = strip_tags(&post.content);
            let words: Vec<&str> = text.split_whitespace().take(55).collect();
            if words.len() == 55 {
                format!("{} ...", words.join(" "))
            } else {
                words.join(" ")
            }
        });

        let reading_time = {
            let text = strip_tags(&post.content);
            let word_count = text.split_whitespace().count();
            ((word_count as f64 / 200.0).ceil() as u32).max(1)
        };

        PostContext {
            id: post.id.to_string(),
            title: post.title.clone(),
            slug: post.slug.clone(),
            content: post.content.clone(),
            excerpt,
            status: post.status.clone(),
            post_type: post.post_type.clone(),
            url,
            parent_id: post.parent_id.map(|id| id.to_string()),
            breadcrumbs,
            published_at: post.published_at.map(|dt| dt.to_rfc3339()),
            updated_at: post.updated_at.to_rfc3339(),
            author: UserContext::from_user(author, base_url),
            categories,
            tags,
            featured_image,
            reading_time,
            comment_count,
            comments_enabled: post.comments_enabled,
            meta,
        }
    }
}

/// Suppress unused import warnings — Media and TaxonomyType are part of the public API
/// surface even if not directly used in every function here.
const _: () = {
    let _ = std::mem::size_of::<Media>();
    let _ = std::mem::size_of::<TaxonomyType>();
};

/// Data required to create a new post.
#[derive(Debug, Deserialize)]
pub struct CreatePost {
    pub site_id: Option<Uuid>,
    pub title: String,
    pub slug: Option<String>,
    pub content: String,
    pub content_format: Option<String>,
    pub excerpt: Option<String>,
    pub status: PostStatus,
    pub post_type: PostType,
    pub author_id: Uuid,
    pub featured_image_id: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub template: Option<String>,
    /// Argon2 hash of the page/post password. None = no protection.
    pub post_password_hash: Option<String>,
    /// Whether readers can post comments on this post.
    pub comments_enabled: bool,
    /// UUID of the parent page. None for top-level pages/posts.
    pub parent_id: Option<Uuid>,
}

/// Data for updating an existing post.
#[derive(Debug, Deserialize)]
pub struct UpdatePost {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub content: Option<String>,
    pub content_format: Option<String>,
    pub excerpt: Option<String>,
    pub status: Option<PostStatus>,
    pub featured_image_id: Option<Uuid>,
    /// When true, set featured_image_id to NULL regardless of featured_image_id value.
    pub clear_featured_image: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub template: Option<String>,
    /// When true, remove password protection entirely.
    pub clear_post_password: bool,
    /// New Argon2 hash to set. None = leave existing password unchanged.
    pub new_post_password_hash: Option<String>,
    /// None = leave unchanged; Some(v) = update comments_enabled.
    pub comments_enabled: Option<bool>,
    /// None = leave unchanged; Some(id) = set parent; Some(None) = clear parent.
    pub parent_id: Option<Option<Uuid>>,
}

/// Strip all HTML tags, returning plain text content.
/// Used internally for word counting and excerpt generation.
fn strip_tags(html: &str) -> String {
    ammonia::Builder::new()
        .tags(Default::default())
        .clean(html)
        .to_string()
}

/// Sanitize HTML content before storage.
/// Uses ammonia to strip disallowed tags/attributes while preserving safe HTML.
/// This is the contract that allows theme templates to use `{{ post.content | safe }}`.
pub fn sanitize_content(html: &str) -> String {
    // ammonia::clean() uses a safe allowlist of tags/attributes.
    // This is intentionally strict for user-submitted content.
    // The Phase 3 admin editor (rich text) should produce clean HTML;
    // sanitization here is the last line of defence.
    ammonia::clean(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::models::user::User;

    fn make_user() -> User {
        User {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: "Test User".to_string(),
            password_hash: "hash".to_string(),
            bio: "".to_string(),
            avatar_media_id: None,
            role: "author".to_string(),
            is_active: true,
            is_protected: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
            default_site_id: None,
        }
    }

    fn make_post(post_type: &str, slug: &str, content: &str, excerpt: Option<String>) -> Post {
        Post {
            id: Uuid::new_v4(),
            site_id: None,
            title: "Test Post".to_string(),
            slug: slug.to_string(),
            content: content.to_string(),
            content_format: "html".to_string(),
            excerpt,
            status: "published".to_string(),
            post_type: post_type.to_string(),
            author_id: Uuid::new_v4(),
            featured_image_id: None,
            published_at: Some(Utc::now()),
            scheduled_at: None,
            submitted_at: None,
            template: None,
            post_password: None,
            comments_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_id: None,
        }
    }

    // --- PostStatus ---

    #[test]
    fn post_status_as_str_all_variants() {
        assert_eq!(PostStatus::Draft.as_str(), "draft");
        assert_eq!(PostStatus::Published.as_str(), "published");
        assert_eq!(PostStatus::Scheduled.as_str(), "scheduled");
        assert_eq!(PostStatus::Trashed.as_str(), "trashed");
    }

    // --- PostType ---

    #[test]
    fn post_type_as_str_both_variants() {
        assert_eq!(PostType::Post.as_str(), "post");
        assert_eq!(PostType::Page.as_str(), "page");
    }

    // --- sanitize_content ---

    #[test]
    fn sanitize_content_strips_script_tag() {
        let html = r#"<p>Hello</p><script>alert("xss")</script>"#;
        let result = sanitize_content(html);
        assert!(!result.contains("<script>"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<p>Hello</p>"));
    }

    #[test]
    fn sanitize_content_strips_iframe_tag() {
        let html = r#"<p>Text</p><iframe src="evil.com"></iframe>"#;
        let result = sanitize_content(html);
        assert!(!result.contains("iframe"));
        assert!(result.contains("Text"));
    }

    #[test]
    fn sanitize_content_strips_onclick_attribute() {
        let html = r#"<a href="/foo" onclick="evil()">Link</a>"#;
        let result = sanitize_content(html);
        assert!(!result.contains("onclick"));
        assert!(result.contains("Link"));
    }

    #[test]
    fn sanitize_content_preserves_safe_tags() {
        let html = "<p>Hello <strong>world</strong> and <a href='/x'>link</a></p>";
        let result = sanitize_content(html);
        assert!(result.contains("<p>"));
        assert!(result.contains("<strong>"));
    }

    #[test]
    fn sanitize_content_empty_string() {
        assert_eq!(sanitize_content(""), "");
    }

    #[test]
    fn sanitize_content_plain_text_passthrough() {
        let text = "Just plain text, no HTML.";
        assert_eq!(sanitize_content(text), text);
    }

    // --- PostContext::build ---

    #[test]
    fn post_context_url_for_post_type() {
        let user = make_user();
        let post = make_post("post", "my-post", "content", None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.url, "https://example.com/my-post");
    }

    #[test]
    fn post_context_url_for_page_type() {
        let user = make_user();
        let post = make_post("page", "about", "content", None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.url, "https://example.com/about");
    }

    #[test]
    fn post_context_excerpt_passthrough_when_provided() {
        let user = make_user();
        let post = make_post("post", "slug", "Some content.", Some("Custom excerpt.".to_string()));
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.excerpt, "Custom excerpt.");
    }

    #[test]
    fn post_context_excerpt_auto_truncates_at_55_words() {
        let user = make_user();
        let content = "word ".repeat(100);
        let post = make_post("post", "slug", &content, None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert!(ctx.excerpt.ends_with(" ..."), "excerpt should end with ' ...'");
        let word_count = ctx.excerpt.trim_end_matches(" ...").split_whitespace().count();
        assert_eq!(word_count, 55);
    }

    #[test]
    fn post_context_excerpt_short_content_no_ellipsis() {
        let user = make_user();
        let post = make_post("post", "slug", "short content here", None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert!(!ctx.excerpt.ends_with(" ..."));
    }

    #[test]
    fn post_context_reading_time_200_words_is_1_min() {
        let user = make_user();
        let content = "word ".repeat(200);
        let post = make_post("post", "slug", &content, None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.reading_time, 1);
    }

    #[test]
    fn post_context_reading_time_400_words_is_2_min() {
        let user = make_user();
        let content = "word ".repeat(400);
        let post = make_post("post", "slug", &content, None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.reading_time, 2);
    }

    #[test]
    fn post_context_reading_time_empty_content_is_1_min() {
        let user = make_user();
        let post = make_post("post", "slug", "", None);
        let ctx = PostContext::build(
            &post, &user, vec![], vec![], None, HashMap::new(), 0, "https://example.com", None, vec![],
        );
        assert_eq!(ctx.reading_time, 1);
    }
}

pub async fn create(pool: &PgPool, data: &CreatePost) -> Result<Post> {
    let slug = data.slug.clone().unwrap_or_else(|| crate::utils::slugify::slugify(&data.title));
    let slug = slug.chars().take(200).collect::<String>();
    let format = data.content_format.as_deref().unwrap_or("html");
    let sanitized_content = sanitize_content(&data.content);
    let clean_title   = data.title.chars().take(255).collect::<String>();
    let clean_excerpt = data.excerpt.as_deref()
        .map(|e| e.chars().take(500).collect::<String>());

    let post = sqlx::query_as::<_, Post>(
        r#"
        INSERT INTO posts (site_id, title, slug, content, content_format, excerpt, status,
                           post_type, author_id, featured_image_id, published_at, template,
                           post_password, comments_enabled, submitted_at, parent_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                CASE WHEN $7 = 'pending' THEN NOW() ELSE NULL END, $15)
        RETURNING *
        "#,
    )
    .bind(data.site_id)
    .bind(&clean_title)
    .bind(&slug)
    .bind(&sanitized_content)
    .bind(format)
    .bind(&clean_excerpt)
    .bind(data.status.as_str())
    .bind(data.post_type.as_str())
    .bind(data.author_id)
    .bind(data.featured_image_id)
    .bind(data.published_at)
    .bind(data.template.as_deref())
    .bind(data.post_password_hash.as_deref())
    .bind(data.comments_enabled)
    .bind(data.parent_id)
    .fetch_one(pool)
    .await?;

    Ok(post)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Post> {
    sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("post {id}")))
}

#[allow(dead_code)]
pub async fn get_by_slug(pool: &PgPool, site_id: Option<Uuid>, slug: &str) -> Result<Post> {
    sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE slug = $1 AND ($2::uuid IS NULL OR site_id = $2)",
    )
    .bind(slug)
    .bind(site_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("post '{slug}'")))
}

pub async fn get_published_by_slug(pool: &PgPool, site_id: Option<Uuid>, slug: &str) -> Result<Post> {
    sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE slug = $1 AND status = 'published' \
         AND ($2::uuid IS NULL OR site_id = $2)",
    )
    .bind(slug)
    .bind(site_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("post '{slug}'")))
}

pub struct ListFilter {
    pub site_id: Option<Uuid>,
    pub status: Option<PostStatus>,
    pub post_type: Option<PostType>,
    pub author_id: Option<Uuid>,
    pub category_slug: Option<String>,
    pub tag_slug: Option<String>,
    /// Optional free-text filter applied to post titles (admin list only).
    /// Stop words are stripped before building ILIKE clauses.
    pub search: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListFilter {
    fn default() -> Self {
        ListFilter {
            site_id: None,
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            author_id: None,
            category_slug: None,
            tag_slug: None,
            search: None,
            limit: 10,
            offset: 0,
        }
    }
}

/// Common English stop words — mirrors the list in `search/index.rs` and `models/comment.rs`.
/// Stripped from admin search input before building ILIKE clauses.
static POST_STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "up", "about", "into", "through", "is",
    "was", "are", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might",
    "shall", "can", "i", "me", "my", "we", "our", "you", "your", "he",
    "him", "his", "she", "her", "it", "its", "they", "them", "their",
    "this", "that", "these", "those", "what", "which", "who", "whom",
    "not", "no", "so", "if", "as", "than", "too", "very", "just", "also",
    "more", "most", "other", "some", "such", "only", "own", "same",
];

/// Split a search string into lowercase terms, stripping stop words.
/// Returns an empty Vec if all terms are stop words (→ no filter applied).
pub fn search_terms(input: &str) -> Vec<String> {
    input.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !POST_STOP_WORDS.contains(&w.as_str()))
        .collect()
}

pub async fn list(pool: &PgPool, filter: &ListFilter) -> Result<Vec<Post>> {
    let posts = if let Some(cat_slug) = &filter.category_slug {
        sqlx::query_as::<_, Post>(
            r#"
            SELECT p.*
            FROM posts p
            JOIN post_taxonomies pt ON pt.post_id = p.id
            JOIN taxonomies t ON t.id = pt.taxonomy_id
            WHERE t.slug = $1
              AND t.taxonomy = 'category'
              AND ($2::text IS NULL OR p.status = $2)
              AND ($3::text IS NULL OR p.post_type = $3)
              AND ($4::uuid IS NULL OR p.site_id = $4)
            ORDER BY p.published_at DESC NULLS LAST
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(cat_slug.as_str())
        .bind(filter.status.as_ref().map(|s| s.as_str()))
        .bind(filter.post_type.as_ref().map(|t| t.as_str()))
        .bind(filter.site_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?
    } else if let Some(tag_slug) = &filter.tag_slug {
        sqlx::query_as::<_, Post>(
            r#"
            SELECT p.*
            FROM posts p
            JOIN post_taxonomies pt ON pt.post_id = p.id
            JOIN taxonomies t ON t.id = pt.taxonomy_id
            WHERE t.slug = $1
              AND t.taxonomy = 'tag'
              AND ($2::text IS NULL OR p.status = $2)
              AND ($3::text IS NULL OR p.post_type = $3)
              AND ($4::uuid IS NULL OR p.site_id = $4)
            ORDER BY p.published_at DESC NULLS LAST
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tag_slug.as_str())
        .bind(filter.status.as_ref().map(|s| s.as_str()))
        .bind(filter.post_type.as_ref().map(|t| t.as_str()))
        .bind(filter.site_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?
    } else {
        // Build dynamic SQL so optional title search terms can be appended as
        // AND LOWER(title) LIKE $n clauses. Fixed params are $1–$4; search terms
        // start at $5; LIMIT/OFFSET come last.
        let terms = filter.search.as_deref().map(search_terms).unwrap_or_default();

        let mut sql = "SELECT * FROM posts \
                       WHERE ($1::text IS NULL OR status = $1) \
                         AND ($2::text IS NULL OR post_type = $2) \
                         AND ($3::uuid IS NULL OR author_id = $3) \
                         AND ($4::uuid IS NULL OR site_id = $4)"
            .to_string();

        for i in 0..terms.len() {
            let n = i + 5;
            sql.push_str(&format!(" AND LOWER(title) LIKE ${n}"));
        }

        let limit_n  = terms.len() + 5;
        let offset_n = terms.len() + 6;
        sql.push_str(&format!(
            " ORDER BY published_at DESC NULLS LAST LIMIT ${limit_n} OFFSET ${offset_n}"
        ));

        let mut q = sqlx::query_as::<_, Post>(&sql)
            .bind(filter.status.as_ref().map(|s| s.as_str()))
            .bind(filter.post_type.as_ref().map(|t| t.as_str()))
            .bind(filter.author_id)
            .bind(filter.site_id);
        for term in &terms {
            q = q.bind(format!("%{term}%"));
        }
        q.bind(filter.limit).bind(filter.offset).fetch_all(pool).await?
    };

    Ok(posts)
}

pub async fn count(
    pool: &PgPool,
    site_id: Option<Uuid>,
    status: Option<PostStatus>,
    post_type: Option<PostType>,
) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts
        WHERE ($1::uuid IS NULL OR site_id = $1)
          AND ($2::text IS NULL OR status = $2)
          AND ($3::text IS NULL OR post_type = $3)
        "#,
    )
    .bind(site_id)
    .bind(status.as_ref().map(|s| s.as_str()))
    .bind(post_type.as_ref().map(|t| t.as_str()))
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Get the post published immediately before this one (within the same site).
pub async fn get_prev(
    pool: &PgPool,
    site_id: Option<Uuid>,
    published_at: DateTime<Utc>,
) -> Result<Option<Post>> {
    Ok(sqlx::query_as::<_, Post>(
        r#"
        SELECT * FROM posts
        WHERE status = 'published'
          AND post_type = 'post'
          AND published_at < $1
          AND ($2::uuid IS NULL OR site_id = $2)
        ORDER BY published_at DESC
        LIMIT 1
        "#,
    )
    .bind(published_at)
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

/// Get the post published immediately after this one (within the same site).
pub async fn get_next(
    pool: &PgPool,
    site_id: Option<Uuid>,
    published_at: DateTime<Utc>,
) -> Result<Option<Post>> {
    Ok(sqlx::query_as::<_, Post>(
        r#"
        SELECT * FROM posts
        WHERE status = 'published'
          AND post_type = 'post'
          AND published_at > $1
          AND ($2::uuid IS NULL OR site_id = $2)
        ORDER BY published_at ASC
        LIMIT 1
        "#,
    )
    .bind(published_at)
    .bind(site_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn update(pool: &PgPool, id: Uuid, data: &UpdatePost) -> Result<Post> {
    // Fetch current record, apply updates, save.
    let current = get_by_id(pool, id).await?;

    let new_slug = data.slug.clone().unwrap_or(current.slug.clone());
    let new_slug = new_slug.chars().take(200).collect::<String>();
    let new_title = data.title.as_deref()
        .map(|t| t.chars().take(255).collect::<String>())
        .unwrap_or(current.title.clone());
    let new_content = match &data.content {
        Some(html) => sanitize_content(html),
        None => current.content.clone(),
    };
    let new_format = data.content_format.clone().unwrap_or(current.content_format.clone());
    let new_excerpt = data.excerpt.as_deref()
        .map(|e| e.chars().take(500).collect::<String>())
        .or(current.excerpt.clone());
    let new_status = data.status.as_ref().map(|s| s.as_str().to_string()).unwrap_or(current.status.clone());
    let new_image = if data.clear_featured_image {
        None
    } else if data.featured_image_id.is_some() {
        data.featured_image_id
    } else {
        current.featured_image_id
    };
    let new_published_at = if data.published_at.is_some() {
        data.published_at
    } else {
        current.published_at
    };

    let new_password: Option<Option<&str>> = if data.clear_post_password {
        Some(None) // explicitly NULL
    } else if let Some(ref hash) = data.new_post_password_hash {
        Some(Some(hash.as_str())) // new hash
    } else {
        None // leave unchanged
    };

    let new_comments_enabled = data.comments_enabled.unwrap_or(current.comments_enabled);

    // parent_id: None = leave unchanged; Some(None) = clear; Some(Some(id)) = set
    let new_parent_id: Option<Uuid> = match data.parent_id {
        Some(v) => v,
        None => current.parent_id,
    };

    let post = sqlx::query_as::<_, Post>(
        r#"
        UPDATE posts
        SET title = $1, slug = $2, content = $3, content_format = $4, excerpt = $5,
            status = $6, featured_image_id = $7, published_at = $8, template = $9,
            post_password = CASE WHEN $10 THEN NULL
                                 WHEN $11::text IS NOT NULL THEN $11
                                 ELSE post_password END,
            comments_enabled = $13,
            parent_id = $14,
            submitted_at = CASE WHEN $6 = 'pending' THEN COALESCE(submitted_at, NOW())
                                ELSE submitted_at END,
            updated_at = NOW()
        WHERE id = $12
        RETURNING *
        "#,
    )
    .bind(&new_title)
    .bind(&new_slug)
    .bind(&new_content)
    .bind(&new_format)
    .bind(&new_excerpt)
    .bind(&new_status)
    .bind(new_image)
    .bind(new_published_at)
    .bind(data.template.as_deref())
    .bind(data.clear_post_password)
    .bind(data.new_post_password_hash.as_deref())
    .bind(id)
    .bind(new_comments_enabled)
    .bind(new_parent_id)
    .fetch_one(pool)
    .await?;
    let _ = new_password; // silence unused warning

    Ok(post)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Fetch all custom fields (post_meta) for a post.
pub async fn get_meta(pool: &PgPool, post_id: Uuid) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT meta_key, meta_value FROM post_meta WHERE post_id = $1",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(k, v)| (k, v)).collect())
}

/// Upsert a custom field value.
#[allow(dead_code)]
pub async fn set_meta(pool: &PgPool, post_id: Uuid, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO post_meta (post_id, meta_key, meta_value)
        VALUES ($1, $2, $3)
        ON CONFLICT (post_id, meta_key)
        DO UPDATE SET meta_value = EXCLUDED.meta_value, updated_at = NOW()
        "#,
    )
    .bind(post_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch posts related by shared taxonomy terms (exclude the source post, same site).
pub async fn get_related(
    pool: &PgPool,
    site_id: Option<Uuid>,
    post_id: Uuid,
    limit: i64,
) -> Result<Vec<Post>> {
    let posts = sqlx::query_as::<_, Post>(
        r#"
        SELECT DISTINCT p.*
        FROM posts p
        JOIN post_taxonomies pt ON pt.post_id = p.id
        WHERE pt.taxonomy_id IN (
            SELECT taxonomy_id FROM post_taxonomies WHERE post_id = $1
        )
        AND p.id != $1
        AND p.status = 'published'
        AND ($3::uuid IS NULL OR p.site_id = $3)
        ORDER BY p.published_at DESC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(post_id)
    .bind(limit)
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(posts)
}

/// Fetch a published page by walking URL segments through the parent hierarchy.
///
/// `segments[0]` must be a root-level page (parent_id IS NULL).
/// Each subsequent segment is matched as a child of the previous page.
pub async fn get_page_by_path(
    pool: &PgPool,
    site_id: Option<Uuid>,
    segments: &[&str],
) -> Result<Post> {
    if segments.is_empty() {
        return Err(AppError::NotFound("page".to_string()));
    }

    // Find root page with no parent
    let mut current = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts \
         WHERE slug = $1 AND parent_id IS NULL \
           AND status = 'published' AND post_type = 'page' \
           AND ($2::uuid IS NULL OR site_id = $2)",
    )
    .bind(segments[0])
    .bind(site_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("page '{}'", segments[0])))?;

    for &seg in &segments[1..] {
        current = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts \
             WHERE slug = $1 AND parent_id = $2 \
               AND status = 'published' AND post_type = 'page' \
               AND ($3::uuid IS NULL OR site_id = $3)",
        )
        .bind(seg)
        .bind(current.id)
        .bind(site_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("page '{}'", seg)))?;
    }

    Ok(current)
}

/// Build the full URL path for a page by walking its parent chain.
/// Returns a string like `/services/service-1` (always starts with `/`).
pub async fn get_full_page_path(pool: &PgPool, page: &Post) -> String {
    let mut segments = vec![page.slug.clone()];
    let mut current_parent = page.parent_id;
    while let Some(pid) = current_parent {
        match get_by_id(pool, pid).await {
            Ok(parent) => {
                segments.push(parent.slug.clone());
                current_parent = parent.parent_id;
            }
            Err(_) => break,
        }
    }
    segments.reverse();
    format!("/{}", segments.join("/"))
}

/// Build breadcrumb trail for a hierarchical page.
/// Returns items from Home → ancestors → current page, each with label, url, is_current.
pub async fn get_page_breadcrumbs(
    pool: &PgPool,
    page: &Post,
    base_url: &str,
) -> Vec<BreadcrumbItem> {
    let mut breadcrumbs = vec![];

    // Collect ancestors (bottom-up), then reverse to get root-first order
    let mut ancestors: Vec<Post> = vec![];
    let mut current_parent = page.parent_id;
    while let Some(pid) = current_parent {
        match get_by_id(pool, pid).await {
            Ok(parent) => {
                current_parent = parent.parent_id;
                ancestors.push(parent);
            }
            Err(_) => break,
        }
    }
    ancestors.reverse();

    // Home always first
    breadcrumbs.push(BreadcrumbItem {
        label: "Home".to_string(),
        url: base_url.to_string(),
        is_current: false,
    });

    // Ancestor pages
    let mut path_segments: Vec<String> = vec![];
    for ancestor in &ancestors {
        path_segments.push(ancestor.slug.clone());
        breadcrumbs.push(BreadcrumbItem {
            label: ancestor.title.clone(),
            url: format!("{}/{}", base_url, path_segments.join("/")),
            is_current: false,
        });
    }

    // Current page
    path_segments.push(page.slug.clone());
    breadcrumbs.push(BreadcrumbItem {
        label: page.title.clone(),
        url: format!("{}/{}", base_url, path_segments.join("/")),
        is_current: true,
    });

    breadcrumbs
}

/// Return all published pages for a site, ordered by title.
/// Used by the admin page editor to populate the parent selector dropdown.
pub async fn get_published_pages_by_site(
    pool: &PgPool,
    site_id: Option<Uuid>,
) -> Result<Vec<Post>> {
    let pages = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts \
         WHERE post_type = 'page' AND status = 'published' \
           AND ($1::uuid IS NULL OR site_id = $1) \
         ORDER BY title",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(pages)
}
