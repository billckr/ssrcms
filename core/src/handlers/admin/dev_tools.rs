//! Native "seed test data" actions for /admin/settings → Advanced → Deploy Test Data.
//!
//! Reimplements what scripts/seed_users.sh and scripts/seed_posts.sh do locally via
//! psql/synap, but as in-process Rust calls so it works from any deployed instance
//! (the deploy script never ships scripts/, bash, or a psql client to the target host).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::post::{CreatePost, PostStatus, PostType};
use crate::models::taxonomy::{self, CreateTaxonomy, TaxonomyType};
use crate::models::{post, site_user, user};

fn forbidden() -> axum::response::Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"}))).into_response()
}

fn is_unique_violation(err: &crate::errors::AppError) -> bool {
    matches!(
        err,
        crate::errors::AppError::Database(sqlx::Error::Database(db_err)) if db_err.is_unique_violation()
    )
}

fn rand_suffix(n: usize) -> String {
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = StdRng::from_entropy();
    (0..n)
        .map(|_| chars[rng.gen_range(0..chars.len())] as char)
        .collect()
}

/// Mirrors seed_users.sh's gen_password() bash function.
fn gen_password() -> String {
    user::generate_password()
}

const FIRST_NAMES: &[&str] = &[
    "James", "Mary", "Robert", "Patricia", "John", "Jennifer", "Michael", "Linda",
    "David", "Elizabeth", "William", "Barbara", "Richard", "Susan", "Joseph", "Jessica",
    "Thomas", "Sarah", "Charles", "Karen", "Daniel", "Nancy", "Matthew", "Lisa",
    "Anthony", "Margaret", "Mark", "Betty", "Paul", "Sandra",
];
const LAST_NAMES: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
    "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
    "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
    "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson",
];
const ADJECTIVES: &[&str] = &[
    "Quick", "Lazy", "Bright", "Dark", "Modern", "Ancient", "Silent", "Loud",
    "Hidden", "Bold", "Clever", "Simple", "Complex", "Fresh", "Wild", "Calm",
    "Sharp", "Soft", "Vast", "Narrow", "Golden", "Silver", "Rustic", "Digital",
];
const NOUNS: &[&str] = &[
    "Guide", "Journey", "Story", "Vision", "Future", "Secret", "Path", "World",
    "Truth", "Dream", "Plan", "Theory", "Chapter", "Moment", "Change", "Force",
    "Light", "Shadow", "Wave", "Edge", "Bridge", "Signal", "Layer", "Canvas",
];
const TOPICS: &[&str] = &[
    "Technology", "Design", "Nature", "Travel", "Food", "Music", "Science",
    "History", "Culture", "Business", "Health", "Education", "Art", "Sport",
    "Finance", "Philosophy", "Architecture", "Photography", "Writing", "Code",
];
const CATEGORY_NAMES: &[&str] = &["Technology", "Design", "Business", "Lifestyle", "Tutorial"];
const TAG_NAMES: &[&str] = &["featured", "popular", "tips", "beginner", "advanced"];

fn slugify_word(s: &str) -> String {
    crate::utils::slugify::slugify(s)
}

// ── Seed users ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SeedUsersRequest {
    site_id: Uuid,
    role: String,
    count: u32,
    password: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatedUser {
    email: String,
    /// Only populated when the password was auto-generated (not admin-supplied),
    /// since an admin-supplied password isn't a secret worth echoing back.
    password: Option<String>,
}

