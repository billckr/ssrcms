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
    std::fs::write(
        &logo_path,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    )
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

/// Serializes `enable_locale_for_test`'s read-modify-write across every test
/// in this binary. `routes.rs` tests run concurrently and all share the one
/// "localhost" site, so two tests reading the enabled-locales list at the
/// same time and each writing back their own addition would silently drop
/// whichever one wrote first — this is exactly what caused
/// `test_locale_prefixed_url_shows_translated_content` to intermittently
/// fail in CI ("expected an hreflang alternate link for '<locale>'") once
/// enough tests ran concurrently to make the race likely. A process-wide
/// lock (all `#[tokio::test]`s share one process) turns the read-modify-write
/// back into an atomic-enough critical section without touching the
/// production `site_locale` model, which has no reason to know about this
/// test-only sharing problem.
static LOCALE_ENABLE_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

/// Adds `locale` to the "localhost" test site's enabled-locales list without
/// clobbering any other locale a concurrently-running test may have already
/// enabled — see `LOCALE_ENABLE_LOCK`.
async fn enable_locale_for_test(pool: &sqlx::PgPool, site_id: uuid::Uuid) -> String {
    use synaptic_core::models::site_locale;
    let _guard = LOCALE_ENABLE_LOCK.lock().await;
    let locale = format!("t{}", &uuid::Uuid::new_v4().simple().to_string()[..6]);
    let mut codes = site_locale::enabled_locales_for_site(pool, site_id).await;
    codes.push(locale.clone());
    site_locale::set_enabled_locales(pool, site_id, &codes)
        .await
        .expect("failed to enable test locale");
    locale
}

/// Same as `enable_locale_for_test`, but for routes (like the admin
/// translate action) that additionally require the code to be a real,
/// recognized language (`utils::locales::display_name`) rather than any
/// arbitrary enabled prefix. Idempotent — safe even if another test already
/// enabled the same real code on this shared site.
async fn enable_real_locale_for_test(pool: &sqlx::PgPool, site_id: uuid::Uuid, code: &str) {
    use synaptic_core::models::site_locale;
    let _guard = LOCALE_ENABLE_LOCK.lock().await;
    let mut codes = site_locale::enabled_locales_for_site(pool, site_id).await;
    if !codes.iter().any(|c| c == code) {
        codes.push(code.to_string());
        site_locale::set_enabled_locales(pool, site_id, &codes)
            .await
            .expect("failed to enable test locale");
    }
}

