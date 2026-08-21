//! Axum router: wires all routes and middleware.

use axum::{
    extract::{DefaultBodyLimit, Request},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;

use crate::app_state::AppState;
use crate::handlers::{account, archive, auth, comment as comment_handler, form as form_handler, poll as poll_handler, home, metrics as metrics_handler, page, plugin_route, post as post_handler, post_unlock, recover, search, subscribe, theme_static, uploads};
use crate::handlers::admin::{activity_log, analytics as admin_analytics, themes, themes_editor, themes_publish, themes_upload, builder as admin_builder, comments as admin_comments, dashboard, designer_hub as admin_designer_hub, dev_tools, documentation as admin_documentation, email_providers as admin_email_providers, form_designer as admin_form_designer, forms as admin_forms, logo_upload, media, menus as admin_menus, poll_designer as admin_poll_designer, poll_results as admin_poll_results, posts, profile, role_picker, settings, site_settings, sites as admin_sites, taxonomy, upload, users, wp_import};

/// Prevent browsers from caching admin and account pages.
///
/// Without this, the browser's back button shows a stale cached copy of a
/// protected page after logout. `no-store` is stronger than `no-cache` — it
/// tells the browser not to write the response to any cache at all.
///
/// Only applies when the handler didn't already set its own `Cache-Control`
/// — a handler serving an asset that's safe (and meant) to cache, like the
/// theme screenshot endpoint's `public, max-age=3600`, knows better than
/// this blanket page-level policy, which only cares about HTML views.
async fn no_store_for_protected(req: Request, next: Next) -> Response {
    let is_protected = {
        let p = req.uri().path();
        p.starts_with("/admin") || p.starts_with("/account")
    };
    let mut response = next.run(req).await;
    if is_protected && !response.headers().contains_key(axum::http::header::CACHE_CONTROL) {
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
    admin_session_layer: SessionManagerLayer<PostgresStore>,
    account_session_layer: SessionManagerLayer<PostgresStore>,
) -> Router {
    // Absolute safety net against unbounded/chunked bodies — fixed at startup,
    // deliberately generous. The real, admin-configurable ceiling is enforced
    // dynamically below by `upload_limit_layer` so it can change without a
    // restart.
    let upload_limit = tower::ServiceBuilder::new()
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), crate::middleware::upload_limit::gate));

    // Collect plugin route paths so we can register each one individually.
    // Axum requires routes to be registered at build time; we add a dedicated
    // handler for each plugin-registered path.
    let plugin_route_paths: Vec<String> = state.plugin_routes.keys().cloned().collect();

    let maintenance_layer = middleware::from_fn_with_state(state.clone(), crate::middleware::maintenance::gate);
    let ip_allowlist_layer = middleware::from_fn_with_state(state.clone(), crate::middleware::ip_allowlist::gate);
    let ip_denylist_layer = middleware::from_fn_with_state(state.clone(), crate::middleware::ip_denylist::gate);

    // Public/subscriber routes get their own session cookie (24h idle timeout —
    // low-risk accounts, self-service only). Split out from admin so each group
    // can carry its own SessionManagerLayer with a different expiry.
    let mut public_router = Router::new()
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
        // ── Public poll voting ──────────────────────────────────────────────
        .route("/poll/{slug}", post(poll_handler::submit))
        .route("/poll/{slug}/results", get(poll_handler::results))
        // ── Subscriber signup ──────────────────────────────────────────────
        .route("/subscribe", get(subscribe::subscribe_form).post(subscribe::subscribe_post))
        // ── Public login (subscriber-facing) ───────────────────────────────
        .route("/login", get(auth::public_login_form).post(auth::public_login_post))
        // ── Password recovery (subscriber-facing) ───────────────────────────
        .route("/recover", get(recover::request_form).post(recover::request_post))
        .route("/recover/{token}", get(recover::reset_form).post(recover::reset_post))
        // ── Account area (any authenticated user) ───────────────────────────
        .route("/account",                        get(account::dashboard))
        .route("/account/profile",                get(account::profile_view))
        .route("/account/profile/update",         post(account::profile_update))
        .route("/account/profile/change-password",post(account::profile_change_password))
        .route("/account/saved-posts",            get(account::saved_posts))
        .route("/account/my-comments",            get(account::my_comments))
        .route("/account/comments/{id}/delete",    post(account::delete_comment))
        .route("/account/logout",                 get(auth::account_logout))
        // ── Static files ──────────────────────────────────────────────────
        .route("/uploads/{*path}", get(uploads::serve))
        .route("/theme/static/{*path}", get(theme_static::serve));

    // Register plugin routes — skip any paths already handled by hardcoded routes.
    for path in &plugin_route_paths {
        if path == "/sitemap.xml" {
            continue; // handled by the hardcoded route above
        }
        public_router = public_router.route(path, get(plugin_route::dispatch));
    }

    // /:slug/unlock must be registered before the fallback.
    // Nested password-protected pages are not supported in MVP (guarded at handler level).
    public_router = public_router.route("/{slug}/unlock", post(post_unlock::unlock_page));
    // fallback handles nested page URLs like /a/b/c and any unmatched /{slug} that resolves to
    // a page. Registered here (inside public_router, before `.layer()` below) rather than on the
    // merged router, because `page::single_page` extracts `Session` — it must be wrapped by
    // account_session_layer or that extraction fails at runtime.
    public_router = public_router.fallback(page::single_page);

    let public_router = public_router.layer(account_session_layer);

    // Admin/staff routes get a short-idle session cookie (2h — higher risk:
    // can edit site config, content, media, and users).
    let admin_router = Router::new()
        // ── Admin auth ─────────────────────────────────────────────────────
        .route("/admin/login", get(auth::login_form).post(auth::login_post))
        .route("/admin/logout", get(auth::logout))
        // ── Admin profile ──────────────────────────────────────────────────
        .route("/admin/profile", get(profile::view))
        .route("/admin/profile/update", post(profile::update_profile))
        .route("/admin/profile/change-password", post(profile::change_password))
        // ── Admin dashboard ────────────────────────────────────────────────
        .route("/admin", get(dashboard::dashboard))
        .route("/admin2", get(dashboard::dashboard2))
        .route("/admin/dashboard/widget-layout", post(dashboard::save_widget_layout))
        .route("/admin/dashboard/dismiss-welcome", post(dashboard::dismiss_welcome_panel))
        // ── Admin posts ────────────────────────────────────────────────────
        .route("/admin/posts", get(posts::list))
        .route("/admin/posts/new", get(posts::new_post).post(posts::save_new))
        .route("/admin/posts/{id}/edit", get(posts::edit_post).post(posts::save_edit))
        .route("/admin/posts/{id}/delete", post(posts::delete_post))
        .route("/admin/api/posts/{id}/sources-public", post(posts::api_set_sources_public))
        .route("/admin/api/posts/{id}/sources", post(posts::api_set_sources))
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
        .route("/admin/api/media/grid", get(media::api_grid))
        .route("/admin/api/media/{id}/meta", post(media::api_update_meta))
        .route("/admin/api/media/{id}/folder", post(media::api_update_folder))
        // ── Admin media ────────────────────────────────────────────────────
        .route("/admin/media", get(media::list))
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
        .route("/admin/users/{id}/suspend", post(users::suspend_user))
        .route("/admin/users/{id}/reactivate", post(users::reactivate_user))
        .route("/admin/users/{id}/erase-personal-data", get(users::erase_personal_data_review).post(users::erase_personal_data))
        .route("/admin/users/bulk-delete", post(users::bulk_delete_users))
        .route("/admin/users/{id}/site-access", get(users::site_access_page))
        .route("/admin/users/{id}/site-access/add", post(users::add_site_access))
        .route("/admin/users/{id}/site-access/remove", post(users::remove_site_access))
        .route("/admin/activity-log", get(activity_log::list))
        // ── Admin plugins — disabled pre-launch, re-enable post-launch ────
        // ── Admin documentation ────────────────────────────────────────────
        .route("/admin/documentation", get(admin_documentation::list))
        // ── Admin themes ───────────────────────────────────────────────────
        .route("/admin/themes", get(themes::list))
        .route("/admin/themes/activate", post(themes::activate))
        .route("/admin/themes/get-theme", post(themes_publish::get_theme))
        .route("/admin/themes/publish-theme", post(themes_publish::publish_theme))
        .route("/admin/themes/delete", post(themes::delete))
        .route("/admin/themes/upload", post(themes_upload::upload_theme).layer(upload_limit.clone()))
        .route("/admin/theme-screenshot/{theme_name}", get(themes::screenshot))
        .route("/admin/themes/create", get(themes_upload::create_form).post(themes_upload::create_theme))
        .route("/admin/themes/editor/{theme}", get(themes_editor::edit_file))
        .route("/admin/themes/editor/{theme}/save", post(themes_editor::save_file))
        .route("/admin/themes/editor/{theme}/restore", post(themes_editor::restore_file))
        .route("/admin/themes/editor/{theme}/new-file", post(themes_editor::new_file))
        .route("/admin/themes/editor/{theme}/delete-file", post(themes_editor::delete_file))
        .route("/admin/themes/editor/{theme}/customizer-save", post(themes_editor::save_customizer))
        .route("/admin/themes/editor/{theme}/customizer-reset", post(themes_editor::reset_options))
        // Old URL, kept as a permanent redirect for existing bookmarks/links.
        .route("/admin/appearance", get(|| async { axum::response::Redirect::permanent("/admin/themes") }))
        // ── Page builder ───────────────────────────────────────────────────
        .route("/admin/builder",                                        get(admin_builder::list))
        .route("/admin/builder/create",                                 post(admin_builder::create_project))
        .route("/admin/builder/deactivate",                             post(admin_builder::deactivate_project))
        .route("/admin/builder/save",                                   post(admin_builder::save))
        .route("/admin/builder/publish",                                post(admin_builder::publish))
        .route("/admin/builder/load/{id}",                              get(admin_builder::load))
        .route("/admin/builder/{project_id}",                           get(admin_builder::project_pages))
        .route("/admin/builder/{project_id}/rename",                     post(admin_builder::rename_project))
        .route("/admin/builder/{project_id}/activate",                  post(admin_builder::activate_project))
        .route("/admin/builder/{project_id}/delete",                    post(admin_builder::delete_project))
        .route("/admin/builder/{project_id}/pages/new",                 get(admin_builder::new_page_form).post(admin_builder::create_page))
        .route("/admin/builder/{project_id}/pages/{page_id}",          get(admin_builder::edit_page))
        .route("/admin/builder/{project_id}/pages/{page_id}/set-homepage", post(admin_builder::set_homepage))
        .route("/admin/builder/{project_id}/pages/{page_id}/duplicate",   post(admin_builder::duplicate_page))
        .route("/admin/builder/{project_id}/pages/{page_id}/delete",      post(admin_builder::delete_page))
        // ── Admin menus ────────────────────────────────────────────────────
        .route("/admin/menus",                                      get(admin_menus::list).post(admin_menus::create))
        .route("/admin/menus/{id}",                                 get(admin_menus::edit).post(admin_menus::update))
        .route("/admin/menus/{id}/delete",                          post(admin_menus::delete))
        .route("/admin/menus/{id}/items/new",                       post(admin_menus::add_item))
        .route("/admin/menus/{id}/items/{item_id}/edit",            post(admin_menus::edit_item))
        .route("/admin/menus/{id}/items/{item_id}/delete",          post(admin_menus::delete_item))
        .route("/admin/menus/{id}/items/reorder",                   post(admin_menus::reorder_items))
        // ── Admin settings ─────────────────────────────────────────────────
        .route("/admin/settings", get(settings::settings).post(settings::save_settings))
        .route("/admin/site-settings", get(site_settings::view).post(site_settings::save_general))
        .route("/admin/site-settings/logo", post(site_settings::upload_logo))
        .route("/admin/site-settings/logo/reset", post(site_settings::reset_logo))
        .route("/admin/settings/logo", post(logo_upload::upload_logo))
        .route("/admin/settings/logo/reset", post(logo_upload::reset_logo))
        .route("/admin/settings/dev-tools/seed-users", post(dev_tools::seed_users))
        .route("/admin/settings/dev-tools/seed-posts", post(dev_tools::seed_posts))
        .route("/admin/settings/dev-tools/clear", post(dev_tools::clear_test_data))
        .route("/admin/settings/dev-tools/nuke-all", post(dev_tools::nuke_all))
        .route("/admin/settings/dev-tools/reindex-search", post(dev_tools::reindex_search))
        // ── Admin sites ────────────────────────────────────────────────────
        .route("/admin/pick-role", get(role_picker::show).post(role_picker::submit))
        .route("/admin/sites", get(admin_sites::list).post(admin_sites::create))
        .route("/admin/sites/go-home", get(admin_sites::go_home))
        .route("/admin/sites/new", get(admin_sites::new_site))
        .route("/admin/sites/switch", post(admin_sites::switch))
        .route("/admin/sites/{id}/settings", get(admin_sites::site_settings))
        .route("/admin/sites/{id}/site-config", post(admin_sites::save_site_config))
        .route("/admin/sites/{id}/maintenance", post(admin_sites::save_maintenance))
        .route("/admin/sites/{id}/import-wp", post(wp_import::import).layer(upload_limit.clone()))
        .route("/admin/sites/{id}/import-wp/status", get(wp_import::status))
        .route("/admin/sites/{id}/import-wp/credentials.csv", get(wp_import::credentials_csv))
        .route("/admin/sites/{id}/email-providers", post(admin_email_providers::create))
        .route("/admin/sites/{id}/email-providers/{provider_id}", post(admin_email_providers::update))
        .route("/admin/sites/{id}/email-providers/{provider_id}/test", post(admin_email_providers::test))
        .route("/admin/sites/{id}/email-providers/{provider_id}/delete", post(admin_email_providers::delete))
        .route("/admin/sites/{id}/delete", post(admin_sites::delete))
        .route("/admin/sites/{id}/provision-ssl", post(admin_sites::provision_ssl))
        // ── Admin forms / polls ──────────────────────────────────────────────
        .route("/admin/designer", get(admin_designer_hub::list))
        .route("/admin/form-designer", get(admin_form_designer::list).post(admin_form_designer::create))
        .route("/admin/form-designer/new", get(admin_form_designer::new_form))
        .route("/admin/form-designer/{id}", get(admin_form_designer::edit_form).post(admin_form_designer::update))
        .route("/admin/form-designer/{id}/delete", post(admin_form_designer::delete))
        .route("/admin/designer/polls", get(admin_poll_designer::list).post(admin_poll_designer::create))
        .route("/admin/designer/polls/new", get(admin_poll_designer::new_poll))
        .route("/admin/designer/polls/{id}", get(admin_poll_designer::edit_poll).post(admin_poll_designer::update))
        .route("/admin/designer/polls/{id}/delete", post(admin_poll_designer::delete))
        .route("/admin/designer/polls/{id}/results", get(admin_poll_results::view))
        .route("/admin/designer/polls/{id}/results/export", get(admin_poll_results::export_csv))
        .route("/admin/designer/polls/{id}/results/reset", post(admin_poll_results::reset))
        .route("/admin/analytics", get(admin_analytics::list))
        .route("/admin/analytics/form/{id}", get(admin_analytics::form_detail))
        .route("/admin/form-data-analytics/{name}", get(admin_forms::view_form))
        .route("/admin/form-data-analytics/{name}/{id}/delete", post(admin_forms::delete_submission))
        .route("/admin/form-data-analytics/{name}/delete-all", post(admin_forms::delete_all))
        .route("/admin/form-data-analytics/{name}/export", get(admin_forms::export_csv))
        .route("/admin/form-data-analytics/{name}/toggle-block", post(admin_forms::toggle_block))
        // ── Static files ───────────────────────────────────────────────────
        // CSS/JS/icons here have no cache-busting in their URLs (no ?v=, no
        // content hash) — a new deploy overwrites the same filenames. So this
        // uses a short max-age plus must-revalidate rather than a long/
        // "immutable" cache: browsers avoid re-fetching within the window,
        // but a deploy is visible within minutes via ServeDir's built-in
        // Last-Modified conditional-GET support (cheap 304s), not stale for
        // however long a longer max-age would otherwise hide it.
        .nest_service(
            "/admin/static",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("public, max-age=300, must-revalidate"),
                ))
                .service(ServeDir::new("admin/static")),
        )
        .layer(admin_session_layer);

    let router = public_router.merge(admin_router);

    router
        .layer(middleware::from_fn(no_store_for_protected))
        .layer(maintenance_layer)
        .layer(ip_allowlist_layer)
        .layer(ip_denylist_layer)
        .layer(middleware::from_fn(track_http_metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
