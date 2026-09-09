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

/// Pull the `name=value` pair out of a login response's `Set-Cookie` header
/// for the given cookie name, dropping the `Path=`/`HttpOnly`/etc.
/// attributes — that's all a subsequent request's `Cookie` header needs.
fn extract_cookie(response: &axum::http::Response<Body>, cookie_name: &str) -> String {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with(&format!("{cookie_name}=")))
        .and_then(|c| c.split(';').next())
        .unwrap_or_else(|| panic!("no {cookie_name} cookie in response"))
        .to_string()
}

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

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_admin_login_escalating_delay_after_repeated_failures() {
    let app = common::test_router().await;
    // Unique per run — auth_security's failure tracker is a process-global
    // static shared by every test in this binary (routes.rs tests run
    // concurrently), so a fixed email would flake against whichever other
    // test happens to touch the same identity.
    let email = format!("escalating-delay-{}@example.com", uuid::Uuid::new_v4());
    let body = format!("email={email}&password=definitely-wrong-password");

    let attempt = |app: axum::Router, body: String| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
    };

    // The first few wrong-password attempts (FREE_ATTEMPTS in
    // middleware::auth_security) are free — no delay imposed yet.
    let first = attempt(app.clone(), body.clone()).await;
    assert_eq!(
        first.status(),
        StatusCode::OK,
        "an early failed attempt should just re-render the login page, not be throttled"
    );

    // Enough further attempts to cross the free threshold and trigger a delay.
    let mut last_status = first.status();
    for _ in 0..5 {
        last_status = attempt(app.clone(), body.clone()).await.status();
        if last_status == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
    }
    assert_eq!(
        last_status,
        StatusCode::TOO_MANY_REQUESTS,
        "repeated failures against the same identity should eventually trigger the escalating delay"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_sign_out_other_devices_invalidates_other_admin_sessions() {
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test user setup");

    // Throwaway super_admin — bypasses site_users entirely, so /admin/profile
    // is reachable without also seeding a site membership row.
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("sign-out-test-{unique}@example.com");
    let password = "Verify12345Pass!";
    let created = user::create(
        &pool,
        &CreateUser {
            username: format!("sotest{}", &unique[..12]),
            email: email.clone(),
            display_name: "Sign Out Test".to_string(),
            password: password.to_string(),
            role: UserRole::SuperAdmin,
        },
    )
    .await
    .expect("failed to create test user");

    let app = common::test_router().await;
    let login_body = format!("email={email}&password={password}");

    let login = |app: axum::Router, body: String| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
    };

    // Two independent logins — two independent sessions, simulating two devices.
    let response_a = login(app.clone(), login_body.clone()).await;
    let cookie_a = extract_cookie(&response_a, "admin_session");
    let response_b = login(app.clone(), login_body.clone()).await;
    let cookie_b = extract_cookie(&response_b, "admin_session");
    assert_ne!(cookie_a, cookie_b, "each login must mint its own session");

    let get_profile = |app: axum::Router, cookie: String| async move {
        app.oneshot(
            Request::builder()
                .uri("/admin/profile")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    };

    // Sanity: both sessions currently work.
    assert_eq!(
        get_profile(app.clone(), cookie_a.clone()).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get_profile(app.clone(), cookie_b.clone()).await.status(),
        StatusCode::OK
    );

    // From device A, sign out every other session.
    let sign_out_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/profile/sign-out-other-devices")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie_a.clone())
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        sign_out_response.status() == StatusCode::FOUND
            || sign_out_response.status() == StatusCode::SEE_OTHER,
        "expected a redirect, got {}",
        sign_out_response.status()
    );

    // Device A's own session was kept alive by re-inserting the fresh
    // credential_version into it — this must NOT be the boilerplate
    // "no-op'd because credential_version already matched" case, since
    // device A's request is what triggered the bump in the first place.
    assert_eq!(
        get_profile(app.clone(), cookie_a.clone()).await.status(),
        StatusCode::OK,
        "the session that requested the sign-out must stay logged in"
    );

    // Device B is booted — its stored credential_version no longer matches.
    let device_b_response = get_profile(app.clone(), cookie_b.clone()).await;
    assert!(
        device_b_response.status() == StatusCode::FOUND
            || device_b_response.status() == StatusCode::SEE_OTHER,
        "expected device B to be redirected to login, got {}",
        device_b_response.status()
    );
    let location = device_b_response
        .headers()
        .get("location")
        .expect("redirect response must have a Location header")
        .to_str()
        .unwrap();
    assert!(
        location.starts_with("/admin/login"),
        "expected device B's stale session to redirect to /admin/login, got {location}"
    );

    let _ = user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_account_dashboard_shows_site_logo() {
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test user setup");

    let app = common::test_router().await; // ensures the "localhost" test site exists
    let site = synaptic_core::models::site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");
    let unique = uuid::Uuid::new_v4().simple().to_string();

    // Give this site its own uploaded logo — `detect_site_admin_logo`
    // (core/src/app_state.rs) looks for
    // admin/static/branding/{site_id}/logo.{svg,png,webp} relative to the
    // process's current working directory, which for `cargo test` is this
    // crate's manifest dir (`core/`), not the workspace root — see
    // `tests/common::workspace_themes_dir`'s doc comment for the identical
    // gotcha. Writing directly to that relative path (rather than relying on
    // the global agency-wide logo, which lives outside `core/` and so is
    // invisible to this process's CWD-relative lookup) sidesteps that and
    // also exercises the actual scenario a site owner hits: their own
    // uploaded logo, not the installation-wide one.
    let logo_dir = std::path::Path::new("admin/static/branding").join(site.id.to_string());
    std::fs::create_dir_all(&logo_dir).expect("failed to create test logo dir");
    let logo_path = logo_dir.join("logo.svg");
    std::fs::write(&logo_path, b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>")
        .expect("failed to write test logo file");

    // Throwaway subscriber — the one actually visiting /account.
    let email = format!("logo-test-sub-{unique}@example.com");
    let password = "Verify12345Pass!";
    let subscriber = user::create(
        &pool,
        &CreateUser {
            username: format!("logosub{}", &unique[8..16]),
            email: email.clone(),
            display_name: "Logo Test Subscriber".to_string(),
            password: password.to_string(),
            role: UserRole::Subscriber,
        },
    )
    .await
    .expect("failed to create test subscriber");
    // /login rejects a subscriber with no site_users row for the resolved
    // site ("Your account does not have access to this site.") — grant one.
    synaptic_core::models::site_user::add(
        &pool,
        site.id,
        subscriber.id,
        synaptic_core::models::site_user::SiteRole::Subscriber,
        None,
        false,
    )
    .await
    .expect("failed to grant site membership");

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(format!("email={email}&password={password}")))
                .unwrap(),
        )
        .await
        .unwrap();
    // Unlike the admin session (cookie name "admin_session"), the account
    // session cookie is just "session" — see bootstrap::build's
    // account_session_layer.
    let cookie = extract_cookie(&login_response, "session");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/account")
                .header("host", "localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let expected_src = format!("/admin/static/branding/{}/logo.svg", site.id);
    let brand_snippet = body
        .find("class=\"brand\"")
        .map(|i| body[i..(i + 200).min(body.len())].to_string())
        .unwrap_or_else(|| "<no class=\"brand\" found at all>".to_string());

    let _ = user::delete(&pool, subscriber.id).await;
    let _ = std::fs::remove_dir_all(&logo_dir);

    assert!(
        body.contains(r#"class="brand-logo""#) && body.contains(&expected_src),
        "expected /account sidebar to render this site's logo <img src=\"{expected_src}\">, got: {brand_snippet}"
    );
}
