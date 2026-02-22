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
    Published,
    Scheduled,
    Trashed,
}

impl PostStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostStatus::Draft => "draft",
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub published_at: Option<String>,
    pub updated_at: String,
    pub author: UserContext,
    pub categories: Vec<TermContext>,
    pub tags: Vec<TermContext>,
    pub featured_image: Option<MediaContext>,
    pub reading_time: u32,
    pub comment_count: i64,
    /// Plugin-registered custom fields, keyed by meta_key.
    pub meta: HashMap<String, String>,
}

impl PostContext {
    /// Build a PostContext from a Post and its related data.
    pub fn build(
        post: &Post,
        author: &User,
        categories: Vec<TermContext>,
        tags: Vec<TermContext>,
        featured_image: Option<MediaContext>,
        meta: HashMap<String, String>,
        comment_count: i64,
        base_url: &str,
    ) -> Self {
        let url = match post.post_type.as_str() {
            "page" => format!("{}/{}", base_url, post.slug),
            _ => format!("{}/blog/{}", base_url, post.slug),
        };

        let excerpt = post.excerpt.clone().unwrap_or_else(|| {
            // Auto-generate: strip HTML, take first 55 words
            let text = ammonia::clean_text(&post.content);
            let words: Vec<&str> = text.split_whitespace().take(55).collect();
            if words.len() == 55 {
                format!("{} ...", words.join(" "))
            } else {
                words.join(" ")
            }
        });

        let reading_time = {
            let text = ammonia::clean_text(&post.content);
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
            published_at: post.published_at.map(|dt| dt.to_rfc3339()),
            updated_at: post.updated_at.to_rfc3339(),
            author: UserContext::from_user(author, base_url),
            categories,
            tags,
            featured_image,
            reading_time,
            comment_count,
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
    pub published_at: Option<DateTime<Utc>>,
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

pub async fn create(pool: &PgPool, data: &CreatePost) -> Result<Post> {
    let slug = data.slug.clone().unwrap_or_else(|| slug::slugify(&data.title));
    let format = data.content_format.as_deref().unwrap_or("html");
    let sanitized_content = sanitize_content(&data.content);

    let post = sqlx::query_as::<_, Post>(
        r#"
        INSERT INTO posts (title, slug, content, content_format, excerpt, status, post_type,
                           author_id, featured_image_id, published_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#,
    )
    .bind(&data.title)
    .bind(&slug)
    .bind(&sanitized_content)
    .bind(format)
    .bind(&data.excerpt)
    .bind(data.status.as_str())
    .bind(data.post_type.as_str())
    .bind(data.author_id)
    .bind(data.featured_image_id)
    .bind(data.published_at)
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
pub async fn get_by_slug(pool: &PgPool, slug: &str) -> Result<Post> {
    sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("post '{slug}'")))
}

pub async fn get_published_by_slug(pool: &PgPool, slug: &str) -> Result<Post> {
    sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("post '{slug}'")))
}

pub struct ListFilter {
    pub status: Option<PostStatus>,
    pub post_type: Option<PostType>,
    pub author_id: Option<Uuid>,
    pub category_slug: Option<String>,
    pub tag_slug: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListFilter {
    fn default() -> Self {
        ListFilter {
            status: Some(PostStatus::Published),
            post_type: Some(PostType::Post),
            author_id: None,
            category_slug: None,
            tag_slug: None,
            limit: 10,
            offset: 0,
        }
    }
}

pub async fn list(pool: &PgPool, filter: &ListFilter) -> Result<Vec<Post>> {
    // Build query dynamically based on filters. We use a simple approach since
    // SQLx compile-time checking doesn't handle fully dynamic queries.
    // For taxonomy filtering, we join through post_taxonomies.
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
            ORDER BY p.published_at DESC NULLS LAST
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(cat_slug.as_str())
        .bind(filter.status.as_ref().map(|s| s.as_str()))
        .bind(filter.post_type.as_ref().map(|t| t.as_str()))
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
            ORDER BY p.published_at DESC NULLS LAST
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tag_slug.as_str())
        .bind(filter.status.as_ref().map(|s| s.as_str()))
        .bind(filter.post_type.as_ref().map(|t| t.as_str()))
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Post>(
            r#"
            SELECT *
            FROM posts
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::text IS NULL OR post_type = $2)
              AND ($3::uuid IS NULL OR author_id = $3)
            ORDER BY published_at DESC NULLS LAST
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(filter.status.as_ref().map(|s| s.as_str()))
        .bind(filter.post_type.as_ref().map(|t| t.as_str()))
        .bind(filter.author_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?
    };

    Ok(posts)
}

pub async fn count(pool: &PgPool, status: Option<PostStatus>, post_type: Option<PostType>) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts
        WHERE ($1::text IS NULL OR status = $1)
          AND ($2::text IS NULL OR post_type = $2)
        "#,
    )
    .bind(status.as_ref().map(|s| s.as_str()))
    .bind(post_type.as_ref().map(|t| t.as_str()))
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Get the post published immediately before this one.
pub async fn get_prev(pool: &PgPool, published_at: DateTime<Utc>) -> Result<Option<Post>> {
    Ok(sqlx::query_as::<_, Post>(
        r#"
        SELECT * FROM posts
        WHERE status = 'published'
          AND post_type = 'post'
          AND published_at < $1
        ORDER BY published_at DESC
        LIMIT 1
        "#,
    )
    .bind(published_at)
    .fetch_optional(pool)
    .await?)
}

/// Get the post published immediately after this one.
pub async fn get_next(pool: &PgPool, published_at: DateTime<Utc>) -> Result<Option<Post>> {
    Ok(sqlx::query_as::<_, Post>(
        r#"
        SELECT * FROM posts
        WHERE status = 'published'
          AND post_type = 'post'
          AND published_at > $1
        ORDER BY published_at ASC
        LIMIT 1
        "#,
    )
    .bind(published_at)
    .fetch_optional(pool)
    .await?)
}

pub async fn update(pool: &PgPool, id: Uuid, data: &UpdatePost) -> Result<Post> {
    // Fetch current record, apply updates, save.
    let current = get_by_id(pool, id).await?;

    let new_slug = data.slug.clone().unwrap_or(current.slug.clone());
    let new_title = data.title.clone().unwrap_or(current.title.clone());
    let new_content = data.content.clone().unwrap_or(current.content.clone());
    let new_format = data.content_format.clone().unwrap_or(current.content_format.clone());
    let new_excerpt = data.excerpt.clone().or(current.excerpt.clone());
    let new_status = data.status.as_ref().map(|s| s.as_str().to_string()).unwrap_or(current.status.clone());
    let new_image = if data.featured_image_id.is_some() {
        data.featured_image_id
    } else {
        current.featured_image_id
    };
    let new_published_at = if data.published_at.is_some() {
        data.published_at
    } else {
        current.published_at
    };

    let post = sqlx::query_as::<_, Post>(
        r#"
        UPDATE posts
        SET title = $1, slug = $2, content = $3, content_format = $4, excerpt = $5,
            status = $6, featured_image_id = $7, published_at = $8, updated_at = NOW()
        WHERE id = $9
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
    .bind(id)
    .fetch_one(pool)
    .await?;

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

/// Fetch posts related by shared taxonomy terms (exclude the source post).
pub async fn get_related(pool: &PgPool, post_id: Uuid, limit: i64) -> Result<Vec<Post>> {
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
        ORDER BY p.published_at DESC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(post_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(posts)
}