/// Creates a throwaway published post directly via the model (no admin
/// login/editor round trip needed for these routing tests).
async fn create_test_post(
    pool: &sqlx::PgPool,
    site_id: uuid::Uuid,
    slug: &str,
) -> synaptic_core::models::post::Post {
    use synaptic_core::models::post::{CreatePost, PostStatus, PostType};
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let author = user::create(
        pool,
        &CreateUser {
            username: format!("locpost{}", &unique[..12]),
            email: format!("locale-test-{unique}@example.com"),
            display_name: "Locale Test Author".to_string(),
            password: "Verify12345Pass!".to_string(),
            role: UserRole::Author,
        },
    )
    .await
    .expect("failed to create test author");

    synaptic_core::models::post::create(
        pool,
        &CreatePost {
            site_id: Some(site_id),
            title: "Hello World".to_string(),
            slug: Some(slug.to_string()),
            content: "<p>Original content.</p>".to_string(),
            content_format: Some("html".to_string()),
            excerpt: None,
            status: PostStatus::Published,
            post_type: PostType::Post,
            author_id: author.id,
            featured_image_id: None,
            published_at: Some(chrono::Utc::now()),
            template: None,
            post_password_hash: None,
            comments_enabled: true,
            parent_id: None,
            sources: vec![],
            sources_public: false,
        },
    )
    .await
    .expect("failed to create test post")
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_locale_prefixed_url_shows_translated_content() {
    use synaptic_core::models::post_translation;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test setup");

    let app = common::test_router().await; // ensures the "localhost" test site exists
    let site = synaptic_core::models::site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let slug = format!("locale-test-post-{}", &unique[..8]);
    let post = create_test_post(&pool, site.id, &slug).await;
    let locale = enable_locale_for_test(&pool, site.id).await;

    post_translation::upsert(
        &pool,
        post.id,
        &locale,
        "Hola Mundo",
        Some("Un resumen"),
        "<p>Contenido traducido.</p>",
        post.updated_at,
    )
    .await
    .expect("failed to save test translation");

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/{locale}/{slug}"))
                .header("host", "localhost")
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

    let _ = synaptic_core::models::post::delete(&pool, post.id).await;

    assert!(
        body.contains("Hola Mundo"),
        "expected the translated title in the response body"
    );
    assert!(
        body.contains("Contenido traducido"),
        "expected the translated content in the response body"
    );
    assert!(
        body.contains(&format!(r#"hreflang="{locale}""#)),
        "expected an hreflang alternate link for '{locale}'"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_locale_prefixed_url_falls_back_to_original_without_translation() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test setup");

    let app = common::test_router().await;
    let site = synaptic_core::models::site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let slug = format!("locale-fallback-post-{}", &unique[..8]);
    let post = create_test_post(&pool, site.id, &slug).await;
    let locale = enable_locale_for_test(&pool, site.id).await;
    // Deliberately no post_translation row for this (post, locale) pair.

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/{locale}/{slug}"))
                .header("host", "localhost")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "an enabled locale with no translation for this post should still render, not 404"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    let _ = synaptic_core::models::post::delete(&pool, post.id).await;

    assert!(
        body.contains("Hello World") && body.contains("Original content."),
        "expected the original (untranslated) content as a silent fallback"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_locale_prefix_not_enabled_404s_normally() {
    let app = common::test_router().await;

    // A slug that matches no post at all — with "xx" not an enabled locale,
    // this must 404 exactly like any other unmatched two-segment path would
    // (a slug that *does* match a post would instead 301 to its canonical
    // permalink via `try_post_permalink`'s existing decorative-segment
    // self-correction, which is a separate, pre-existing behavior this test
    // isn't targeting).
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let missing_slug = format!("locale-notenabled-missing-{}", &unique[..8]);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/xx/{missing_slug}"))
                .header("host", "localhost")
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a non-enabled locale-shaped prefix over a nonexistent path should 404 exactly as any other unmatched path would"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_ai_provider_model_discovery_retains_saved_key_when_edit_form_leaves_it_blank() {
    use synaptic_core::models::ai_provider::{self, AiProviderConfig};
    use synaptic_core::models::site;
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test setup");

    let app = common::test_router().await; // ensures the "localhost" test site exists
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    // `test_router()`'s AppConfig always uses this fixed secret_key (see
    // common::test_config) — matching it here is what lets the route
    // handler's own decrypt_config call (using the app's real secret_key)
    // successfully decrypt this provider back out.
    let secret_key = "test-secret-key-not-for-production-use-only";
    let saved_config = AiProviderConfig::Anthropic {
        api_key: "sk-ant-adhoc-saved-key".to_string(),
        model_name: "claude-old-model".to_string(),
    };
    let provider = ai_provider::create(
        &pool,
        test_site.id,
        "Adhoc Retain-Key Test",
        &saved_config,
        secret_key,
    )
    .await
    .expect("failed to seed test AI provider");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("retain-key-test-{unique}@example.com");
    let password = "Verify12345Pass!";
    let admin_user = user::create(
        &pool,
        &CreateUser {
            username: format!("retainkey{}", &unique[..12]),
            email: email.clone(),
            display_name: "Retain Key Test".to_string(),
            password: password.to_string(),
            role: UserRole::SuperAdmin,
        },
    )
    .await
    .expect("failed to create test admin");

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(format!("email={email}&password={password}")))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    // The Edit form's request when a user leaves the API key field blank and
    // just picks a different model from the (already-loaded) dropdown —
    // exactly the "Connect and load models" click on an existing provider.
    let body = "label=Adhoc+Retain-Key+Test&provider_type=anthropic&anthropic_api_key=&anthropic_model_choice=claude-different-model&anthropic_model_name=";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/sites/{}/ai-providers/{}/models",
                    test_site.id, provider.id
                ))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_body = String::from_utf8(response_body.to_vec()).unwrap();

    let _ = ai_provider::delete(&pool, provider.id, test_site.id).await;
    let _ = user::delete(&pool, admin_user.id).await;

    // "sk-ant-adhoc-saved-key" isn't a real Anthropic key, so the actual
    // upstream call is expected to fail — that's fine and not what this test
    // checks. What it rules out is the specific failure mode of the saved
    // key NOT being reused at all: config_from_form only ever returns "Enter
    // an API key before loading models." when the blank form field has
    // nothing saved to fall back to, which would mean editing a provider
    // silently lost its stored key.
    assert!(
        !response_body.contains("Enter an API key before loading models"),
        "blank API key field on an edit form must fall back to the saved encrypted key, not demand a fresh one — got {status}: {response_body}"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_ai_provider_save_retains_saved_key_and_updates_model_when_edit_form_leaves_key_blank(
) {
    use synaptic_core::models::ai_provider::{self, AiProviderConfig};
    use synaptic_core::models::site;
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url)
        .await
        .expect("failed to connect for test setup");

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    let secret_key = "test-secret-key-not-for-production-use-only";
    let saved_config = AiProviderConfig::Anthropic {
        api_key: "sk-ant-adhoc-saved-key-2".to_string(),
        model_name: "claude-old-model-2".to_string(),
    };
    let provider = ai_provider::create(
        &pool,
        test_site.id,
        "Adhoc Save Retain-Key Test",
        &saved_config,
        secret_key,
    )
    .await
    .expect("failed to seed test AI provider");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("save-retain-key-test-{unique}@example.com");
    let password = "Verify12345Pass!";
    let admin_user = user::create(
        &pool,
        &CreateUser {
            username: format!("saveretain{}", &unique[..10]),
            email: email.clone(),
            display_name: "Save Retain Key Test".to_string(),
            password: password.to_string(),
            role: UserRole::SuperAdmin,
        },
    )
    .await
    .expect("failed to create test admin");

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(format!("email={email}&password={password}")))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    // The actual "Save Provider" submit — blank key, a newly-picked model
    // (as if loaded via the dropdown and selected), same label.
    let body = "label=Adhoc+Save+Retain-Key+Test&provider_type=anthropic&anthropic_api_key=&anthropic_model_choice=claude-new-model-2&anthropic_model_name=";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/sites/{}/ai-providers/{}",
                    test_site.id, provider.id
                ))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();

    let row_after = ai_provider::get_by_id(&pool, provider.id)
        .await
        .expect("failed to refetch provider")
        .expect("provider must still exist after update");
    let decrypted_after = ai_provider::decrypt_config(secret_key, &row_after);

    let _ = ai_provider::delete(&pool, provider.id, test_site.id).await;
    let _ = user::delete(&pool, admin_user.id).await;

    assert!(
        status == StatusCode::FOUND || status == StatusCode::SEE_OTHER,
        "expected the save to redirect back to settings, got {status}"
    );
    match decrypted_after {
        Some(AiProviderConfig::Anthropic {
            api_key,
            model_name,
        }) => {
            assert_eq!(
                api_key, "sk-ant-adhoc-saved-key-2",
                "leaving the API key field blank on save must keep the originally saved key, not blank it out or drop the provider into an unusable state"
            );
            assert_eq!(
                model_name, "claude-new-model-2",
                "the newly-selected model from the dropdown must be the one actually saved"
            );
        }
        other => panic!("expected a decryptable Anthropic config after save, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_openai_compatible_provider_creation_rejected_for_site_scoped_admin() {
    use synaptic_core::models::ai_provider;
    use synaptic_core::models::site;
    use synaptic_core::models::site_user::{self, SiteRole};
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    // A site-scoped admin — enough to pass `require_site_manager` for this
    // site, but not `is_global_admin` — is exactly who local/self-hosted
    // model providers must be withheld from, since they don't control the
    // underlying server's network the way a super admin does.
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("site-scoped-local-ai-{unique}@example.com");
    let password = "Verify12345Pass!";
    let admin_user = user::create(
        &pool,
        &CreateUser {
            username: format!("localaisite{}", &unique[..9]),
            email: email.clone(),
            display_name: "Site-Scoped Local AI Test".to_string(),
            password: password.to_string(),
            role: UserRole::SiteAdmin,
        },
    )
    .await
    .expect("failed to create test admin");
    site_user::add(&pool, test_site.id, admin_user.id, SiteRole::Admin, None, false)
        .await
        .expect("failed to grant site-admin role");

    let login_response = app
        .clone()
        .oneshot(login_request(format!("email={email}&password={password}")))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    let body = "label=Local+Ollama&provider_type=openai_compatible&openai_compatible_base_url=http%3A%2F%2F127.0.0.1%3A11434%2Fv1&openai_compatible_api_key=&openai_compatible_model_choice=&openai_compatible_model_name=llama3";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/sites/{}/ai-providers", test_site.id))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_body = String::from_utf8(response_body.to_vec()).unwrap();

    let created = ai_provider::label_exists_for_site(&pool, test_site.id, "Local Ollama", None)
        .await
        .unwrap_or(false);
    let _ = user::delete(&pool, admin_user.id).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a site-scoped (non-global) admin must not be able to configure a custom-base-URL provider, got {status}: {response_body}"
    );
    assert!(
        response_body.contains("super admin"),
        "expected the local-model rejection message, got: {response_body}"
    );
    assert!(
        !created,
        "the rejected provider must not have been persisted"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_openai_compatible_provider_creation_allowed_for_global_admin() {
    use synaptic_core::models::ai_provider;
    use synaptic_core::models::site;
    use synaptic_core::models::user;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");
    let (admin_user, password) = create_mfa_test_admin(&pool, "localaiglobal").await;

    let login_response = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            admin_user.email
        )))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    let body = "label=Local+Ollama+Global&provider_type=openai_compatible&openai_compatible_base_url=http%3A%2F%2F127.0.0.1%3A11434%2Fv1&openai_compatible_api_key=&openai_compatible_model_choice=&openai_compatible_model_name=llama3";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/sites/{}/ai-providers", test_site.id))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();

    let created_row = ai_provider::list_for_site(&pool, test_site.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|row| row.label == "Local Ollama Global");
    if let Some(row) = &created_row {
        let _ = ai_provider::delete(&pool, row.id, test_site.id).await;
    }
    let _ = user::delete(&pool, admin_user.id).await;

    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "a global admin must be allowed to configure a custom-base-URL provider, got {status}"
    );
    assert!(
        created_row.is_some(),
        "the provider row should be persisted even though verification against a real Ollama server isn't attempted in this test"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_concurrent_ai_provider_creation_with_same_label_never_creates_two_rows() {
    // Regression test for a double-submit bug: a slow provider-verification
    // call made the "Add Provider" click look like nothing happened, so the
    // admin clicked again — creating a second provider row. The
    // application-level `label_exists_for_site` pre-check is a plain
    // SELECT-then-INSERT, so two genuinely concurrent submissions can both
    // pass the "not taken" check before either INSERT commits. This fires
    // both at once (via `tokio::join!`, not sequentially) so the check is
    // actually racy, and asserts the database's own unique constraint on
    // (site_id, label) — migrations/0007_ai_provider_label_unique.sql — is
    // what actually stops a second row, with the loser turned into a
    // friendly redirect rather than a 500 (see `is_unique_violation` in
    // `handlers::admin::ai_providers::create`).
    use synaptic_core::models::ai_provider;
    use synaptic_core::models::site;
    use synaptic_core::models::user;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");
    let (admin_user, password) = create_mfa_test_admin(&pool, "aidupe").await;

    let login_response = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            admin_user.email
        )))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let label = format!("Racing Provider {}", &unique[..8]);
    let make_request = |model_suffix: &str| {
        let body = format!(
            "label={}&provider_type=anthropic&anthropic_api_key=sk-ant-race&anthropic_model_choice=&anthropic_model_name=claude-race-{model_suffix}",
            label.replace(' ', "+"),
        );
        Request::builder()
            .method("POST")
            .uri(format!("/admin/sites/{}/ai-providers", test_site.id))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("host", "localhost")
            .header("origin", "http://localhost")
            .header("cookie", cookie.clone())
            .extension(common::connect_info())
            .body(Body::from(body))
            .unwrap()
    };

    let (response_a, response_b) = tokio::join!(
        app.clone().oneshot(make_request("a")),
        app.clone().oneshot(make_request("b")),
    );
    let status_a = response_a.unwrap().status();
    let status_b = response_b.unwrap().status();

    let matching_rows: Vec<_> = ai_provider::list_for_site(&pool, test_site.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row.label == label)
        .collect();

    for row in &matching_rows {
        let _ = ai_provider::delete(&pool, row.id, test_site.id).await;
    }
    let _ = user::delete(&pool, admin_user.id).await;

    for status in [status_a, status_b] {
        assert!(
            status == StatusCode::FOUND || status == StatusCode::SEE_OTHER,
            "neither racing submission should ever produce a server error — got {status}"
        );
    }
    assert_eq!(
        matching_rows.len(),
        1,
        "exactly one row must exist for this label no matter how the two concurrent submissions raced, got {}",
        matching_rows.len()
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_translate_post_rejected_for_site_scoped_admin_using_local_model_provider() {
    // A site-scoped admin can't configure or test a local/self-hosted
    // (openai_compatible) provider (see the `_rejected_for_site_scoped_admin`
    // creation test above), but that restriction is pointless if the same
    // admin can still pick an already-configured local-model provider from
    // the Translate dropdown and have the server call it on their behalf —
    // that's the actual SSRF the restriction exists to prevent. This
    // exercises the real translate route end-to-end with a provider a
    // global admin already set up, from a site-scoped admin's session.
    use synaptic_core::models::ai_provider::{self, AiProviderConfig};
    use synaptic_core::models::site;
    use synaptic_core::models::site_user::{self, SiteRole};
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");
    let locale = "es".to_string();
    enable_real_locale_for_test(&pool, test_site.id, &locale).await;

    let secret_key = "test-secret-key-not-for-production-use-only";
    let provider = ai_provider::create(
        &pool,
        test_site.id,
        "Local Model For Translate Test",
        &AiProviderConfig::OpenaiCompatible {
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            api_key: String::new(),
            model_name: "llama3".to_string(),
        },
        secret_key,
    )
    .await
    .expect("failed to seed local-model provider");
    ai_provider::mark_verified(&pool, provider.id)
        .await
        .expect("failed to mark provider verified");

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let slug = format!("translate-local-model-test-{}", &unique[..8]);
    let post = create_test_post(&pool, test_site.id, &slug).await;

    let email = format!("site-scoped-translate-{unique}@example.com");
    let password = "Verify12345Pass!";
    let admin_user = user::create(
        &pool,
        &CreateUser {
            username: format!("localaitr{}", &unique[..9]),
            email: email.clone(),
            display_name: "Site-Scoped Translate Test".to_string(),
            password: password.to_string(),
            role: UserRole::SiteAdmin,
        },
    )
    .await
    .expect("failed to create test admin");
    site_user::add(&pool, test_site.id, admin_user.id, SiteRole::Admin, None, false)
        .await
        .expect("failed to grant site-admin role");

    let login_response = app
        .clone()
        .oneshot(login_request(format!("email={email}&password={password}")))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    let body = format!("locale={locale}&provider_id={}", provider.id);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/posts/{}/translate", post.id))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_body = String::from_utf8(response_body.to_vec()).unwrap();

    let translation_created = synaptic_core::models::post_translation::get(&pool, post.id, &locale)
        .await
        .unwrap_or_default()
        .is_some();

    let _ = ai_provider::delete(&pool, provider.id, test_site.id).await;
    let _ = synaptic_core::models::post::delete(&pool, post.id).await;
    let _ = user::delete(&pool, admin_user.id).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a site-scoped admin must not be able to translate using a local-model provider, got {status}: {response_body}"
    );
    assert!(
        response_body.contains("super admin"),
        "expected the local-model rejection message, got: {response_body}"
    );
    assert!(
        !translation_created,
        "no translation (and no outbound call to the local model) should have happened"
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_local_model_provider_base_url_hidden_from_site_scoped_admin_settings_view() {
    // The base URL of a local/self-hosted (openai_compatible) provider can
    // point at internal network addresses. Restricting who can configure it
    // is pointless if a site-scoped admin can still read it off the Site
    // Settings page (in the provider's hint text and the edit form's
    // placeholder) — so the row itself must not be rendered for them at all.
    use synaptic_core::models::ai_provider::{self, AiProviderConfig};
    use synaptic_core::models::site;
    use synaptic_core::models::site_user::{self, SiteRole};
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();

    let app = common::test_router().await;
    let test_site = site::get_by_hostname(&pool, "localhost")
        .await
        .expect("test site must exist");

    let secret_key = "test-secret-key-not-for-production-use-only";
    let distinctive_base_url = "http://10.88.7.2:11434/v1";
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let label = format!("Local Model Visibility Test {}", &unique[..8]);
    let provider = ai_provider::create(
        &pool,
        test_site.id,
        &label,
        &AiProviderConfig::OpenaiCompatible {
            base_url: distinctive_base_url.to_string(),
            api_key: String::new(),
            model_name: "llama3".to_string(),
        },
        secret_key,
    )
    .await
    .expect("failed to seed local-model provider");

    let email = format!("site-scoped-settings-view-{unique}@example.com");
    let password = "Verify12345Pass!";
    let admin_user = user::create(
        &pool,
        &CreateUser {
            username: format!("localaisv{}", &unique[..9]),
            email: email.clone(),
            display_name: "Site-Scoped Settings View Test".to_string(),
            password: password.to_string(),
            role: UserRole::SiteAdmin,
        },
    )
    .await
    .expect("failed to create test admin");
    site_user::add(&pool, test_site.id, admin_user.id, SiteRole::Admin, None, false)
        .await
        .expect("failed to grant site-admin role");

    let login_response = app
        .clone()
        .oneshot(login_request(format!("email={email}&password={password}")))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_response, "admin_session");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/admin/sites/{}/settings?tab=ai-translation",
                    test_site.id
                ))
                .header("host", "localhost")
                .header("cookie", cookie)
                .extension(common::connect_info())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    let _ = ai_provider::delete(&pool, provider.id, test_site.id).await;
    let _ = user::delete(&pool, admin_user.id).await;

    assert_eq!(status, StatusCode::OK, "settings page must render normally");
    assert!(
        !body.contains(distinctive_base_url),
        "a site-scoped admin's settings page must never render a local-model provider's base URL"
    );
    assert!(
        !body.contains(&label),
        "a site-scoped admin's settings page must not even show the local-model provider's row"
    );
}

