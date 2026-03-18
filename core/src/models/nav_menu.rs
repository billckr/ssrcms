//! Navigation menu model — database-backed nav menus populated into the Tera template context.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::Result;
use crate::templates::context::{NavContext, NavItemContext, NavMenuContext};

/// Lightweight entry used to inject all site menus into the builder Tera context.
#[derive(Debug, Clone, Serialize)]
pub struct BuilderMenuEntry {
    pub id: Uuid,
    pub name: String,
    pub items: Vec<NavItemContext>,
}

/// Load all menus for a site and build their item trees.
/// Returns a `HashMap<menu_uuid_string, BuilderMenuEntry>` for easy Tera subscript lookup.
/// All errors are swallowed — a broken menu never breaks the builder.
pub async fn load_all_for_builder(
    pool: &PgPool,
    site_id: Uuid,
) -> HashMap<String, BuilderMenuEntry> {
    let menus = match list_by_site(pool, site_id).await {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    let mut result = HashMap::new();
    for menu in menus {
        let items = match items_for_menu(pool, menu.id).await {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Resolve page_ids → URL paths
        let page_ids: Vec<Uuid> = items
            .iter()
            .filter_map(|i| i.page_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let mut page_urls: HashMap<Uuid, String> = HashMap::new();
        for pid in page_ids {
            if let Ok(page) = crate::models::post::get_by_id(pool, pid).await {
                let path = crate::models::post::get_full_page_path(pool, &page).await;
                page_urls.insert(pid, path);
            }
        }

        let tree = build_tree(&items, &page_urls, None, "");
        result.insert(
            menu.id.to_string(),
            BuilderMenuEntry { id: menu.id, name: menu.name, items: tree },
        );
    }
    result
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NavMenu {
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub location: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NavMenuItem {
    pub id: Uuid,
    pub menu_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub sort_order: i32,
    pub label: String,
    pub url: Option<String>,
    pub page_id: Option<Uuid>,
    pub target: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Menu CRUD ────────────────────────────────────────────────────────────────

pub async fn list_by_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<NavMenu>> {
    let menus = sqlx::query_as::<_, NavMenu>(
        "SELECT * FROM nav_menus WHERE site_id = $1 ORDER BY name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;
    Ok(menus)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<NavMenu> {
    sqlx::query_as::<_, NavMenu>("SELECT * FROM nav_menus WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::errors::AppError::NotFound(format!("nav_menu {id}")))
}

pub async fn get_by_location(
    pool: &PgPool,
    site_id: Uuid,
    location: &str,
) -> Result<Option<NavMenu>> {
    Ok(sqlx::query_as::<_, NavMenu>(
        "SELECT * FROM nav_menus WHERE site_id = $1 AND location = $2 LIMIT 1",
    )
    .bind(site_id)
    .bind(location)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(
    pool: &PgPool,
    site_id: Uuid,
    name: &str,
    location: Option<&str>,
) -> Result<NavMenu> {
    let menu = sqlx::query_as::<_, NavMenu>(
        "INSERT INTO nav_menus (site_id, name, location) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(site_id)
    .bind(name)
    .bind(location)
    .fetch_one(pool)
    .await?;
    Ok(menu)
}

/// Update a menu's name and/or location.
/// If setting a location, clears any existing menu at that location for the same site first
/// (enforces at-most-one-menu-per-location in the application layer).
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    location: Option<&str>,
) -> Result<()> {
    // Fetch current record to know the site_id
    let current = get_by_id(pool, id).await?;

    // If assigning a location, unassign any other menu currently at that location
    if let Some(loc) = location {
        if !loc.is_empty() {
            sqlx::query(
                "UPDATE nav_menus SET location = NULL, updated_at = NOW() \
                 WHERE site_id = $1 AND location = $2 AND id != $3",
            )
            .bind(current.site_id)
            .bind(loc)
            .bind(id)
            .execute(pool)
            .await?;
        }
    }

    sqlx::query(
        "UPDATE nav_menus SET name = $1, location = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(name)
    .bind(location.filter(|s| !s.is_empty()))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM nav_menus WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Item CRUD ────────────────────────────────────────────────────────────────

pub async fn items_for_menu(pool: &PgPool, menu_id: Uuid) -> Result<Vec<NavMenuItem>> {
    let items = sqlx::query_as::<_, NavMenuItem>(
        "SELECT * FROM nav_menu_items WHERE menu_id = $1 ORDER BY sort_order, created_at",
    )
    .bind(menu_id)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_item(
    pool: &PgPool,
    menu_id: Uuid,
    parent_id: Option<Uuid>,
    sort_order: i32,
    label: &str,
    url: Option<&str>,
    page_id: Option<Uuid>,
    target: &str,
) -> Result<NavMenuItem> {
    let item = sqlx::query_as::<_, NavMenuItem>(
        "INSERT INTO nav_menu_items (menu_id, parent_id, sort_order, label, url, page_id, target) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(menu_id)
    .bind(parent_id)
    .bind(sort_order)
    .bind(label)
    .bind(url.filter(|s| !s.is_empty()))
    .bind(page_id)
    .bind(target)
    .fetch_one(pool)
    .await?;
    Ok(item)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_item(
    pool: &PgPool,
    item_id: Uuid,
    parent_id: Option<Uuid>,
    sort_order: i32,
    label: &str,
    url: Option<&str>,
    page_id: Option<Uuid>,
    target: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE nav_menu_items \
         SET parent_id = $1, sort_order = $2, label = $3, url = $4, page_id = $5, target = $6 \
         WHERE id = $7",
    )
    .bind(parent_id)
    .bind(sort_order)
    .bind(label)
    .bind(url.filter(|s| !s.is_empty()))
    .bind(page_id)
    .bind(target)
    .bind(item_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_item(pool: &PgPool, item_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM nav_menu_items WHERE id = $1")
        .bind(item_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Context builder ───────────────────────────────────────────────────────────

/// Build a [`NavContext`] for the given site and request path.
/// Loads the primary and footer menus from the database, resolves page URLs,
/// and assembles a nested tree of [`NavItemContext`] values.
/// Errors are swallowed — a broken menu never breaks the page.
pub async fn build_nav_context(
    pool: &PgPool,
    site_id: Uuid,
    request_path: &str,
) -> NavContext {
    let primary = load_menu_for_location(pool, site_id, "primary", request_path).await;
    let footer  = load_menu_for_location(pool, site_id, "footer",  request_path).await;
    NavContext { primary, footer }
}

async fn load_menu_for_location(
    pool: &PgPool,
    site_id: Uuid,
    location: &str,
    request_path: &str,
) -> NavMenuContext {
    let menu = match get_by_location(pool, site_id, location).await {
        Ok(Some(m)) => m,
        _ => return NavMenuContext::default(),
    };

    let items = match items_for_menu(pool, menu.id).await {
        Ok(v) => v,
        Err(_) => return NavMenuContext::default(),
    };

    // Resolve page_ids → URL paths in one batch
    let page_ids: Vec<Uuid> = items
        .iter()
        .filter_map(|i| i.page_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut page_urls: HashMap<Uuid, String> = HashMap::new();
    for pid in page_ids {
        if let Ok(page) = crate::models::post::get_by_id(pool, pid).await {
            let path = crate::models::post::get_full_page_path(pool, &page).await;
            page_urls.insert(pid, path);
        }
    }

    let tree = build_tree(&items, &page_urls, None, request_path);
    NavMenuContext { items: tree }
}

pub fn build_tree(
    items: &[NavMenuItem],
    page_urls: &HashMap<Uuid, String>,
    parent_id: Option<Uuid>,
    request_path: &str,
) -> Vec<NavItemContext> {
    items
        .iter()
        .filter(|i| i.parent_id == parent_id)
        .map(|i| {
            let url = if let Some(pid) = i.page_id {
                page_urls.get(&pid).cloned().unwrap_or_default()
            } else {
                i.url.clone().unwrap_or_default()
            };
            let is_current = !url.is_empty() && url == request_path;
            let children = build_tree(items, page_urls, Some(i.id), request_path);
            NavItemContext {
                label: i.label.clone(),
                url,
                target: i.target.clone(),
                is_current,
                children,
            }
        })
        .collect()
}