pub async fn seed_users(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<SeedUsersRequest>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return forbidden();
    }

    if body.count < 1 || body.count > 200 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "count must be between 1 and 200"}))).into_response();
    }
    let (site_role, users_role) = match body.role.as_str() {
        "admin" => (site_user::SiteRole::Admin, user::UserRole::SiteAdmin),
        "editor" => (site_user::SiteRole::Editor, user::UserRole::Editor),
        "author" => (site_user::SiteRole::Author, user::UserRole::Author),
        "subscriber" => (site_user::SiteRole::Subscriber, user::UserRole::Subscriber),
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "role must be admin, editor, author, or subscriber"}))).into_response();
        }
    };

    let site = match crate::models::site::get_by_id(&state.db, body.site_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
    };

    let mut rng = StdRng::from_entropy();
    let mut created: Vec<CreatedUser> = Vec::new();
    let mut skipped: u32 = 0;

    for _ in 0..body.count {
        let first = FIRST_NAMES.choose(&mut rng).unwrap();
        let last = LAST_NAMES.choose(&mut rng).unwrap();
        let display_name = format!("{first} {last}");
        let suffix = rand_suffix(5);
        let username = format!("{}-{}-{}", first.to_lowercase(), last.to_lowercase(), suffix);
        let email = format!("{username}@{}", site.hostname);

        let (used_password, echoed_password) = match &body.password {
            Some(p) => (p.clone(), None),
            None => {
                let p = gen_password();
                (p.clone(), Some(p))
            }
        };

        let create = user::CreateUser {
            username: username.clone(),
            email: email.clone(),
            display_name,
            password: used_password,
            role: users_role.clone(),
        };

        let new_user = match user::create(&state.db, &create).await {
            Ok(u) => u,
            Err(e) if is_unique_violation(&e) => {
                skipped += 1;
                continue;
            }
            Err(e) => {
                tracing::error!("seed_users: create failed: {e}");
                skipped += 1;
                continue;
            }
        };

        if let Err(e) = site_user::add(&state.db, site.id, new_user.id, site_role, admin.user.id.into(), false).await {
            tracing::error!("seed_users: site_user::add failed: {e}");
            skipped += 1;
            continue;
        }

        // Tag the row so "Clear test data" can optionally remove exactly the users
        // this feature created, and never a real user.
        if let Err(e) = sqlx::query("UPDATE users SET is_seeded = TRUE WHERE id = $1")
            .bind(new_user.id)
            .execute(&state.db)
            .await
        {
            tracing::error!("seed_users: failed to mark is_seeded: {e}");
        }

        created.push(CreatedUser { email, password: echoed_password });
    }

    Json(json!({
        "ok": true,
        "created": created.len(),
        "skipped": skipped,
        "users": created,
    }))
    .into_response()
}

// ── Seed posts / pages ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SeedPostsRequest {
    site_id: Uuid,
    author_email: String,
    post_type: String,
    count: u32,
    status: String,
    extras: bool,
}

