//! Integration tests for model CRUD operations.
//!
//! All tests require a live PostgreSQL instance.
//!
//! To run these tests, a `[lib]` target must first be added to core/Cargo.toml
//! so that the crate is importable from this integration test harness.
//!
//! Run with:
//!   DATABASE_URL=postgres://user:pass@localhost/synaptic_test \
//!     cargo test -p synaptic-core --test model_crud -- --include-ignored

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_create_and_get() {
    // Steps:
    //   1. Connect to DATABASE_URL
    //   2. Create a user via user::create()
    //   3. Create a post via post::create() with that user as author
    //   4. Fetch via post::get_by_id() and assert title/slug match
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_update() {
    // Steps:
    //   1. Create a post (Draft)
    //   2. Call post::update() with a new title
    //   3. Assert the returned post has the updated title
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_delete() {
    // Steps:
    //   1. Create a post
    //   2. Call post::delete()
    //   3. Assert post::get_by_id() returns Err (NotFound)
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_list_filter_published_only() {
    // Steps:
    //   1. Create one Published post and one Draft post for the same author
    //   2. Call post::list() with status=Published, author_id filter
    //   3. Assert only the Published post is in results
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_user_create_and_auth() {
    // Steps:
    //   1. Create a user with a known plaintext password
    //   2. Assert user.verify_password(correct) == true
    //   3. Assert user.verify_password(wrong)   == false
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_taxonomy_attach_detach() {
    // Steps:
    //   1. Create a post and a taxonomy term
    //   2. taxonomy::attach(pool, post_id, term_id)
    //   3. taxonomy::for_post(pool, post_id) returns the term
    //   4. taxonomy::detach(pool, post_id, term_id)
    //   5. taxonomy::for_post(pool, post_id) returns empty
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL: set DATABASE_URL and run cargo test -- --include-ignored"]
async fn test_post_meta_set_get() {
    // Steps:
    //   1. Create a post
    //   2. post::set_meta(pool, post_id, "seo_title", "Custom SEO Title")
    //   3. post::get_meta(pool, post_id) returns map with "seo_title" => "Custom SEO Title"
    todo!("implement after lib target is added to core/Cargo.toml")
}
