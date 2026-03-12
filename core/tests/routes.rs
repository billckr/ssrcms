//! Integration tests for HTTP routes.
//!
//! All tests require a live PostgreSQL instance, seeded data, and configured
//! themes/plugins directories.
//!
//! To run these tests, a `[lib]` target must first be added to core/Cargo.toml
//! so that the router can be constructed from this integration test harness.
//!
//! Run with:
//!   DATABASE_URL=postgres://user:pass@localhost/synaptic_test \
//!     cargo test -p synaptic-core --test routes -- --include-ignored

#[tokio::test]
#[ignore = "requires live PostgreSQL and seeded themes/plugins dirs"]
async fn test_home_route_200() {
    // Steps:
    //   1. Build AppState from test AppConfig (DATABASE_URL env var)
    //   2. Construct the Axum router via router::build()
    //   3. Send GET / using axum::test helpers (tower::ServiceExt::oneshot)
    //   4. Assert response.status() == 200
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL and seeded themes/plugins dirs"]
async fn test_post_route_404_for_nonexistent_slug() {
    // Steps:
    //   1. Build the Axum router
    //   2. Send GET /this-slug-does-not-exist
    //   3. Assert response.status() == 404
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL and seeded themes/plugins dirs"]
async fn test_search_route_returns_200() {
    // Steps:
    //   1. Build the Axum router
    //   2. Send GET /search?q=test
    //   3. Assert response.status() == 200 (empty results is still OK)
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL and seeded themes/plugins dirs"]
async fn test_admin_requires_auth_redirects_to_login() {
    // Steps:
    //   1. Build the Axum router (no session cookie)
    //   2. Send GET /admin
    //   3. Assert response.status() is 302 or 303
    //   4. Assert Location header points to /admin/login
    todo!("implement after lib target is added to core/Cargo.toml")
}

#[tokio::test]
#[ignore = "requires live PostgreSQL and seeded themes/plugins dirs"]
async fn test_admin_login_post_bad_credentials() {
    // Steps:
    //   1. Build the Axum router
    //   2. POST /admin/login with bad credentials (form body)
    //   3. Assert response does NOT set an authenticated session cookie
    //   4. Assert response redirects back to /admin/login
    todo!("implement after lib target is added to core/Cargo.toml")
}
