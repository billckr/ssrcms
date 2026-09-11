//! AI-generated translations of a post/page's title, excerpt, and content
//! into another language. One row per (post, locale) — see
//! `crate::translate` for the code that generates these, and
//! `crate::models::site_locale` for which locales a site has enabled.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::errors::Result;
use crate::models::post::Post;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostTranslationRow {
    pub id: Uuid,
    pub post_id: Uuid,
    pub locale: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub content: String,
    /// `posts.updated_at` at the time this translation was generated — used
    /// to flag a translation as stale once the source post changes again.
    pub source_updated_at: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PostTranslationRow {
    /// Whether the source post has changed since this translation was
    /// generated — the editor uses this to show a "source changed" badge.
    pub fn is_stale(&self, post: &Post) -> bool {
        post.updated_at > self.source_updated_at
    }
}

pub async fn get(pool: &PgPool, post_id: Uuid, locale: &str) -> Result<Option<PostTranslationRow>> {
    let row = sqlx::query_as::<_, PostTranslationRow>(
        "SELECT * FROM post_translations WHERE post_id = $1 AND locale = $2",
    )
    .bind(post_id)
    .bind(locale)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Every translation for a post, ordered by locale.
pub async fn list_for_post(pool: &PgPool, post_id: Uuid) -> Result<Vec<PostTranslationRow>> {
    let rows = sqlx::query_as::<_, PostTranslationRow>(
        "SELECT * FROM post_translations WHERE post_id = $1 ORDER BY locale",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Create or replace the translation for this (post, locale) pair, bumping
/// `generated_at` to now.
pub async fn upsert(
    pool: &PgPool,
    post_id: Uuid,
    locale: &str,
    title: &str,
    excerpt: Option<&str>,
    content: &str,
    source_updated_at: DateTime<Utc>,
) -> Result<PostTranslationRow> {
    let row = sqlx::query_as::<_, PostTranslationRow>(
        "INSERT INTO post_translations (post_id, locale, title, excerpt, content, source_updated_at, generated_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())
         ON CONFLICT (post_id, locale) DO UPDATE
         SET title = EXCLUDED.title,
             excerpt = EXCLUDED.excerpt,
             content = EXCLUDED.content,
             source_updated_at = EXCLUDED.source_updated_at,
             generated_at = NOW(),
             updated_at = NOW()
         RETURNING *",
    )
    .bind(post_id)
    .bind(locale)
    .bind(title)
    .bind(excerpt)
    .bind(content)
    .bind(source_updated_at)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Transaction-friendly form used when a post translation and translations
/// of its embedded resources must become visible atomically.
pub async fn upsert_on(
    conn: &mut PgConnection,
    post_id: Uuid,
    locale: &str,
    title: &str,
    excerpt: Option<&str>,
    content: &str,
    source_updated_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO post_translations (post_id, locale, title, excerpt, content, source_updated_at, generated_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())
         ON CONFLICT (post_id, locale) DO UPDATE
         SET title = EXCLUDED.title,
             excerpt = EXCLUDED.excerpt,
             content = EXCLUDED.content,
             source_updated_at = EXCLUDED.source_updated_at,
             generated_at = NOW(),
             updated_at = NOW()",
    )
    .bind(post_id)
    .bind(locale)
    .bind(title)
    .bind(excerpt)
    .bind(content)
    .bind(source_updated_at)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, post_id: Uuid, locale: &str) -> Result<()> {
    sqlx::query("DELETE FROM post_translations WHERE post_id = $1 AND locale = $2")
        .bind(post_id)
        .bind(locale)
        .execute(pool)
        .await?;
    Ok(())
}
