//! Maps an imported WordPress attachment's old URL to the Synap `media` row it
//! became. Written by the WP media importer (`handlers/admin/wp_import.rs`);
//! read by a future post-content importer to rewrite `<img>` references
//! found in imported post/page bodies.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

pub async fn record(pool: &PgPool, site_id: Uuid, old_url: &str, media_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO wp_import_media_map (site_id, old_url, media_id) VALUES ($1, $2, $3) \
         ON CONFLICT (site_id, old_url) DO UPDATE SET media_id = EXCLUDED.media_id",
    )
    .bind(site_id)
    .bind(old_url)
    .bind(media_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Looks up a previously-imported attachment by its old WP URL — lets a
/// re-run of the importer (same or a later export from the same site) reuse
/// the already-downloaded file instead of re-fetching it.
pub async fn find(pool: &PgPool, site_id: Uuid, old_url: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT media_id FROM wp_import_media_map WHERE site_id = $1 AND old_url = $2",
    )
    .bind(site_id)
    .bind(old_url)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| id))
}

/// Every old-URL → media_id mapping recorded for a site, for the
/// post-content importer to rewrite `<img>` references in bulk without a
/// query per attachment.
pub async fn map_for_site(pool: &PgPool, site_id: Uuid) -> Result<HashMap<String, Uuid>> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT old_url, media_id FROM wp_import_media_map WHERE site_id = $1",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Records which Synap post a WXR item (by its `wp:post_id`) became, so a
/// later re-run of the same export updates that post instead of creating a
/// duplicate. See `find_post`.
pub async fn record_post(pool: &PgPool, site_id: Uuid, wp_post_id: &str, post_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO wp_import_post_map (site_id, wp_post_id, post_id) VALUES ($1, $2, $3) \
         ON CONFLICT (site_id, wp_post_id) DO UPDATE SET post_id = EXCLUDED.post_id",
    )
    .bind(site_id)
    .bind(wp_post_id)
    .bind(post_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Looks up the Synap post a WXR item was previously imported as, if any —
/// `None` means this item hasn't been imported for this site before (or the
/// post it became was since deleted, which cascades this row away too).
pub async fn find_post(pool: &PgPool, site_id: Uuid, wp_post_id: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT post_id FROM wp_import_post_map WHERE site_id = $1 AND wp_post_id = $2",
    )
    .bind(site_id)
    .bind(wp_post_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| id))
}