pub async fn seed_posts(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<SeedPostsRequest>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return forbidden();
    }

    if body.count < 1 || body.count > 200 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "count must be between 1 and 200"}))).into_response();
    }
    let post_type = match body.post_type.as_str() {
        "post" => PostType::Post,
        "page" => PostType::Page,
        _ => return (StatusCode::BAD_REQUEST, Json(json!({"error": "post_type must be post or page"}))).into_response(),
    };
    if !["mixed", "published", "draft", "pending"].contains(&body.status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "status must be mixed, published, draft, or pending"}))).into_response();
    }

    let site = match crate::models::site::get_by_id(&state.db, body.site_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
    };

    let author = match user::get_by_email(&state.db, &body.author_email).await {
        Ok(u) => u,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No user found with that email"}))).into_response(),
    };

    // Mirrors seed_posts.sh's access check: super_admin OR a site_users row.
    if !admin.caps.is_global_admin {
        let has_role = site_user::has_any_role(&state.db, site.id, author.id).await.unwrap_or(false);
        if author.role != "super_admin" && !has_role {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "That user has no access to the selected site"})),
            )
                .into_response();
        }
    }

    let mut rng = StdRng::from_entropy();
    let statuses = ["published", "draft", "pending"];
    let mut created_ids: Vec<Uuid> = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let mut skipped: u32 = 0;

    for _ in 0..body.count {
        let adj = ADJECTIVES.choose(&mut rng).unwrap();
        let noun = NOUNS.choose(&mut rng).unwrap();
        let topic = TOPICS.choose(&mut rng).unwrap();
        let status_str = if body.status == "mixed" {
            statuses[rng.gen_range(0..statuses.len())]
        } else {
            body.status.as_str()
        };
        let status = match status_str {
            "published" => PostStatus::Published,
            "draft" => PostStatus::Draft,
            _ => PostStatus::Pending,
        };

        let title = format!("{adj} {noun} of {topic}");
        let slug = format!("{}-{}", slugify_word(&title), rand_suffix(4));

        let published_at = if status_str == "published" {
            let days_ago = rng.gen_range(0..90);
            Some(chrono::Utc::now() - chrono::Duration::days(days_ago))
        } else {
            None
        };

        let content = format!(
            "<p>This is a sample post about <strong>{topic}</strong>. Lorem ipsum dolor sit amet, \
             consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna \
             aliqua. This article explores the {adj} aspects of {topic} from a fresh perspective.</p>\
             <p>Pellentesque habitant morbi tristique senectus et netus et malesuada fames. \
             Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.</p>"
        );
        let excerpt = format!("A {adj} look at {topic} — exploring {noun} and beyond.");

        let create = CreatePost {
            site_id: Some(site.id),
            title,
            slug: Some(slug.clone()),
            content,
            content_format: None,
            excerpt: Some(excerpt),
            status,
            post_type: post_type.clone(),
            author_id: author.id,
            featured_image_id: None,
            published_at,
            template: None,
            post_password_hash: None,
            comments_enabled: false,
            parent_id: None,
            sources: Vec::new(),
            sources_public: false,
        };

        match post::create(&state.db, &create).await {
            Ok(p) => {
                urls.push(format!("http://{}/{}", site.hostname, p.slug));
                created_ids.push(p.id);
            }
            Err(e) if is_unique_violation(&e) => skipped += 1,
            Err(e) => {
                tracing::error!("seed_posts: create failed: {e}");
                skipped += 1;
            }
        }
    }

    let mut assigned: u32 = 0;
    if body.extras && !created_ids.is_empty() {
        let mut cat_ids: Vec<Uuid> = Vec::new();
        let mut tag_ids: Vec<Uuid> = Vec::new();

        for name in CATEGORY_NAMES {
            if let Some(id) = ensure_taxonomy(&state, site.id, name, TaxonomyType::Category).await {
                cat_ids.push(id);
            }
        }
        for name in TAG_NAMES {
            if let Some(id) = ensure_taxonomy(&state, site.id, name, TaxonomyType::Tag).await {
                tag_ids.push(id);
            }
        }

        if !cat_ids.is_empty() || !tag_ids.is_empty() {
            let mut shuffled = created_ids.clone();
            shuffled.shuffle(&mut rng);
            let pct = 50 + rng.gen_range(0..51);
            let subset = ((shuffled.len() * pct + 99) / 100).max(1);
            for post_id in &shuffled[..subset] {
                if !cat_ids.is_empty() {
                    let n = rng.gen_range(1..=cat_ids.len().min(3));
                    let mut c = cat_ids.clone();
                    c.shuffle(&mut rng);
                    for tid in &c[..n] {
                        if taxonomy::attach_to_post(&state.db, *post_id, *tid).await.is_ok() {
                            assigned += 1;
                        }
                    }
                }
                if !tag_ids.is_empty() {
                    let n = rng.gen_range(1..=tag_ids.len().min(3));
                    let mut t = tag_ids.clone();
                    t.shuffle(&mut rng);
                    for tid in &t[..n] {
                        if taxonomy::attach_to_post(&state.db, *post_id, *tid).await.is_ok() {
                            assigned += 1;
                        }
                    }
                }
            }
        }
    }

    urls.truncate(20);
    Json(json!({
        "ok": true,
        "created": created_ids.len(),
        "skipped": skipped,
        "assigned": assigned,
        "urls": urls,
    }))
    .into_response()
}

/// Look up a category/tag by slug for this site, creating it if missing.
/// Mirrors seed_posts.sh's `INSERT ... ON CONFLICT DO NOTHING` + re-select.
async fn ensure_taxonomy(state: &AppState, site_id: Uuid, name: &str, kind: TaxonomyType) -> Option<Uuid> {
    let slug = slugify_word(name);
    if let Ok(existing) = taxonomy::get_by_slug(&state.db, Some(site_id), &slug, kind.clone()).await {
        return Some(existing.id);
    }
    let create = CreateTaxonomy {
        site_id: Some(site_id),
        name: name.to_string(),
        slug: slug.clone(),
        taxonomy: kind.clone(),
        description: None,
    };
    match taxonomy::create(&state.db, &create).await {
        Ok(t) => Some(t.id),
        Err(_) => taxonomy::get_by_slug(&state.db, Some(site_id), &slug, kind).await.ok().map(|t| t.id),
    }
}

// ── Clear test data ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ClearRequest {
    site_id: Uuid,
    #[serde(default)]
    delete_users: bool,
}

// ── Nuke all app data (super_admin only, keeps default site + super_admins) ──