// ── TOTP MFA for staff logins ────────────────────────────────────────────

/// Creates a throwaway super_admin (bypasses site_users entirely, same
/// reasoning as `test_sign_out_other_devices_invalidates_other_admin_sessions`
/// above) and returns it plus its plaintext password.
async fn create_mfa_test_admin(
    pool: &sqlx::PgPool,
    prefix: &str,
) -> (synaptic_core::models::user::User, String) {
    use synaptic_core::models::user::{self, CreateUser, UserRole};
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let password = "Verify12345Pass!".to_string();
    let created = user::create(
        pool,
        &CreateUser {
            username: format!("{prefix}{}", &unique[..12]),
            email: format!("{prefix}-{unique}@example.com"),
            display_name: format!("{prefix} Test"),
            password: password.clone(),
            role: UserRole::SuperAdmin,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("failed to create test user: {e:?}"));
    (created, password)
}

fn login_request(body: String) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/admin/login")
        .header("content-type", "application/x-www-form-urlencoded")
        // middleware::csrf::same_origin requires a matching Host + Origin
        // pair on state-changing requests to protected paths.
        .header("host", "localhost")
        .header("origin", "http://localhost")
        .extension(common::connect_info())
        .body(Body::from(body))
        .unwrap()
}

fn mfa_verify_request(cookie: &str, body: String) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/admin/login/mfa")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("host", "localhost")
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .extension(common::connect_info())
        .body(Body::from(body))
        .unwrap()
}

