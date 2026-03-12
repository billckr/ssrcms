//! Axum router: wires all routes and middleware.

use axum::{
    extract::{DefaultBodyLimit, Request},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;

use crate::app_state::AppState;
use crate::handlers::{account, archive, auth, comment as comment_handler, form as form_handler, home, metrics as metrics_handler, page, plugin_route, post as post_handler, post_unlock, search, subscribe, theme_static};
use crate::handlers::admin::{appearance, comments as admin_comments, dashboard, documentation as admin_documentation, forms as admin_forms, media, media2, menus as admin_menus, posts, profile, settings, sites as admin_sites, taxonomy, upload, users};

/// Prevent browsers from caching admin and account pages.
///
/// Without this, the browser's back button shows a stale cached copy of a
/// protected page after logout. `no-store` is stronger than `no-cache` — it
/// tells the browser not to write the response to any cache at all.
async fn no_store_for_protected(req: Request, next: Next) -> Response {
    let is_protected = {
        let p = req.uri().path();
        p.starts_with("/admin") || p.starts_with("/account")
    };
    let mut response = next.run(req).await;
    if is_protected {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
    }
    response
}

/// Tower middleware that records per-request HTTP metrics.
async fn track_http_metrics(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let start = std::time::Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!("synaptic_http_requests_total",
        "method" => method.clone(),
        "status" => status
    ).increment(1);
    metrics::histogram!("synaptic_http_request_duration_seconds",
        "method" => method
    ).record(duration);

    response
}

pub fn build(
    state: AppState,
    uploads_dir: &str,
    session_layer: SessionManagerLayer<PostgresStore>,
) -> Router {
    let upload_limit = DefaultBodyLimit::max(
        (state.config.max_upload_mb as usize).saturating_mul(1024 * 1024),
    );
    // Static file services
    let uploads_service = ServeDir::new(uploads_dir);

    // Collect plugin route paths so we can register each one individually.
    // Axum requires routes to be registered at build time; we add a dedicated
    // handler for each plugin-registered path.
    let plugin_route_paths: Vec<String> = state.plugin_routes.keys().cloned().collect();

    let mut router = Router::new()
        // ── Observability ──────────────────────────────────────────────────
        .route("/metrics", get(metrics_handler::metrics))
        // ── Public content routes ──────────────────────────────────────────
        .route("/", get(home::home))
        .route("/{slug}", get(post_handler::single_post))
        .route("/{slug}/comment", post(comment_handler::submit))
        .route("/{slug}/save", post(post_handler::save_post))
        .route("/{slug}/unsave", post(post_handler::unsave_post))
        .route("/category/{slug}", get(archive::category_archive))
        .route("/tag/{slug}", get(archive::tag_archive))
        .route("/author/{username}", get(archive::author_archive))
        .route("/search", get(search::search))
        .route("/sitemap.xml", get(plugin_route::sitemap))
        // ── Public form submissions ────────────────────────────────────────
        .route("/form/{name}", post(form_handler::submit))
        // ── Subscriber signup ──────────────────────────────────────────────
        .route("/subscribe", get(subscribe::subscribe_form).post(subscribe::subscribe_post))
        // ── Public login (subscriber-facing) ───────────────────────────────
        .route("/login", get(auth::public_login_form).post(auth::public_login_post))
        // ── Admin auth ─────────────────────────────────────────────────────
        .route("/admin/login", get(auth::login_form).post(auth::login_post))
        .route("/admin/logout", get(auth::logout))
        // ── Account area (any authenticated user) ───────────────────────────
        .route("/account",                        get(account::dashboard))
        .route("/account/profile",                get(account::profile_view))
        .route("/account/profile/update",         post(account::profile_update))
        .route("/account/profile/change-password",post(account::profile_change_password))
        .route("/account/saved-posts",            get(account::saved_posts))
        .route("/account/my-comments",            get(account::my_comments))
        .route("/account/comments/{id}/delete",    post(account::delete_comment))
        .route("/account/logout",                 get(auth::account_logout))
        // ── Admin profile ──────────────────────────────────────────────────
        .route("/admin/profile", get(profile::view))
        .route("/admin/profile/update", post(profile::update_profile))
        .route("/admin/profile/change-password", post(profile::change_password))
        // ── Admin dashboard ────────────────────────────────────────────────
        .route("/admin", get(dashboard::dashboard))
        // ── Admin posts ────────────────────────────────────────────────────
        .route("/admin/posts", get(posts::list))
        .route("/admin/posts/new", get(posts::new_post).post(posts::save_new))
        .route("/admin/posts/{id}/edit", get(posts::edit_post).post(posts::save_edit))
        .route("/admin/posts/{id}/delete", post(posts::delete_post))
        .route("/admin/comments/{id}/delete", post(admin_comments::delete))
        .route("/admin/posts/bulk-delete", post(posts::bulk_delete_posts))
        // ── Admin pages ────────────────────────────────────────────────────
        .route("/admin/pages", get(posts::list_pages))
        .route("/admin/pages/new", get(posts::new_page).post(posts::save_new))
        .route("/admin/pages/{id}/edit", get(posts::edit_page).post(posts::save_edit))
        .route("/admin/pages/{id}/delete", post(posts::delete_page))
        .route("/admin/pages/bulk-delete", post(posts::bulk_delete_pages))
        // ── Admin media API (JSON) ─────────────────────────────────────────
        .route("/admin/api/media", get(media::api_list))
        .route("/admin/api/media/{id}/meta", post(media::api_update_meta))
        // ── Admin media ────────────────────────────────────────────────────
        .route("/admin/media", get(media::list))
        .route("/admin/media2", get(media2::list))
        .route("/admin/media/upload", post(upload::upload).layer(upload_limit.clone()))
        .route("/admin/media/folders/new", post(media::create_folder))
        .route("/admin/media/folders/{id}/delete", post(media::delete_folder))
        .route("/admin/media/{id}/delete", post(media::delete))
        // ── Admin categories ───────────────────────────────────────────────
        .route("/admin/categories", get(taxonomy::categories))
        .route("/admin/categories/new", post(taxonomy::create))
        .route("/admin/categories/{id}/delete", post(taxonomy::delete_category))
        // ── Admin tags ─────────────────────────────────────────────────────
        .route("/admin/tags", get(taxonomy::tags))
        .route("/admin/tags/new", post(taxonomy::create))
        .route("/admin/tags/{id}/delete", post(taxonomy::delete_tag))
        // ── Admin users ────────────────────────────────────────────────────
        .route("/admin/users", get(users::list))
        .route("/admin/users/new", get(users::new_user).post(users::save_new))
        .route("/admin/users/{id}/edit", get(users::edit_user).post(users::save_edit))
        .route("/admin/users/{id}/delete", post(users::delete_user))
        .route("/admin/users/bulk-delete", post(users::bulk_delete_users))
        .route("/admin/users/{id}/site-access", get(users::site_access_page))
        .route("/admin/users/{id}/site-access/add", post(users::add_site_access))
        .route("/admin/users/{id}/site-access/remove", post(users::remove_site_access))
        // ── Admin plugins — disabled pre-launch, re-enable post-launch ────
        // ── Admin documentation ────────────────────────────────────────────
        .route("/admin/documentation", get(admin_documentation::list))
        // ── Admin appearance ───────────────────────────────────────────────
        .route("/admin/appearance", get(appearance::list))
        .route("/admin/appearance/activate", post(appearance::activate))
        .route("/admin/appearance/get-theme", post(appearance::get_theme))
        .route("/admin/appearance/publish-theme", post(appearance::publish_theme))
        .route("/admin/appearance/delete", post(appearance::delete))
        .route("/admin/appearance/upload", post(appearance::upload_theme).layer(upload_limit))
        .route("/admin/theme-screenshot/{theme_name}", get(appearance::screenshot))
        .route("/admin/appearance/create", get(appearance::create_form).post(appearance::create_theme))
        .route("/admin/appearance/editor/{theme}", get(appearance::edit_file))
        .route("/admin/appearance/editor/{theme}/save", post(appearance::save_file))
        .route("/admin/appearance/editor/{theme}/restore", post(appearance::restore_file))
        .route("/admin/appearance/editor/{theme}/new-file", post(appearance::new_file))
        .route("/admin/appearance/editor/{theme}/delete-file", post(appearance::delete_file))
        // ── Admin menus ────────────────────────────────────────────────────
        .route("/admin/menus",                                      get(admin_menus::list).post(admin_menus::create))
        .route("/admin/menus/{id}",                                 get(admin_menus::edit).post(admin_menus::update))
        .route("/admin/menus/{id}/delete",                          post(admin_menus::delete))
        .route("/admin/menus/{id}/items/new",                       post(admin_menus::add_item))
        .route("/admin/menus/{id}/items/{item_id}/edit",            post(admin_menus::edit_item))
        .route("/admin/menus/{id}/items/{item_id}/delete",          post(admin_menus::delete_item))
        // ── Admin settings ─────────────────────────────────────────────────
        .route("/admin/settings", get(settings::settings).post(settings::save_settings))
        // ── Admin sites ────────────────────────────────────────────────────
        .route("/admin/sites", get(admin_sites::list).post(admin_sites::create))
        .route("/admin/sites/go-home", get(admin_sites::go_home))
        .route("/admin/sites/new", get(admin_sites::new_site))
        .route("/admin/sites/switch", post(admin_sites::switch))
        .route("/admin/sites/{id}/settings", get(admin_sites::site_settings).post(admin_sites::save_site_settings))
        .route("/admin/sites/{id}/site-config", post(admin_sites::save_site_config))
        .route("/admin/sites/{id}/delete", post(admin_sites::delete))
        .route("/admin/sites/{id}/provision-ssl", post(admin_sites::provision_ssl))
        // ── Admin forms ────────────────────────────────────────────────────
        .route("/admin/forms", get(admin_forms::list_forms))
        .route("/admin/forms/{name}", get(admin_forms::view_form))
        .route("/admin/forms/{name}/{id}/delete", post(admin_forms::delete_submission))
        .route("/admin/forms/{name}/delete-all", post(admin_forms::delete_all))
        .route("/admin/forms/{name}/export", get(admin_forms::export_csv))
        .route("/admin/forms/{name}/toggle-block", post(admin_forms::toggle_block))
    // ── Static files ───────────────────────────────────────────────────
        .nest_service("/uploads", uploads_service)
        .route("/theme/static/{*path}", get(theme_static::serve))
        .nest_service("/admin/static", ServeDir::new("admin/static"));

    // Register plugin routes — skip any paths already handled by hardcoded routes.
    for path in plugin_route_paths {
        if path == "/sitemap.xml" {
            continue; // handled by the hardcoded route above
        }
        router = router.route(&path, get(plugin_route::dispatch));
    }

    // /:slug/unlock must be registered before the fallback.
    // Nested password-protected pages are not supported in MVP (guarded at handler level).
    router = router.route("/{slug}/unlock", post(post_unlock::unlock_page));
    // fallback handles nested page URLs like /a/b/c and any unmatched /{slug} that resolves to a page.
    router = router.fallback(page::single_page);

    router
        .layer(middleware::from_fn(no_store_for_protected))
        .layer(middleware::from_fn(track_http_metrics))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