#[derive(Debug, Deserialize)]
pub struct NukeAllRequest {
    /// Must equal "DELETE ALL" — a typed confirmation, not just a checkbox,
    /// since this is far more destructive than the per-site Clear Test Data
    /// above (it removes every other site outright and every non-super-admin
    /// user in the app, not just one site's content).
    #[serde(default)]
    confirm: String,
}

/// Removes a deleted site's on-disk data (themes, uploads, plugin dir, and
/// the hostname → uuid upload symlink). Mirrors the cleanup block in
/// `handlers::admin::sites::delete` — kept here instead of shared because
/// that handler is HTTP-response-shaped and this call site isn't.
fn remove_site_disk_data(state: &AppState, site_id: Uuid, hostname: &str) {
    let site_data_dir = std::path::Path::new(&state.config.sites_dir).join(site_id.to_string());
    if site_data_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&site_data_dir) {
            tracing::warn!("nuke_all: failed to remove site data dir for {}: {:?}", site_id, e);
        }
    }
    let sym_path = std::path::Path::new(&state.config.uploads_dir).join(hostname);
    if sym_path.is_symlink() {
        if let Err(e) = std::fs::remove_file(&sym_path) {
            tracing::warn!("nuke_all: failed to remove upload symlink for '{}': {:?}", hostname, e);
        }
    }
    let site_upload_dir = std::path::Path::new(&state.config.uploads_dir).join(site_id.to_string());
    if site_upload_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&site_upload_dir) {
            tracing::warn!("nuke_all: failed to remove upload dir for site {}: {:?}", site_id, e);
        }
    }
    let site_plugin_dir = std::path::Path::new(&state.config.plugins_dir).join("sites").join(site_id.to_string());
    if site_plugin_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&site_plugin_dir) {
            tracing::warn!("nuke_all: failed to remove plugin dir for site {}: {:?}", site_id, e);
        }
    }
}

