//! Integration tests for HTTP routes.
//!
//! Builds the real Axum `Router` (same `bootstrap::build` + `router::build`
//! sequence a live process uses — see `tests/common/mod.rs`) and drives it
//! directly via `tower::ServiceExt::oneshot`, so these exercise real
//! routing, middleware, and session wiring, not a hand-rolled
//! approximation of it.
//!
//! Requires a live PostgreSQL instance. Uses whatever `DATABASE_URL` points
//! at — same convention as `model_crud.rs`. Prefer a dedicated test
//! database over pointing this at a real dev/production one; these tests
//! don't currently write data, but nothing enforces that for tests added
//! here later.
//!
//! Run with:
//!   DATABASE_URL=postgres://user:pass@localhost/synaptic_signals \
//!     cargo test -p synaptic-core --test routes -- --include-ignored

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_home_route_200() {
    let app = common::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_post_route_404_for_nonexistent_slug() {
    let app = common::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/this-slug-does-not-exist")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_search_route_returns_200() {
    let app = common::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=test")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Empty results is still a 200 — searching is not itself an error.
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_admin_requires_auth_redirects_to_login() {
    let app = common::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        response.status() == StatusCode::FOUND || response.status() == StatusCode::SEE_OTHER,
        "expected a redirect, got {}",
        response.status()
    );
    let location = response
        .headers()
        .get("location")
        .expect("redirect response must have a Location header")
        .to_str()
        .unwrap();
    assert!(
        location.starts_with("/admin/login"),
        "expected redirect to /admin/login, got {location}"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_admin_login_post_bad_credentials() {
    let app = common::test_router().await;
    let body = "email=nonexistent-test-user%40example.com&password=definitely-wrong-password";
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header("content-type", "application/x-www-form-urlencoded")
                // middleware::csrf::same_origin requires a matching Host +
                // Origin pair on state-changing requests to protected paths.
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    // login_post re-renders the login page inline with an error on bad
    // credentials (no redirect) — see handlers/auth.rs.
    assert_eq!(response.status(), StatusCode::OK);

    // The real assertion: no authenticated session cookie got set. A
    // Set-Cookie header naming the admin session cookie here would mean a
    // failed login somehow still authenticated the request.
    let set_cookie_headers: Vec<&str> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        !set_cookie_headers
            .iter()
            .any(|c| c.starts_with("admin_session=") && !c.contains("admin_session=;")),
        "bad credentials must not set an admin_session cookie, got: {set_cookie_headers:?}"
    );
}
