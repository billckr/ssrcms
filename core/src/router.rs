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
use crate::handlers::{archive, auth, form as form_handler, home, metrics as metrics_handler, page, plugin_route, post as post_handler, post_unlock, search, theme_static};
use crate::handlers::admin::{appearance, dashboard, forms as admin_forms, media, plugins, posts, profile, settings, sites as admin_sites, taxonomy, upload, users};

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
        .route("/blog/{slug}", get(post_handler::single_post))
        .route("/blog/{slug}/unlock", post(post_unlock::unlock_post))
        .route("/category/{slug}", get(archive::category_archive))
        .route("/tag/{slug}", get(archive::tag_archive))
        .route("/author/{username}", get(archive::author_archive))
        .route("/search", get(search::search))
        .route("/sitemap.xml", get(plugin_route::sitemap))
        // ── Public form submissions ────────────────────────────────────────
        .route("/form/{name}", post(form_handler::submit))
        // ── Admin auth ─────────────────────────────────────────────────────
        .route("/admin/login", get(auth::login_form).post(auth::login_post))
        .route("/admin/logout", get(auth::logout))
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
        .route("/admin/media/upload", post(upload::upload).layer(upload_limit.clone()))
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
        .route("/admin/users/{id}/delete", post(users::delete_user))        .route("/admin/users/{id}/site-access", get(users::site_access_page))
        .route("/admin/users/{id}/site-access/add", post(users::add_site_access))
        .route("/admin/users/{id}/site-access/remove", post(users::remove_site_access))        // ── Admin plugins ──────────────────────────────────────────────────
        .route("/admin/plugins", get(plugins::list))
        .route("/admin/plugins/install",    post(plugins::install))
        .route("/admin/plugins/upload",     post(plugins::upload).layer(upload_limit.clone()))
        .route("/admin/plugins/activate",   post(plugins::activate))
        .route("/admin/plugins/deactivate", post(plugins::deactivate))
        .route("/admin/plugins/delete",     post(plugins::delete))
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

    // /:slug/unlock and /:slug must be last — /:slug catches anything not matched above
    router = router.route("/{slug}/unlock", post(post_unlock::unlock_page));
    router = router.route("/{slug}", get(page::single_page));

    router
        .layer(middleware::from_fn(track_http_metrics))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
