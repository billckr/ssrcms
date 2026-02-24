//! Integration tests for model CRUD operations.
//!
//! All tests require a live PostgreSQL instance.
//!
//! Run with:
//!   DATABASE_URL=postgres://user:pass@localhost/synaptic_signals \
//!     cargo test -p synaptic-core --test model_crud -- --include-ignored

use synaptic_core::db;
use synaptic_core::models::{post, taxonomy, user};
use synaptic_core::models::post::{CreatePost, ListFilter, PostStatus, PostType, UpdatePost};
use synaptic_core::models::taxonomy::{CreateTaxonomy, TaxonomyType};
use synaptic_core::models::user::{CreateUser, UserRole};
use synaptic_core::models::site;
use synaptic_core::models::site_user;

async fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests");
    db::connect(&url).await.expect("failed to connect to test database")
}

/// Generate a unique 8-char suffix for test isolation.
fn uid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}

// ── Helpers ────────────────────────────────────────────────────────────────────

async fn make_test_user(pool: &sqlx::PgPool) -> user::User {
    let id = uid();
    user::create(pool, &CreateUser {
        username: format!("testuser_{id}"),
        email: format!("test_{id}@example.com"),
        display_name: format!("Test User {id}"),
        password: "TestPass123!".to_string(),
        role: UserRole::Author,
    })
    .await
    .expect("failed to create test user")
}

