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
