//! End-to-end theme rendering test.
//!
//! Full pipeline: DB → create user/post → PostContext::build → TemplateEngine::new
//! → render single.html → assert HTML contains expected content.
//!
//! Requires a live PostgreSQL instance and the themes/default/ directory at workspace root.
//!
//! Run with:
//!   DATABASE_URL=postgres://user:pass@localhost/synaptic_signals \
//!     cargo test -p synaptic-core --test theme_e2e -- --include-ignored

use std::collections::HashMap;
use std::sync::Arc;

use synaptic_core::db;
use synaptic_core::models::post::{self, CreatePost, PostContext, PostStatus, PostType};
use synaptic_core::models::user::{self, CreateUser, UserRole};
use synaptic_core::plugins::HookRegistry;
use synaptic_core::templates::TemplateEngine;
use synaptic_core::templates::context::{
    ContextBuilder, NavContext, RequestContext, SessionContext, SiteContext,
};

async fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests");
    db::connect(&url).await.expect("failed to connect to test database")
}

fn uid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}

#[tokio::test]
#[ignore = "requires live PostgreSQL and themes/default/ at workspace root"]
async fn test_single_post_renders_html() {
    let pool = test_pool().await;
    let id = uid();

    // ── Create test user and post ──────────────────────────────────────────────
    let author = user::create(&pool, &CreateUser {
        username: format!("e2euser_{id}"),
        email: format!("e2e_{id}@example.com"),
        display_name: format!("E2E Author {id}"),
        password: "E2ePass!123".to_string(),
        role: UserRole::Author,
    })
    .await
    .expect("failed to create test user");

    let p = post::create(&pool, &CreatePost {
        site_id: None,
        title: format!("E2E Test Post {id}"),
        slug: Some(format!("e2e-test-post-{id}")),
        content: format!("<p>E2E content for post {id}.</p>"),
        content_format: Some("html".to_string()),
        excerpt: None,
        status: PostStatus::Published,
        post_type: PostType::Post,
        author_id: author.id,
        featured_image_id: None,
        published_at: Some(chrono::Utc::now()),
        template: None,
        post_password_hash: None,
        comments_enabled: false,
        parent_id: None,
    })
    .await
    .expect("failed to create test post");

    // ── Build PostContext ──────────────────────────────────────────────────────
    let base_url = "http://localhost:3000";
    let post_ctx = PostContext::build(
        &p,
        &author,
        vec![],       // categories
        vec![],       // tags
        None,         // featured_image
        HashMap::new(), // meta
        0,            // comment_count
        base_url,
        None,         // page_path
        vec![],       // breadcrumbs
    );

    // ── Initialise TemplateEngine ─────────────────────────────────────────────
    // cargo test sets cwd to workspace root, so "themes" resolves correctly.
    let hook_registry = Arc::new(HookRegistry::new());
    let engine = TemplateEngine::new(
        "themes",
        "sites",
        "default",
        base_url,
        hook_registry,
        pool.clone(),
    )
    .expect("TemplateEngine::new should succeed with themes/default/");

    // ── Build Tera context ────────────────────────────────────────────────────
    let mut ctx = ContextBuilder {
        site: SiteContext {
            name: "E2E Test Site".to_string(),
            description: "Integration test".to_string(),
            url: base_url.to_string(),
            language: "en".to_string(),
            theme: "default".to_string(),
            post_count: 1,
            page_count: 0,
        },
        request: RequestContext {
            url: format!("{}/{}", base_url, p.slug),
            path: format!("/{}", p.slug),
            query: HashMap::new(),
        },
        session: SessionContext {
            is_logged_in: false,
            user: None,
        },
        nav: NavContext::default(),
    }
    .into_tera_context();

    ctx.insert("post", &post_ctx);
    ctx.insert("prev_post", &Option::<PostContext>::None);
    ctx.insert("next_post", &Option::<PostContext>::None);
    ctx.insert("related_posts", &Vec::<PostContext>::new());

    // ── Render single.html ────────────────────────────────────────────────────
    let html = engine
        .render("single.html", &ctx)
        .expect("render single.html should succeed");

    // ── Assert expected content ───────────────────────────────────────────────
    assert!(
        html.contains(&post_ctx.title),
        "rendered HTML should contain post title"
    );
    assert!(
        html.contains(&format!("E2E content for post {id}")),
        "rendered HTML should contain post content text"
    );
    assert!(
        html.contains(&author.display_name),
        "rendered HTML should contain author display name"
    );

    // ── Cleanup ───────────────────────────────────────────────────────────────
    post::delete(&pool, p.id).await.ok();
    user::delete(&pool, author.id).await.ok();
}