async fn make_test_post(pool: &sqlx::PgPool, author_id: uuid::Uuid, status: PostStatus) -> post::Post {
    let id = uid();
    post::create(pool, &CreatePost {
        site_id: None,
        title: format!("Test Post {id}"),
        slug: Some(format!("test-post-{id}")),
        content: "<p>Integration test content.</p>".to_string(),
        content_format: Some("html".to_string()),
        excerpt: None,
        status,
        post_type: PostType::Post,
        author_id,
        featured_image_id: None,
        published_at: Some(chrono::Utc::now()),
    })
    .await
    .expect("failed to create test post")
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_create_and_get() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let created = make_test_post(&pool, author.id, PostStatus::Published).await;

    let fetched = post::get_by_id(&pool, created.id)
        .await
        .expect("get_by_id should succeed");

    assert_eq!(fetched.title, created.title);
    assert_eq!(fetched.slug, created.slug);
    assert_eq!(fetched.author_id, author.id);

    // Cleanup
    post::delete(&pool, created.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_update() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let created = make_test_post(&pool, author.id, PostStatus::Draft).await;

    let updated = post::update(&pool, created.id, &UpdatePost {
        title: Some("Updated Title".to_string()),
        slug: None,
        content: None,
        content_format: None,
        excerpt: None,
        status: Some(PostStatus::Published),
        featured_image_id: None,
        published_at: None,
    })
    .await
    .expect("update should succeed");

    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.status, "published");

    // Cleanup
    post::delete(&pool, created.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_delete() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let created = make_test_post(&pool, author.id, PostStatus::Draft).await;
    let post_id = created.id;

    post::delete(&pool, post_id)
        .await
        .expect("delete should succeed");

    let result = post::get_by_id(&pool, post_id).await;
    assert!(result.is_err(), "get_by_id should return Err after delete");

    // Cleanup
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_list_filter_published_only() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let published = make_test_post(&pool, author.id, PostStatus::Published).await;
    let draft = make_test_post(&pool, author.id, PostStatus::Draft).await;

    let results = post::list(&pool, &ListFilter {
        status: Some(PostStatus::Published),
        post_type: Some(PostType::Post),
        author_id: Some(author.id),
        ..Default::default()
    })
    .await
    .expect("list should succeed");

    let ids: Vec<_> = results.iter().map(|p| p.id).collect();
    assert!(ids.contains(&published.id), "published post should be in results");
    assert!(!ids.contains(&draft.id), "draft post should not be in filtered results");

    // Cleanup
    post::delete(&pool, published.id).await.ok();
    post::delete(&pool, draft.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_user_create_and_auth() {
    let pool = test_pool().await;

    let id = uid();
    let created = user::create(&pool, &CreateUser {
        username: format!("authuser_{id}"),
        email: format!("auth_{id}@example.com"),
        display_name: format!("Auth User {id}"),
        password: "CorrectPass!99".to_string(),
        role: UserRole::Author,
    })
    .await
    .expect("user create should succeed");

    assert!(
        created.verify_password("CorrectPass!99"),
        "verify_password should return true for correct password"
    );
    assert!(
        !created.verify_password("WrongPass!99"),
        "verify_password should return false for wrong password"
    );

    // Cleanup
    user::delete(&pool, created.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_taxonomy_attach_detach() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let p = make_test_post(&pool, author.id, PostStatus::Published).await;

    let id = uid();
    let term = taxonomy::create(&pool, &CreateTaxonomy {
        site_id: None,
        name: format!("Test Category {id}"),
        slug: format!("test-cat-{id}"),
        taxonomy: TaxonomyType::Category,
        description: None,
    })
    .await
    .expect("taxonomy create should succeed");

    // Attach
    taxonomy::attach_to_post(&pool, p.id, term.id)
        .await
        .expect("attach_to_post should succeed");

    let terms = taxonomy::for_post(&pool, p.id)
        .await
        .expect("for_post should succeed");
    let term_ids: Vec<_> = terms.iter().map(|t| t.id).collect();
    assert!(term_ids.contains(&term.id), "term should be attached to post");

    // Detach
    taxonomy::detach_from_post(&pool, p.id, term.id)
        .await
        .expect("detach_from_post should succeed");

    let terms_after = taxonomy::for_post(&pool, p.id)
        .await
        .expect("for_post after detach should succeed");
    assert!(
        terms_after.iter().all(|t| t.id != term.id),
        "term should be detached from post"
    );

    // Cleanup
    taxonomy::delete(&pool, term.id).await.ok();
    post::delete(&pool, p.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_meta_set_get() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;
    let p = make_test_post(&pool, author.id, PostStatus::Draft).await;

    post::set_meta(&pool, p.id, "seo_title", "Custom SEO Title")
        .await
        .expect("set_meta should succeed");

    let meta = post::get_meta(&pool, p.id)
        .await
        .expect("get_meta should succeed");

    assert_eq!(
        meta.get("seo_title").map(|s| s.as_str()),
        Some("Custom SEO Title"),
        "meta key 'seo_title' should have expected value"
    );

    // Cleanup
    post::delete(&pool, p.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_user_delete_cascades_posts() {
    let pool = test_pool().await;

    let author = make_test_user(&pool).await;

    // Create a post and a page under this user
    let id = uid();
    let p = post::create(&pool, &CreatePost {
        site_id: None,
        title: format!("Cascade Post {id}"),
        slug: Some(format!("cascade-post-{id}")),
        content: "<p>Will be cascaded.</p>".to_string(),
        content_format: Some("html".to_string()),
        excerpt: None,
        status: PostStatus::Draft,
        post_type: PostType::Post,
        author_id: author.id,
        featured_image_id: None,
        published_at: None,
    })
    .await
    .expect("post create should succeed");

    let id2 = uid();
    let pg = post::create(&pool, &CreatePost {
        site_id: None,
        title: format!("Cascade Page {id2}"),
        slug: Some(format!("cascade-page-{id2}")),
        content: "<p>Will be cascaded.</p>".to_string(),
        content_format: Some("html".to_string()),
        excerpt: None,
        status: PostStatus::Draft,
        post_type: PostType::Page,
        author_id: author.id,
        featured_image_id: None,
        published_at: None,
    })
    .await
    .expect("page create should succeed");

    // Delete user — should cascade to both posts
    user::delete(&pool, author.id)
        .await
        .expect("user delete should succeed");

    assert!(
        user::get_by_id(&pool, author.id).await.is_err(),
        "user should be gone after delete"
    );
    assert!(
        post::get_by_id(&pool, p.id).await.is_err(),
        "post should be cascaded after user delete"
    );
    assert!(
        post::get_by_id(&pool, pg.id).await.is_err(),
        "page should be cascaded after user delete"
    );
}

// ── Ownership & Role Tests ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_create_site_with_defaults_seeds_settings_and_admin_role() {
    let pool = test_pool().await;
    let owner = make_test_user(&pool).await;
    let id = uid();

    let s = site::create_with_defaults(&pool, &format!("{id}.example.com"), owner.id)
        .await
        .expect("create_with_defaults should succeed");

    // owner_user_id is recorded
    assert_eq!(s.owner_user_id, Some(owner.id));

    // site_settings rows exist
    let settings = synaptic_core::app_state::SiteSettings::load(&pool, s.id)
        .await
        .expect("site settings should be loadable");
    assert_eq!(settings.active_theme, "default");
    assert!(!settings.site_name.is_empty());

    // owner has admin role in site_users
    let role = site_user::get_role(&pool, s.id, owner.id)
        .await
        .expect("get_role should succeed");
    assert_eq!(role.as_deref(), Some("admin"), "owner should be admin on their site");

    // Cleanup
    site::delete(&pool, s.id).await.ok();
    user::delete(&pool, owner.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_list_by_owner_scoped_to_creator() {
    let pool = test_pool().await;
    let admin1 = make_test_user(&pool).await;
    let admin2 = make_test_user(&pool).await;
    let id = uid();

    let s1 = site::create_with_defaults(&pool, &format!("admin1-{id}.example.com"), admin1.id)
        .await
        .expect("create s1");
    let s2 = site::create_with_defaults(&pool, &format!("admin2-{id}.example.com"), admin2.id)
        .await
        .expect("create s2");

    let admin1_sites = site::list_by_owner(&pool, admin1.id)
        .await
        .expect("list_by_owner should succeed");
    let site_ids: Vec<_> = admin1_sites.iter().map(|s| s.id).collect();

    assert!(site_ids.contains(&s1.id), "admin1 should see their site");
    assert!(!site_ids.contains(&s2.id), "admin1 should NOT see admin2's site");

    // Cleanup
    site::delete(&pool, s1.id).await.ok();
    site::delete(&pool, s2.id).await.ok();
    user::delete(&pool, admin1.id).await.ok();
    user::delete(&pool, admin2.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_invited_by_recorded_on_site_user() {
    let pool = test_pool().await;
    let inviter = make_test_user(&pool).await;
    let invitee = make_test_user(&pool).await;
    let id = uid();

    let s = site::create_with_defaults(&pool, &format!("invitetest-{id}.example.com"), inviter.id)
        .await
        .expect("create site");

    site_user::add(&pool, s.id, invitee.id, "author", Some(inviter.id))
        .await
        .expect("add invitee");

    // Check that invited_by is stored correctly
    let su = sqlx::query_as::<_, synaptic_core::models::site_user::SiteUser>(
        "SELECT * FROM site_users WHERE site_id = $1 AND user_id = $2",
    )
    .bind(s.id)
    .bind(invitee.id)
    .fetch_one(&pool)
    .await
    .expect("should find site_user row");

    assert_eq!(su.invited_by, Some(inviter.id), "invited_by should be the inviter");

    // Cleanup
    site::delete(&pool, s.id).await.ok();
    user::delete(&pool, inviter.id).await.ok();
    user::delete(&pool, invitee.id).await.ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_soft_delete_preserves_posts() {
    let pool = test_pool().await;
    let author = make_test_user(&pool).await;
    let p = make_test_post(&pool, author.id, PostStatus::Published).await;

    user::soft_delete(&pool, author.id)
        .await
        .expect("soft_delete should succeed");

    // User no longer visible in get_by_id (filtered as deleted)
    assert!(
        user::get_by_id(&pool, author.id).await.is_err(),
        "soft-deleted user should not be returned by get_by_id"
    );

    // Post is still in the DB
    let fetched = post::get_by_id(&pool, p.id)
        .await
        .expect("post should survive soft delete of author");
    assert_eq!(fetched.id, p.id);

    // User absent from list()
    let all_users = user::list(&pool).await.expect("list should succeed");
    assert!(
        all_users.iter().all(|u| u.id != author.id),
        "soft-deleted user should not appear in list()"
    );

    // Hard cleanup (need to bypass soft delete)
    post::delete(&pool, p.id).await.ok();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(author.id)
        .execute(&pool)
        .await
        .ok();
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_soft_delete_idempotent_on_already_deleted_user() {
    let pool = test_pool().await;
    let author = make_test_user(&pool).await;

    user::soft_delete(&pool, author.id).await.expect("first soft_delete");
    // Second call should return NotFound (already deleted_at IS NOT NULL)
    let result = user::soft_delete(&pool, author.id).await;
    assert!(result.is_err(), "soft_delete on already-deleted user should fail");

    // Hard cleanup
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(author.id)
        .execute(&pool)
        .await
        .ok();
}