fn authenticated_post_request(uri: &str, cookie: &str, body: String) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("host", "localhost")
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .extension(common::connect_info())
        .body(Body::from(body))
        .unwrap()
}

fn authenticated_get_request(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("cookie", cookie)
        .extension(common::connect_info())
        .body(Body::empty())
        .unwrap()
}

fn redirect_location(response: &axum::http::Response<Body>) -> String {
    assert!(
        response.status() == StatusCode::FOUND || response.status() == StatusCode::SEE_OTHER,
        "expected a redirect, got {}",
        response.status()
    );
    response
        .headers()
        .get("location")
        .expect("redirect response must have a Location header")
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_admin_login_unaffected_when_mfa_not_enabled() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfaoff").await;

    let app = common::test_router().await;
    let response = app
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();

    assert_eq!(
        redirect_location(&response),
        "/admin",
        "an account with no MFA enrolled must log in directly, with no second-factor step"
    );

    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_totp_enrollment_and_login_flow() {
    use synaptic_core::models::user_totp;
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfaenr").await;

    let app = common::test_router().await;

    // Log in once (no MFA yet) to get an authenticated session to enroll from.
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    assert_eq!(redirect_location(&login_resp), "/admin");
    let cookie = extract_cookie(&login_resp, "admin_session");

    // Start enrollment.
    let start_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/setup/start",
            &cookie,
            format!("current_password={password}"),
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&start_resp), "/admin/profile/2fa/setup");

    // Read the pending secret directly (not scraped from HTML) to compute a
    // valid code, matching how a real authenticator app would.
    let secret = user_totp::pending_secret(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap()
        .expect("enrollment should have created a pending secret");
    let code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();

    // Confirm enrollment.
    let confirm_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/setup/confirm",
            &cookie,
            format!("code={code}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        confirm_resp.status(),
        StatusCode::OK,
        "confirming with the right code should show the recovery codes page"
    );
    assert!(user_totp::is_enabled(&pool, created.id).await.unwrap());

    // A fresh login now must stop at /admin/login/mfa, not /admin.
    let login2_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    assert_eq!(redirect_location(&login2_resp), "/admin/login/mfa");
    let pending_cookie = extract_cookie(&login2_resp, "admin_session");

    // /admin/profile is not reachable yet on the pending-MFA cookie.
    let profile_resp = app
        .clone()
        .oneshot(authenticated_get_request(
            "/admin/profile",
            &pending_cookie,
        ))
        .await
        .unwrap();
    assert!(
        profile_resp.status() == StatusCode::FOUND || profile_resp.status() == StatusCode::SEE_OTHER,
        "a pending-MFA session must not satisfy the AdminUser extractor, got {}",
        profile_resp.status()
    );

    // Submit the correct code to complete login.
    let login_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    let verify_resp = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie,
            format!("code={login_code}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        redirect_location(&verify_resp),
        "/admin",
        "the correct second factor must complete the login"
    );
    let final_cookie = extract_cookie(&verify_resp, "admin_session");

    let profile_resp2 = app
        .clone()
        .oneshot(authenticated_get_request("/admin/profile", &final_cookie))
        .await
        .unwrap();
    assert_eq!(
        profile_resp2.status(),
        StatusCode::OK,
        "the fully-authenticated cookie must now reach /admin/profile"
    );

    let _ = synaptic_core::models::mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_totp_login_rejects_wrong_code_and_blocks_replay() {
    use synaptic_core::models::user_totp;
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfarep").await;

    // Enroll directly via the model — the HTTP enrollment path is already
    // covered by test_totp_enrollment_and_login_flow above.
    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());

    let app = common::test_router().await;

    // Wrong code is rejected — session stays pending, not authenticated.
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie = extract_cookie(&login_resp, "admin_session");
    let wrong_resp = app
        .clone()
        .oneshot(mfa_verify_request(&pending_cookie, "code=000000".to_string()))
        .await
        .unwrap();
    assert_eq!(
        wrong_resp.status(),
        StatusCode::OK,
        "a wrong code re-renders the MFA form, it does not redirect"
    );
    let still_pending = app
        .clone()
        .oneshot(authenticated_get_request(
            "/admin/profile",
            &pending_cookie,
        ))
        .await
        .unwrap();
    assert!(
        still_pending.status() == StatusCode::FOUND || still_pending.status() == StatusCode::SEE_OTHER,
        "a wrong code must not have completed the login"
    );

    // Correct code succeeds once.
    let login_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    let first = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie,
            format!("code={login_code}"),
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&first), "/admin");

    // A brand new login attempt, replaying the SAME code, must be rejected —
    // `last_used_step` blocks it even though the code is still numerically
    // "valid" within its time window.
    let login_resp2 = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie2 = extract_cookie(&login_resp2, "admin_session");
    let replay = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie2,
            format!("code={login_code}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        StatusCode::OK,
        "replaying an already-used code must be rejected, not redirect to /admin"
    );

    let _ = synaptic_core::models::mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_totp_recovery_code_login_and_single_use() {
    use synaptic_core::models::{mfa_recovery_code, user_totp};
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfarec").await;

    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());
    let codes = mfa_recovery_code::generate_batch();
    mfa_recovery_code::replace_all(&pool, created.id, &codes)
        .await
        .unwrap();
    let recovery_code = codes[0].clone();

    let app = common::test_router().await;
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie = extract_cookie(&login_resp, "admin_session");

    let verify = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie,
            format!("code={recovery_code}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        redirect_location(&verify),
        "/admin",
        "a valid, unused recovery code must complete login in place of a TOTP code"
    );

    // Single-use: a second login attempt with the same recovery code fails.
    let login_resp2 = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie2 = extract_cookie(&login_resp2, "admin_session");
    let replay = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie2,
            format!("code={recovery_code}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        StatusCode::OK,
        "a used recovery code must be rejected on reuse"
    );

    let _ = mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_mfa_disable_removes_login_requirement() {
    use synaptic_core::models::{mfa_recovery_code, user_totp};
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfadis").await;

    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());

    let app = common::test_router().await;
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    assert_eq!(redirect_location(&login_resp), "/admin/login/mfa");
    let pending_cookie = extract_cookie(&login_resp, "admin_session");
    let login_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    let verify_resp = app
        .clone()
        .oneshot(mfa_verify_request(
            &pending_cookie,
            format!("code={login_code}"),
        ))
        .await
        .unwrap();
    let full_cookie = extract_cookie(&verify_resp, "admin_session");

    let disable_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/disable",
            &full_cookie,
            format!("current_password={password}"),
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&disable_resp), "/admin/profile?success=mfa_disabled");
    assert!(!user_totp::is_enabled(&pool, created.id).await.unwrap());

    // A fresh login now goes straight to /admin — no second factor required.
    let login_resp2 = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    assert_eq!(redirect_location(&login_resp2), "/admin");

    let _ = mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_mfa_endpoints_reject_wrong_current_password() {
    use synaptic_core::models::{mfa_recovery_code, user_totp};
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfawrg").await;

    let app = common::test_router().await;
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let cookie = extract_cookie(&login_resp, "admin_session");
    let wrong_password_body = "current_password=totally-wrong-password".to_string();

    // Setup start with the wrong password must not create a pending secret.
    let start_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/setup/start",
            &cookie,
            wrong_password_body.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&start_resp), "/admin/profile?error=mfa_wrong_password");
    assert!(
        user_totp::pending_secret(&pool, common::TEST_SECRET_KEY, created.id)
            .await
            .unwrap()
            .is_none(),
        "a wrong current-password must not start enrollment"
    );

    // Enroll for real (directly via the model) to test disable/regenerate.
    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());
    let codes = mfa_recovery_code::generate_batch();
    mfa_recovery_code::replace_all(&pool, created.id, &codes)
        .await
        .unwrap();
    let original_code = codes[0].clone();

    // Disable with the wrong password must not disable.
    let disable_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/disable",
            &cookie,
            wrong_password_body.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&disable_resp), "/admin/profile?error=mfa_wrong_password");
    assert!(user_totp::is_enabled(&pool, created.id).await.unwrap());

    // Regenerate with the wrong password must not replace the codes — the
    // original code must still work.
    let regen_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            "/admin/profile/2fa/recovery-codes/regenerate",
            &cookie,
            wrong_password_body,
        ))
        .await
        .unwrap();
    assert_eq!(redirect_location(&regen_resp), "/admin/profile?error=mfa_wrong_password");
    assert!(
        mfa_recovery_code::consume(&pool, created.id, &original_code)
            .await
            .unwrap(),
        "the original recovery codes must be unchanged after a rejected regenerate attempt"
    );

    let _ = mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_mfa_login_rate_limited_after_repeated_wrong_codes() {
    use synaptic_core::models::user_totp;
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfalim").await;

    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());

    let app = common::test_router().await;
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie = extract_cookie(&login_resp, "admin_session");

    let mut last_status = StatusCode::OK;
    for _ in 0..8 {
        last_status = app
            .clone()
            .oneshot(mfa_verify_request(&pending_cookie, "code=000000".to_string()))
            .await
            .unwrap()
            .status();
        if last_status == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
    }
    assert_eq!(
        last_status,
        StatusCode::TOO_MANY_REQUESTS,
        "repeated wrong codes against one pending login must eventually be throttled"
    );

    let _ = synaptic_core::models::mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_mfa_login_route_rejects_cross_origin_post() {
    use synaptic_core::models::user_totp;
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (created, password) = create_mfa_test_admin(&pool, "mfacsr").await;

    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, created.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        created.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());

    let app = common::test_router().await;
    let login_resp = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={password}",
            created.email
        )))
        .await
        .unwrap();
    let pending_cookie = extract_cookie(&login_resp, "admin_session");

    let hostile_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login/mfa")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://attacker.invalid")
                .header("cookie", pending_cookie)
                .extension(common::connect_info())
                .body(Body::from("code=123456"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        hostile_resp.status(),
        StatusCode::FORBIDDEN,
        "a cross-origin POST to /admin/login/mfa must be rejected by the same-origin CSRF check"
    );

    let _ = synaptic_core::models::mfa_recovery_code::delete_all_for_user(&pool, created.id).await;
    let _ = user_totp::disable(&pool, created.id).await;
    let _ = synaptic_core::models::user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_mfa_login_route_rejects_oversized_body() {
    let app = common::test_router().await;
    let big_body = format!("code={}", "9".repeat(20_000));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login/mfa")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(big_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_subscriber_login_never_prompts_for_mfa() {
    use synaptic_core::models::site_user::SiteRole;
    use synaptic_core::models::user::{self, CreateUser, UserRole};

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let password = "Verify12345Pass!".to_string();
    let created = user::create(
        &pool,
        &CreateUser {
            username: format!("mfasub{}", &unique[..9]),
            email: format!("mfasub-{unique}@example.com"),
            display_name: "MFA Subscriber Test".to_string(),
            password: password.clone(),
            role: UserRole::Subscriber,
        },
    )
    .await
    .unwrap();
    // Subscriber login requires site membership, same as staff — seed a
    // role on the "localhost" test site (see common::ensure_test_site).
    let site = synaptic_core::models::site::get_by_hostname(&pool, "localhost")
        .await
        .unwrap();
    synaptic_core::models::site_user::add(&pool, site.id, created.id, SiteRole::Subscriber, None, false)
        .await
        .unwrap();

    let app = common::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .extension(common::connect_info())
                .body(Body::from(format!(
                    "email={}&password={password}",
                    created.email
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let location = redirect_location(&response);
    assert!(
        !location.contains("mfa"),
        "subscriber login must never involve the staff MFA gate, got redirect to {location}"
    );

    let _ = user::delete(&pool, created.id).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL instance — see module docs"]
async fn test_super_admin_can_disable_mfa_for_locked_out_user() {
    use synaptic_core::models::{mfa_recovery_code, user_totp};
    use synaptic_core::utils::totp;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run integration tests (see tests/routes.rs)");
    let pool = synaptic_core::db::connect(&database_url).await.unwrap();
    let (target, target_password) = create_mfa_test_admin(&pool, "mfalck").await;
    let (actor, actor_password) = create_mfa_test_admin(&pool, "mfaact").await;

    let secret = user_totp::start_enrollment(&pool, common::TEST_SECRET_KEY, target.id)
        .await
        .unwrap();
    let confirm_code = totp::generate_code(&secret, chrono::Utc::now().timestamp() as u64).unwrap();
    assert!(user_totp::confirm_enrollment(
        &pool,
        common::TEST_SECRET_KEY,
        target.id,
        &confirm_code,
        chrono::Utc::now(),
    )
    .await
    .unwrap());
    mfa_recovery_code::replace_all(&pool, target.id, &mfa_recovery_code::generate_batch())
        .await
        .unwrap();

    let app = common::test_router().await;
    let actor_login = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={actor_password}",
            actor.email
        )))
        .await
        .unwrap();
    let actor_cookie = extract_cookie(&actor_login, "admin_session");

    let disable_resp = app
        .clone()
        .oneshot(authenticated_post_request(
            &format!("/admin/users/{}/disable-mfa", target.id),
            &actor_cookie,
            String::new(),
        ))
        .await
        .unwrap();
    assert!(
        disable_resp.status() == StatusCode::FOUND || disable_resp.status() == StatusCode::SEE_OTHER,
        "expected a redirect, got {}",
        disable_resp.status()
    );
    assert!(!user_totp::is_enabled(&pool, target.id).await.unwrap());
    assert_eq!(
        mfa_recovery_code::count_remaining(&pool, target.id)
            .await
            .unwrap(),
        0,
        "recovery codes must be cleared along with the TOTP secret"
    );

    // The previously locked-out target can now log in with password only.
    let target_login = app
        .clone()
        .oneshot(login_request(format!(
            "email={}&password={target_password}",
            target.email
        )))
        .await
        .unwrap();
    assert_eq!(redirect_location(&target_login), "/admin");

    let _ = synaptic_core::models::user::delete(&pool, target.id).await;
    let _ = synaptic_core::models::user::delete(&pool, actor.id).await;
}