/// POST /admin/settings/dev-tools/nuke-all — wipe every site but the caller's
/// default site, clear that default site's own content (posts, taxonomies,
/// media, nav menus, form submissions), and delete every non-super-admin
/// user in the app. Exists for repeatedly resetting a dev/test instance
/// between test-data runs (see scripts/populate-wp-test-data.sh's WordPress
/// equivalent) without hand-picking sites in the per-site Clear Test Data
/// tool above.
///
/// super_admin only, and only reachable while on the caller's own default
/// site — same gate as the rest of /admin/settings (`can_manage_settings`).
pub async fn nuke_all(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<NukeAllRequest>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings || !admin.caps.is_global_admin {
        return forbidden();
    }
    if body.confirm != "DELETE ALL" {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Confirmation text did not match \"DELETE ALL\""}))).into_response();
    }

    let default_site_id = match admin.user.default_site_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Your account has no default site set — set one under Sites before using this."})),
            )
                .into_response();
        }
    };

    let sites = match crate::models::site::list(&state.db).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("nuke_all: failed to list sites: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };

    // Delete every other site outright — model-level delete cascades posts,
    // media, taxonomies, comments, form_submissions, nav_menus, site_users,
    // site_plugins, and removes now-fully-orphaned non-super-admin users.
    let mut deleted_sites: u32 = 0;
    for site in &sites {
        if site.id == default_site_id {
            continue;
        }
        if let Err(e) = crate::models::site::delete(&state.db, site.id).await {
            tracing::error!("nuke_all: failed to delete site {}: {e}", site.id);
            continue;
        }
        remove_site_disk_data(&state, site.id, &site.hostname);
        deleted_sites += 1;
    }

    // Clear the default site's own content (same scope as clear_test_data
    // above) — the site row itself, its settings, and its theme are kept.
    let media_paths: Vec<String> = sqlx::query_scalar("SELECT path FROM media WHERE site_id = $1")
        .bind(default_site_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("nuke_all: begin failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };
    let clear_result: Result<(), sqlx::Error> = async {
        sqlx::query("DELETE FROM posts WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM taxonomies WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM form_submissions WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media_folders WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM nav_menus WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        // builder_projects cascades to page_compositions.project_id.
        sqlx::query("DELETE FROM builder_projects WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM page_compositions WHERE site_id = $1").bind(default_site_id).execute(&mut *tx).await?;
        Ok(())
    }
    .await;
    if let Err(e) = clear_result {
        tracing::error!("nuke_all: failed to clear default site content: {e}");
        let _ = tx.rollback().await;
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to clear default site content — no further changes were made"}))).into_response();
    }
    if let Err(e) = tx.commit().await {
        tracing::error!("nuke_all: commit failed: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
    }

    // Remove the default site's uploaded files from disk now that the DB
    // rows are gone (best-effort — a missing file is not an error).
    for path in &media_paths {
        let full_path = std::path::Path::new(&state.config.uploads_dir).join(path);
        if let Err(e) = std::fs::remove_file(&full_path) {
            tracing::warn!("nuke_all: failed to remove media file {:?}: {:?}", full_path, e);
        }
    }

    // Delete every non-super-admin user left in the app (including any on
    // the default site — its content is already gone, so posts/media
    // ON DELETE RESTRICT won't block it). One at a time so a single
    // unexpected FK reference doesn't abort the whole batch.
    let victim_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE role != 'super_admin'")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    let mut deleted_users: u32 = 0;
    let mut skipped_users: u32 = 0;
    for uid in victim_ids {
        match sqlx::query("DELETE FROM users WHERE id = $1").bind(uid).execute(&state.db).await {
            Ok(_) => deleted_users += 1,
            Err(e) => {
                tracing::warn!("nuke_all: failed to delete user {}: {:?}", uid, e);
                skipped_users += 1;
            }
        }
    }

    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("nuke_all: site cache reload failed: {:?}", e);
    }

    super::audit(
        &state,
        &admin,
        "app.nuked_test_data",
        "app",
        None,
        &format!("{deleted_sites} site(s), {deleted_users} user(s)"),
        None,
    )
    .await;

    Json(json!({
        "ok": true,
        "deleted_sites": deleted_sites,
        "deleted_users": deleted_users,
        "skipped_users": skipped_users,
    }))
    .into_response()
}

pub async fn clear_test_data(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<ClearRequest>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings {
        return forbidden();
    }

    let site = match crate::models::site::get_by_id(&state.db, body.site_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
    };

    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("clear_test_data: begin failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };

    // Deletes posts/pages, taxonomies, form submissions, media rows, nav menus, and
    // Page Builder projects/pages for the site, all within this one transaction. Site
    // settings are always untouched. Users are only removed when delete_users is set,
    // and even then only rows tagged is_seeded — i.e. exactly the users this feature
    // created, never a real account.
    let result: Result<i64, sqlx::Error> = async {
        sqlx::query("DELETE FROM posts WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM taxonomies WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM form_submissions WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media_folders WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM nav_menus WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        // builder_projects cascades to page_compositions.project_id.
        sqlx::query("DELETE FROM builder_projects WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM page_compositions WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;

        if body.delete_users {
            let deleted = sqlx::query(
                "DELETE FROM users WHERE is_seeded = TRUE \
                 AND id IN (SELECT user_id FROM site_users WHERE site_id = $1)",
            )
            .bind(site.id)
            .execute(&mut *tx)
            .await?;
            Ok(deleted.rows_affected() as i64)
        } else {
            Ok(0)
        }
    }
    .await;

    match result {
        Ok(deleted_users) => {
            if let Err(e) = tx.commit().await {
                tracing::error!("clear_test_data: commit failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
            }
            Json(json!({"ok": true, "deleted_users": deleted_users})).into_response()
        }
        Err(e) => {
            tracing::error!("clear_test_data: delete failed: {e}");
            let _ = tx.rollback().await;
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to clear data — no changes were made"}))).into_response()
        }
    }
}

/// POST /admin/settings/dev-tools/reindex-search — rebuild the Tantivy search
/// index from the database on demand, so posts added/edited outside the admin
/// handlers (imports, seed scripts, direct DB writes) become searchable without
/// a full app restart. Runs the same rebuild used at startup
/// (`search::indexer::rebuild_index`), just triggered manually and awaited so
/// the UI can report how many documents were indexed.
///
/// Index-wide (covers every site), so gated like Nuke All: super_admin only.
pub async fn reindex_search(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_settings || !admin.caps.is_global_admin {
        return forbidden();
    }

    let index = (*state.search_index).clone();
    let db = state.db.clone();
    match crate::search::indexer::rebuild_index(index, db).await {
        Some(count) => Json(json!({"ok": true, "indexed": count})).into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Reindex failed — check server logs"}))).into_response(),
    }
}
