//! Native "seed test data" actions for /admin/settings → Advanced → Deploy Test Data.
//!
//! Reimplements what scripts/seed_users.sh and scripts/seed_posts.sh do locally via
//! psql/synap-cli, but as in-process Rust calls so it works from any deployed instance
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

/// 8-char password satisfying the app's validate_password rule: 1 upper, 1 digit,
/// 1 symbol from !@#$%&, rest lowercase, then shuffled. Mirrors seed_users.sh's
/// gen_password() bash function.
fn gen_password() -> String {
    let mut rng = StdRng::from_entropy();
    let lower = b"abcdefghijklmnopqrstuvwxyz";
    let symbols = b"!@#$%&";
    let mut chars: Vec<char> = Vec::with_capacity(8);
    chars.push((lower[rng.gen_range(0..lower.len())] as char).to_ascii_uppercase());
    chars.push(char::from_digit(rng.gen_range(0..10), 10).unwrap());
    chars.push(symbols[rng.gen_range(0..symbols.len())] as char);
    for _ in 0..5 {
        chars.push(lower[rng.gen_range(0..lower.len())] as char);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
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
        "admin" => ("admin", user::UserRole::SiteAdmin),
        "editor" => ("editor", user::UserRole::Editor),
        "author" => ("author", user::UserRole::Author),
        "subscriber" => ("subscriber", user::UserRole::Subscriber),
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

        if let Err(e) = site_user::add(&state.db, site.id, new_user.id, site_role, admin.user.id.into()).await {
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
        let has_role = matches!(site_user::get_role(&state.db, site.id, author.id).await, Ok(Some(_)));
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

    // Deletes posts/pages, taxonomies, form submissions, media rows, and nav menus for
    // the site, all within this one transaction. Site settings are always untouched.
    // Users are only removed when delete_users is set, and even then only rows tagged
    // is_seeded — i.e. exactly the users this feature created, never a real account.
    let result: Result<i64, sqlx::Error> = async {
        sqlx::query("DELETE FROM posts WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM taxonomies WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM form_submissions WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM media_folders WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM nav_menus WHERE site_id = $1").bind(site.id).execute(&mut *tx).await?;

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
