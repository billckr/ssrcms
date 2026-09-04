use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::post::{CreatePost, PostStatus, PostType, UpdatePost, ListFilter};
use crate::models::taxonomy::TaxonomyType;
use admin::pages::posts::{PostEdit, PostRow, TermOption};

#[derive(Deserialize, Default)]
pub struct PostsQuery {
    pub page: Option<i64>,
    pub status: Option<String>,
    /// Free-text filter for post title — stop words stripped before building ILIKE clauses.
    #[serde(default)]
    pub search: Option<String>,
    /// When set (any value), return only the table fragment HTML for JS live-search.
    #[serde(default)]
    pub partial: Option<String>,
    /// Column to sort by: "title" | "status" | "author" | "domain" | "date".
    #[serde(default)]
    pub sort: Option<String>,
    /// Sort direction: "asc" or "desc".
    #[serde(default)]
    pub dir: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let author_filter = if admin.site_role == Some(crate::models::site_user::SiteRole::Author) { Some(admin.user.id) } else { None };
    list_type(state, "post", q.page, q.status.as_deref(), q.search.as_deref(), q.partial.as_deref(), admin.site_id, author_filter, q.sort.as_deref(), q.dir.as_deref(), ctx).await
}

pub async fn list_pages(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    list_type(state, "page", q.page, q.status.as_deref(), q.search.as_deref(), q.partial.as_deref(), admin.site_id, None, q.sort.as_deref(), q.dir.as_deref(), ctx).await.into_response()
}

#[allow(clippy::too_many_arguments)]
async fn list_type(state: AppState, post_type: &str, page: Option<i64>, status_filter: Option<&str>, search: Option<&str>, partial: Option<&str>, site_id: Option<Uuid>, author_id: Option<Uuid>, sort: Option<&str>, dir: Option<&str>, ctx: admin::PageContext) -> Html<String> {
    let per_page = 20i64;
    let page = page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    // Resolve the PostStatus filter. When no filter is selected ("All"), exclude trashed posts.
    let status_enum: Option<PostStatus> = match status_filter {
        Some("draft")     => Some(PostStatus::Draft),
        Some("pending")   => Some(PostStatus::Pending),
        Some("published") => Some(PostStatus::Published),
        Some("scheduled") => Some(PostStatus::Scheduled),
        Some("trashed")   => Some(PostStatus::Trashed),
        _                 => None,
    };
    let status_sql = status_enum.as_ref().map(|s| s.as_str());
    let exclude_trashed = status_enum.is_none(); // When viewing "All", exclude trashed posts

    // Strip stop words from the search input once; reuse for both COUNT and SELECT.
    let search_str = search.unwrap_or("").trim();
    let search_opt = if search_str.is_empty() { None } else { Some(search_str) };
    let terms = search_opt.map(crate::models::post::search_terms).unwrap_or_default();

    // COUNT — same filters as SELECT. Dynamic ILIKE clauses mirror the SELECT query.
    // Fixed params: $1=site_id, $2=post_type, $3=author_id, $4=status, $5=exclude_trashed,
    // $6=template (always NULL now — the pages-list template filter dropdown was
    // removed, but the query keeps this param slot rather than renumbering
    // everything after it); search terms start at $7. LEFT JOIN users so a
    // search term can match the author's display name as well as the title.
    let mut count_sql = "SELECT COUNT(*) FROM posts p \
                         LEFT JOIN users u ON u.id = p.author_id \
                         WHERE ($1::uuid IS NULL OR p.site_id = $1) \
                           AND p.post_type = $2 \
                           AND ($3::uuid IS NULL OR p.author_id = $3) \
                           AND ($4::text IS NULL OR p.status = $4) \
                           AND (NOT $5::bool OR p.status != 'trashed') \
                           AND ($6::text IS NULL OR ($6 = '__default__' AND p.template IS NULL) OR p.template = $6)"
        .to_string();
    for i in 0..terms.len() {
        let n = i + 7;
        count_sql.push_str(&format!(" AND (LOWER(p.title) LIKE ${n} OR LOWER(u.display_name) LIKE ${n})"));
    }
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql)
        .bind(site_id)
        .bind(post_type)
        .bind(author_id)
        .bind(status_sql)
        .bind(exclude_trashed)
        .bind(None::<String>);
    for term in &terms {
        count_q = count_q.bind(format!("%{term}%"));
    }
    let total: i64 = count_q.fetch_one(&state.db).await.unwrap_or(0);

    let total_pages = ((total + per_page - 1) / per_page).max(1);

    // Count of all pending posts for this site (for the tab badge regardless of current filter)
    let pending_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM posts
           WHERE status = 'pending'
             AND post_type = $1
             AND ($2::uuid IS NULL OR site_id = $2)
             AND ($3::uuid IS NULL OR author_id = $3)"#,
    )
    .bind(post_type)
    .bind(site_id)
    .bind(author_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // Count of scheduled posts for this author (so we can conditionally show the Scheduled tab)
    let author_scheduled_count: i64 = if author_id.is_some() {
        sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM posts
               WHERE status = 'scheduled'
                 AND post_type = $1
                 AND ($2::uuid IS NULL OR site_id = $2)
                 AND ($3::uuid IS NULL OR author_id = $3)"#,
        )
        .bind(post_type)
        .bind(site_id)
        .bind(author_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0)
    } else {
        0
    };

    let filter = ListFilter {
        site_id,
        status: status_enum,
        post_type: Some(if post_type == "page" { PostType::Page } else { PostType::Post }),
        author_id,
        limit: per_page,
        offset,
        search: search_opt.map(|s| s.to_string()),
        template: None,
        exclude_trashed,
        sort: sort.map(|s| s.to_string()),
        sort_dir: dir.map(|s| s.to_string()),
        ..Default::default()
    };

    let raw = crate::models::post::list(&state.db, &filter).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list {} items: {:?}", post_type, e);
        vec![]
    });

    // Snapshot site hostname + permalink_structure once so we don't hold the
    // lock (or re-fetch settings) per-row.
    let (site_hostnames, permalink_structures): (
        std::collections::HashMap<Uuid, String>,
        std::collections::HashMap<Uuid, String>,
    ) = state.site_cache.read()
        .map(|cache| {
            let hostnames = cache.values().map(|(s, _)| (s.id, s.hostname.clone())).collect();
            let structures = cache.values().map(|(s, settings)| (s.id, settings.permalink_structure.clone())).collect();
            (hostnames, structures)
        })
        .unwrap_or_default();

    let mut rows: Vec<PostRow> = Vec::new();

    for p in raw.iter() {
        let author_name = crate::models::user::get_by_id_include_inactive(&state.db, p.author_id)
            .await
            .map(|u| u.display_name)
            .unwrap_or_else(|e| {
                tracing::warn!("failed to fetch author {}: {:?}", p.author_id, e);
                "Unknown".to_string()
            });

        let site_hostname = p.site_id
            .and_then(|sid| site_hostnames.get(&sid).cloned())
            .unwrap_or_default();

        // View link: pages keep the flat `/{slug}` path (unaffected by
        // permalink_structure — see SiteSettings::permalink_structure's doc
        // comment); posts use the site's configured structure, same as the
        // public-facing URL PostContext.url would build.
        let view_path = if p.post_type == "page" {
            format!("/{}", p.slug)
        } else {
            let structure = p.site_id
                .and_then(|sid| permalink_structures.get(&sid))
                .map(|s| s.as_str())
                .unwrap_or("/%postname%");
            let category_slug = if structure.contains("%category%") {
                crate::models::taxonomy::for_post(&state.db, p.id).await
                    .unwrap_or_default()
                    .into_iter()
                    .find(|t| t.taxonomy == "category")
                    .map(|t| t.slug)
            } else {
                None
            };
            crate::models::post::build_permalink(structure, p, category_slug.as_deref())
        };

        rows.push(PostRow {
            id: p.id.to_string(),
            title: p.title.clone(),
            status: p.status.clone(),
            slug: p.slug.clone(),
            post_type: p.post_type.clone(),
            author_name,
            published_at: p.published_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
            post_password_set: p.post_password.is_some(),
            site_hostname,
            view_path,
        });
    }

    // `partial=<anything>` means the JS live-search is requesting only the table
    // fragment so it can swap div#posts-list without a full page reload.
    if partial.is_some() {
        Html(admin::pages::posts::posts_list_fragment(&rows, post_type, page, total_pages, &ctx, status_filter, search_str, sort, dir))
    } else {
        Html(admin::pages::posts::render_list(&rows, post_type, page, total_pages, None, &ctx, status_filter, pending_count, author_scheduled_count, search_str, sort, dir))
    }
}

/// Look up an existing post's author for the Author card, tolerating both
/// a missing post and a missing/suspended author rather than failing the
/// whole form re-render (used on validation-failure paths in `save_edit`).
async fn resolve_post_author(state: &AppState, post_id: Uuid) -> (String, bool, String) {
    let Ok(post) = crate::models::post::get_by_id(&state.db, post_id).await else {
        return (String::new(), true, String::new());
    };
    crate::models::user::get_by_id_include_inactive(&state.db, post.author_id)
        .await
        .map(|u| (u.display_name, u.is_active, u.id.to_string()))
        .unwrap_or_else(|_| ("Unknown".to_string(), true, post.author_id.to_string()))
}

pub async fn new_post(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    new_post_type(state, "post", admin.site_id, &admin, &cs, ctx).await
}

pub async fn new_page(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    new_post_type(state, "page", admin.site_id, &admin, &cs, ctx).await.into_response()
}

async fn new_post_type(state: AppState, post_type: &str, site_id: Option<Uuid>, admin: &AdminUser, site_hostname: &str, ctx: admin::PageContext) -> Html<String> {
    let (categories, tags) = fetch_term_options(&state, site_id).await;
    let available_templates = if post_type == "page" { scan_templates(&state, site_id) } else { vec![] };
    let available_parents = if post_type == "page" {
        fetch_parent_options(&state, site_id, None).await
    } else {
        vec![]
    };
    let edit = PostEdit {
        id: None,
        title: String::new(),
        slug: String::new(),
        content: String::new(),
        excerpt: String::new(),
        status: "draft".into(),
        published_at: None,
        post_type: post_type.to_string(),
        categories,
        tags,
        selected_categories: vec![],
        selected_tags: vec![],
        template: None,
        available_templates,
        featured_image_id: None,
        featured_image_url: None,
        post_password_set: false,
        comments_enabled: false,
        comment_count: 0,
        author_name: admin.user.display_name.clone(),
        author_is_active: admin.user.is_active,
        author_id: admin.user.id.to_string(),
        site_name: site_hostname.to_string(),
        site_id: site_id.map(|id| id.to_string()).unwrap_or_default(),
        parent_id: None,
        available_parents,
        sources: vec![],
        sources_public: false,
        live_url: None,
        preview_url: None,
        saved_forms: fetch_saved_forms(&state, site_id).await,
        saved_polls: fetch_saved_polls(&state, site_id).await,
        form_analytics: vec![], // brand-new post has no content yet to embed a form in
        created_at: None,
        updated_at: None,
        version: None,
    };
    Html(admin::pages::posts::render_editor(&edit, None, &ctx))
}

#[derive(Deserialize, Default)]
pub struct EditPostQuery {
    pub success: Option<String>,
}

pub async fn edit_post(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(q): Query<EditPostQuery>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let is_restricted_author = admin.site_role == Some(crate::models::site_user::SiteRole::Author) && !admin.can_self_publish;
    edit_post_type(state, id, admin.site_id, is_restricted_author, admin.user.id, ctx, q.success.as_deref()).await
}

pub async fn edit_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(q): Query<EditPostQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    edit_post_type(state, id, admin.site_id, false, admin.user.id, ctx, q.success.as_deref()).await.into_response()
}

/// `is_author` here really means "is a *restricted* Author" — callers
/// already fold in `!admin.can_self_publish`, so a self-publishing author
/// reaches this function with `false`, same as an Editor.
async fn edit_post_type(state: AppState, id: Uuid, site_id: Option<Uuid>, is_author: bool, user_id: Uuid, ctx: admin::PageContext, success: Option<&str>) -> impl IntoResponse {
    let post = match crate::models::post::get_by_id(&state.db, id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("post {} not found for editing: {:?}", id, e);
            return Redirect::to("/admin/posts").into_response();
        }
    };

    // Site isolation: non-global admins may only edit posts that belong to their site.
    if !ctx.is_global_admin && post.site_id != site_id {
        return Redirect::to("/admin/posts").into_response();
    }

    // Author restriction: authors can only edit their own draft/pending content.
    if is_author {
        if post.author_id != user_id {
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            return Redirect::to(redirect).into_response();
        }
        if post.status == "published" || post.status == "scheduled" {
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            return Redirect::to(redirect).into_response();
        }
    }

    let (categories, tags) = fetch_term_options(&state, site_id).await;
    let available_templates = if post.post_type == "page" { scan_templates(&state, site_id) } else { vec![] };

    let post_terms = crate::models::taxonomy::for_post(&state.db, id).await.unwrap_or_else(|e| {
        tracing::warn!("failed to fetch terms for post {}: {:?}", id, e);
        vec![]
    });
    let selected_categories: Vec<String> = post_terms.iter()
        .filter(|t| t.taxonomy == "category")
        .map(|t| t.id.to_string())
        .collect();
    let selected_tags: Vec<String> = post_terms.iter()
        .filter(|t| t.taxonomy == "tag")
        .map(|t| t.id.to_string())
        .collect();

    // Keep the UUID-prefixed path — this is admin (browsed via the shared
    // bckr.local host, not the site's own domain), so a bare-filename URL
    // can't resolve; see the comment in handlers/admin/media.rs::list.
    let featured_image_url = if let Some(img_id) = post.featured_image_id {
        crate::models::media::get_by_id(&state.db, img_id).await
            .ok()
            .map(|m| format!("/uploads/{}", m.path))
    } else {
        None
    };

    let (author_name, author_is_active) = crate::models::user::get_by_id_include_inactive(&state.db, post.author_id)
        .await
        .map(|u| (u.display_name, u.is_active))
        .unwrap_or_else(|_| ("Unknown".to_string(), true));

    let site_name = post.site_id
        .and_then(|sid| {
            state.site_cache.read().ok()
                .and_then(|cache| cache.values().find(|(s, _)| s.id == sid).map(|(s, _)| s.hostname.clone()))
        })
        .unwrap_or_default();

    let comment_count = crate::models::comment::count_for_post(&state.db, post.id)
        .await
        .unwrap_or(0) as u64;

    let available_parents = if post.post_type == "page" {
        fetch_parent_options(&state, site_id, Some(id)).await
    } else {
        vec![]
    };

    // Absolute, not relative: admin for a site other than the one currently
    // in the address bar is routine here (super admin browsing another
    // site's content, or admin reached via the shared bckr.local host) — a
    // relative href would resolve against whatever origin the admin page
    // itself loaded from instead of the post's own site.
    let site_lookup = post.site_id.and_then(|sid| state.get_site_by_id(sid));
    let post_path = if post.post_type == "page" {
        crate::models::post::get_full_page_path(&state.db, &post).await
    } else {
        let structure = site_lookup.as_ref()
            .map(|(_, settings)| settings.permalink_structure.as_str())
            .unwrap_or("/%postname%");
        let category_slug = if structure.contains("%category%") {
            crate::models::taxonomy::for_post(&state.db, post.id).await
                .unwrap_or_default()
                .into_iter()
                .find(|t| t.taxonomy == "category")
                .map(|t| t.slug)
        } else {
            None
        };
        crate::models::post::build_permalink(structure, &post, category_slug.as_deref())
    };
    let post_base_url = site_lookup
        .map(|(site, settings)| {
            if settings.base_url != "http://localhost:3000" {
                settings.base_url
            } else {
                format!("http://{}", site.hostname)
            }
        })
        .unwrap_or_default();
    let post_url = format!("{}{}", post_base_url, post_path);
    let live_url = if post.status == "published" { Some(post_url.clone()) } else { None };
    let preview_url = match post.status.as_str() {
        "draft" | "pending" | "scheduled" => Some(post_url),
        _ => None,
    };

    let edit = PostEdit {
        id: Some(post.id.to_string()),
        title: post.title.clone(),
        slug: post.slug.clone(),
        content: post.content.clone(),
        excerpt: post.excerpt.unwrap_or_default(),
        status: post.status.clone(),
        published_at: post.published_at.map(|d| d.format("%Y-%m-%dT%H:%M").to_string()),
        post_type: post.post_type.clone(),
        categories,
        tags,
        selected_categories,
        selected_tags,
        template: post.template.clone(),
        comments_enabled: post.comments_enabled,
        comment_count,
        available_templates,
        featured_image_id: post.featured_image_id.map(|id| id.to_string()),
        featured_image_url,
        post_password_set: post.post_password.is_some(),
        author_name,
        author_is_active,
        author_id: post.author_id.to_string(),
        site_name,
        site_id: post.site_id.map(|id| id.to_string()).unwrap_or_default(),
        parent_id: post.parent_id.map(|id| id.to_string()),
        available_parents,
        sources: serde_json::from_value(post.sources.clone()).unwrap_or_default(),
        sources_public: post.sources_public,
        live_url,
        preview_url,
        saved_forms: fetch_saved_forms(&state, site_id).await,
        saved_polls: fetch_saved_polls(&state, site_id).await,
        // post.site_id, not site_id (the viewing admin's current session
        // site) — a global admin can open a post belonging to a different
        // site than the one they're currently in, and forms are looked up
        // by slug scoped to a site, so using the admin's site here could
        // silently find nothing (or, if unlucky, a same-slugged form
        // belonging to the wrong site) instead of this post's real form.
        form_analytics: fetch_form_analytics(&state, post.site_id, &post.content).await,
        created_at: Some(post.created_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        updated_at: Some(post.updated_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        version: Some(post.updated_at.to_rfc3339()),
    };

    let flash = match success {
        Some("saved") => Some("Saved."),
        _ => None,
    };
    Html(admin::pages::posts::render_editor(&edit, flash, &ctx)).into_response()
}

#[derive(Deserialize)]
pub struct PostForm {
    /// Timestamp from when the edit form was loaded; prevents stale saves.
    pub expected_updated_at: Option<String>,
    pub title: String,
    pub slug: Option<String>,
    pub content: String,
    pub excerpt: Option<String>,
    pub status: String,
    pub post_type: String,
    pub published_at: Option<String>,
    pub template: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub featured_image_id: Option<String>,
    pub featured_image_url: Option<String>,
    /// "1" when the user clicked "Remove featured image". A plain empty
    /// `featured_image_id` is indistinguishable from "field not submitted"
    /// once serde_html_form collapses it to `None`, so clearing needs its
    /// own explicit signal.
    pub featured_image_cleared: Option<String>,
    /// "on" when the Protected checkbox is ticked.
    pub post_protected: Option<String>,
    /// Plain-text password from the admin form (never stored; hashed before insert/update).
    pub post_password: Option<String>,
    /// "on" when the Allow Comments checkbox is ticked, absent to disable.
    pub comments_enabled: Option<String>,
    /// UUID of the parent page, empty string = no parent.
    pub parent_id: Option<String>,
    /// JSON-encoded array of source URL strings, assembled by JS before submit.
    pub sources_json: Option<String>,
    /// "on" when the "Show sources on the live page" checkbox is ticked.
    pub sources_public: Option<String>,
}

/// Parse the `sources_json` form field into a list of source URLs.
/// Falls back to an empty list on missing/invalid JSON rather than failing
/// the whole save — a malformed sources field should never block publishing.
fn parse_sources(sources_json: Option<&str>) -> Vec<String> {
    sources_json
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PostForm>,
) -> impl IntoResponse {
    if form.post_type == "page" && !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    // Authors may only save as draft or pending — clamp anything else to
    // draft — unless this specific author has been granted can_self_publish
    // (see PageContext::can_self_publish's doc comment), in which case they
    // behave like an Editor for status purposes.
    let status = if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && !admin.can_self_publish {
        match parse_status(&form.status) {
            PostStatus::Pending => PostStatus::Pending,
            _ => PostStatus::Draft,
        }
    } else {
        parse_status(&form.status)
    };
    let post_type = if form.post_type == "page" { PostType::Page } else { PostType::Post };
    let published_at = parse_datetime(form.published_at.as_deref());

    if matches!(status, PostStatus::Scheduled) && published_at.is_none() {
        return (axum::http::StatusCode::BAD_REQUEST, "A scheduled post requires a valid publication date and time.").into_response();
    }

    let form_comments_enabled = form.comments_enabled.as_deref() == Some("on");

    let form_parent_id: Option<Uuid> = form.parent_id.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<Uuid>().ok());

    if let Err(message) = validate_editor_references(&state, admin.site_id, form.featured_image_id.as_deref(), form_parent_id, None, form.post_type == "page").await {
        return (axum::http::StatusCode::BAD_REQUEST, message).into_response();
    }

    // Require content when publishing.
    if matches!(status, PostStatus::Published) && content_is_empty(&form.content) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
        let available_parents = if form.post_type == "page" { fetch_parent_options(&state, admin.site_id, None).await } else { vec![] };
        let edit = PostEdit {
            id: None,
            title: form.title,
            slug: form.slug.unwrap_or_default(),
            content: form.content,
            excerpt: form.excerpt.unwrap_or_default(),
            status: form.status,
            published_at: form.published_at,
            post_type: form.post_type.clone(),
            categories,
            tags,
            selected_categories: form.categories,
            selected_tags: form.tags,
            template: form.template.clone().filter(|s| !s.is_empty()),
            available_templates: if form.post_type == "page" { scan_templates(&state, admin.site_id) } else { vec![] },
            featured_image_id: form.featured_image_id.clone(),
            featured_image_url: form.featured_image_url.clone(),
            post_password_set: false,
            comments_enabled: form_comments_enabled,
            comment_count: 0,
            author_name: String::new(),
            author_is_active: true,
            author_id: String::new(),
            site_name: String::new(),
            site_id: String::new(),
            parent_id: form.parent_id.clone().filter(|s| !s.is_empty()),
            available_parents,
            sources: parse_sources(form.sources_json.as_deref()),
            sources_public: form.sources_public.as_deref() == Some("on"),
            live_url: None,
            preview_url: None,
            saved_forms: fetch_saved_forms(&state, admin.site_id).await,
        saved_polls: fetch_saved_polls(&state, admin.site_id).await,
            form_analytics: vec![],
            created_at: None,
            updated_at: None,
            version: None,
        };
        return Html(admin::pages::posts::render_editor(&edit, Some("Content is required before publishing."), &ctx)).into_response();
    }

    let post_password_hash = if form.post_protected.as_deref() == Some("on") {
        form.post_password.as_deref()
            .filter(|s| !s.is_empty())
            .and_then(|pw| crate::models::user::hash_password(pw).ok())
    } else {
        None
    };

    let create = CreatePost {
        site_id: admin.site_id,
        title: form.title.clone(),
        slug: form.slug.clone().filter(|s| !s.is_empty()).map(|s| crate::utils::slugify::slugify(&s)),
        content: form.content.clone(),
        content_format: Some("html".into()),
        excerpt: form.excerpt.clone().filter(|s| !s.is_empty()),
        status,
        post_type,
        author_id: admin.user.id,
        featured_image_id: form.featured_image_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()),
        published_at,
        template: form.template.clone().filter(|s| !s.is_empty()),
        post_password_hash,
        comments_enabled: form_comments_enabled,
        parent_id: form_parent_id,
        sources: parse_sources(form.sources_json.as_deref()),
        sources_public: form.sources_public.as_deref() == Some("on"),
    };

    match crate::models::post::create(&state.db, &create).await {
        Ok(post) => {
            save_post_terms(&state, post.id, post.site_id, &form.categories, &form.tags).await;
            if post.status == "published" {
                crate::search::indexer::index_post(&state.search_index, &post);
            }
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            Redirect::to(redirect).into_response()
        }
        Err(e) => {
            tracing::error!("create post error: {:?}", e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
            let available_parents = if form.post_type == "page" { fetch_parent_options(&state, admin.site_id, None).await } else { vec![] };
            let edit = PostEdit {
                id: None,
                title: form.title,
                slug: form.slug.unwrap_or_default(),
                content: form.content,
                excerpt: form.excerpt.unwrap_or_default(),
                status: form.status,
                published_at: form.published_at,
                post_type: form.post_type.clone(),
                categories,
                tags,
                selected_categories: form.categories,
                selected_tags: form.tags,
                template: form.template.clone().filter(|s| !s.is_empty()),
                available_templates: if form.post_type == "page" { scan_templates(&state, admin.site_id) } else { vec![] },
                featured_image_id: form.featured_image_id,
                featured_image_url: form.featured_image_url,
                post_password_set: false,
                comments_enabled: form_comments_enabled,
                comment_count: 0,
                author_name: String::new(),
                author_is_active: true,
                author_id: String::new(),
                site_name: String::new(),
                site_id: String::new(),
                parent_id: form_parent_id.map(|id| id.to_string()),
                available_parents,
                sources: parse_sources(form.sources_json.as_deref()),
                sources_public: form.sources_public.as_deref() == Some("on"),
                live_url: None,
                preview_url: None,
                saved_forms: fetch_saved_forms(&state, admin.site_id).await,
        saved_polls: fetch_saved_polls(&state, admin.site_id).await,
                form_analytics: vec![],
                created_at: None,
                updated_at: None,
                version: None,
            };
            let msg = friendly_save_error(&e);
            Html(admin::pages::posts::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<PostForm>,
) -> impl IntoResponse {
    let redirect = if form.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
    if form.post_type == "page" && !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    if let Some(expected) = form.expected_updated_at.as_deref().filter(|s| !s.is_empty()) {
        let Ok(expected) = chrono::DateTime::parse_from_rfc3339(expected) else {
            return (axum::http::StatusCode::BAD_REQUEST, "This edit form is invalid. Please reload the page.").into_response();
        };
        let Ok(current) = crate::models::post::get_by_id(&state.db, id).await else {
            return Redirect::to(redirect).into_response();
        };
        if current.updated_at != expected.with_timezone(&chrono::Utc) {
            return (axum::http::StatusCode::CONFLICT,
                "This post was changed by another editor while you were editing. Reload the page before saving your changes.").into_response();
        }
    }
    // Site isolation: verify the post belongs to the admin's site before updating.
    if !admin.caps.is_global_admin {
        let post = crate::models::post::get_by_id(&state.db, id).await;
        match post {
            Ok(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to(redirect).into_response();
                }
                // Author restriction: authors can only edit their own posts,
                // and (unless can_self_publish) only while still
                // draft/pending — once an Editor publishes it, a restricted
                // author can no longer touch it. A self-publishing author
                // keeps editing their own published/scheduled work, same as
                // any other status, since they don't need an Editor's
                // involvement to begin with.
                if admin.site_role == Some(crate::models::site_user::SiteRole::Author) {
                    if p.author_id != admin.user.id {
                        return Redirect::to(redirect).into_response();
                    }
                    if !admin.can_self_publish && (p.status == "published" || p.status == "scheduled") {
                        return Redirect::to(redirect).into_response();
                    }
                }
            }
            Err(_) => return Redirect::to(redirect).into_response(),
        }
    }

    // Authors may only save as draft or pending — clamp anything else to
    // draft — unless this specific author has been granted can_self_publish
    // (see PageContext::can_self_publish's doc comment), in which case they
    // behave like an Editor for status purposes.
    let status = if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && !admin.can_self_publish {
        match parse_status(&form.status) {
            PostStatus::Pending => PostStatus::Pending,
            _ => PostStatus::Draft,
        }
    } else {
        parse_status(&form.status)
    };
    let published_at = parse_datetime(form.published_at.as_deref());
    let form_comments_enabled = form.comments_enabled.as_deref() == Some("on");

    if matches!(status, PostStatus::Scheduled) && published_at.is_none() {
        return (axum::http::StatusCode::BAD_REQUEST, "A scheduled post requires a valid publication date and time.").into_response();
    }
    let form_parent_id: Option<Uuid> = form.parent_id.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<Uuid>().ok());

    if let Err(message) = validate_editor_references(&state, admin.site_id, form.featured_image_id.as_deref(), form_parent_id, Some(id), form.post_type == "page").await {
        return (axum::http::StatusCode::BAD_REQUEST, message).into_response();
    }

    // Require content when publishing.
    if matches!(status, PostStatus::Published) && content_is_empty(&form.content) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
        let available_parents = if form.post_type == "page" { fetch_parent_options(&state, admin.site_id, Some(id)).await } else { vec![] };
        let (author_name, author_is_active, author_id) = resolve_post_author(&state, id).await;
        let edit = PostEdit {
            id: Some(id.to_string()),
            title: form.title,
            slug: form.slug.unwrap_or_default(),
            content: form.content,
            excerpt: form.excerpt.unwrap_or_default(),
            status: form.status,
            published_at: form.published_at,
            post_type: form.post_type.clone(),
            categories,
            tags,
            selected_categories: form.categories,
            selected_tags: form.tags,
            template: form.template.clone().filter(|s| !s.is_empty()),
            available_templates: if form.post_type == "page" { scan_templates(&state, admin.site_id) } else { vec![] },
            featured_image_id: form.featured_image_id.clone(),
            featured_image_url: form.featured_image_url.clone(),
            post_password_set: false,
            comments_enabled: form_comments_enabled,
            comment_count: 0,
            author_name,
            author_is_active,
            author_id,
            site_name: cs.clone(),
            site_id: admin.site_id.map(|id| id.to_string()).unwrap_or_default(),
            parent_id: form.parent_id.clone().filter(|s| !s.is_empty()),
            available_parents,
            sources: parse_sources(form.sources_json.as_deref()),
            sources_public: form.sources_public.as_deref() == Some("on"),
            live_url: None,
            preview_url: None,
            saved_forms: fetch_saved_forms(&state, admin.site_id).await,
        saved_polls: fetch_saved_polls(&state, admin.site_id).await,
            form_analytics: vec![],
            created_at: None,
            updated_at: None,
            version: None,
        };
        return Html(admin::pages::posts::render_editor(&edit, Some("Content is required before publishing."), &ctx)).into_response();
    }

    let (clear_post_password, new_post_password_hash) =
        if form.post_protected.as_deref() == Some("on") {
            let new_hash = form.post_password.as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|pw| crate::models::user::hash_password(pw).ok());
            (false, new_hash) // keep existing if no new password typed
        } else {
            (true, None) // unchecked = clear
        };

    let update = UpdatePost {
        title: Some(form.title.clone()),
        slug: Some(match form.slug.as_deref().map(str::trim) {
            Some(s) if !s.is_empty() => crate::utils::slugify::slugify(s),
            _ => crate::utils::slugify::slugify(&form.title),
        }),
        content: Some(form.content.clone()),
        content_format: None,
        excerpt: form.excerpt.clone(),
        status: Some(status),
        clear_featured_image: form.featured_image_cleared.as_deref() == Some("1"),
        featured_image_id: form.featured_image_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()),
        published_at,
        template: form.template.clone().filter(|s| !s.is_empty()),
        clear_post_password,
        new_post_password_hash,
        comments_enabled: Some(form_comments_enabled),
        // Some(None) clears parent; Some(Some(id)) sets it; None leaves unchanged
        parent_id: Some(form_parent_id),
        sources: Some(parse_sources(form.sources_json.as_deref())),
        sources_public: Some(form.sources_public.as_deref() == Some("on")),
    };

    match crate::models::post::update(&state.db, id, &update).await {
        Ok(post) => {
            save_post_terms(&state, post.id, post.site_id, &form.categories, &form.tags).await;
            if post.status == "published" {
                crate::search::indexer::index_post(&state.search_index, &post);
            } else {
                crate::search::indexer::delete_post(&state.search_index, &post.id.to_string());
            }
            let redirect = if post.post_type == "page" {
                format!("/admin/pages/{}/edit?success=saved", post.id)
            } else {
                format!("/admin/posts/{}/edit?success=saved", post.id)
            };
            Redirect::to(&redirect).into_response()
        }
        Err(e) => {
            tracing::error!("update post {} error: {:?}", id, e);
            let cs = state.site_hostname(admin.site_id);
            let ctx = super::page_ctx_full(&state, &admin, &cs).await;
            let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
            let post_terms = crate::models::taxonomy::for_post(&state.db, id).await.unwrap_or_else(|_| vec![]);
            let selected_categories: Vec<String> = post_terms.iter()
                .filter(|t| t.taxonomy == "category")
                .map(|t| t.id.to_string())
                .collect();
            let selected_tags: Vec<String> = post_terms.iter()
                .filter(|t| t.taxonomy == "tag")
                .map(|t| t.id.to_string())
                .collect();
            let available_parents = if form.post_type == "page" { fetch_parent_options(&state, admin.site_id, Some(id)).await } else { vec![] };
            let (author_name, author_is_active, author_id) = resolve_post_author(&state, id).await;
            let edit = PostEdit {
                id: Some(id.to_string()),
                title: form.title,
                slug: form.slug.unwrap_or_default(),
                content: form.content,
                excerpt: form.excerpt.unwrap_or_default(),
                status: form.status,
                published_at: form.published_at,
                post_type: form.post_type.clone(),
                categories,
                tags,
                selected_categories,
                selected_tags,
                template: form.template.clone().filter(|s| !s.is_empty()),
                available_templates: if form.post_type == "page" { scan_templates(&state, admin.site_id) } else { vec![] },
                featured_image_id: form.featured_image_id,
                featured_image_url: form.featured_image_url,
                post_password_set: form.post_protected.as_deref() == Some("on"),
                comments_enabled: form_comments_enabled,
                comment_count: 0,
                author_name,
                author_is_active,
                author_id,
                site_name: cs.clone(),
                site_id: admin.site_id.map(|id| id.to_string()).unwrap_or_default(),
                parent_id: form_parent_id.map(|id| id.to_string()),
                available_parents,
                sources: parse_sources(form.sources_json.as_deref()),
                sources_public: form.sources_public.as_deref() == Some("on"),
                live_url: None,
                preview_url: None,
                saved_forms: fetch_saved_forms(&state, admin.site_id).await,
        saved_polls: fetch_saved_polls(&state, admin.site_id).await,
                form_analytics: vec![],
                created_at: None,
                updated_at: None,
                version: None,
            };
            let msg = friendly_save_error(&e);
            Html(admin::pages::posts::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

/// POST /admin/api/posts/{id}/sources-public — auto-save the "Show sources on
/// the live page" toggle without requiring a full post form submission.
pub async fn api_set_sources_public(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let public = body.get("public").and_then(|v| v.as_bool()).unwrap_or(false);

    let post = match crate::models::post::get_by_id(&state.db, id).await {
        Ok(p) => p,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Not found"}))).into_response(),
    };
    if !admin.caps.is_global_admin && post.site_id != admin.site_id {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
    }
    if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && post.author_id != admin.user.id {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
    }

    let update = UpdatePost {
        title: None,
        slug: None,
        content: None,
        content_format: None,
        excerpt: None,
        status: None,
        featured_image_id: None,
        clear_featured_image: false,
        published_at: None,
        template: None,
        clear_post_password: false,
        new_post_password_hash: None,
        comments_enabled: None,
        parent_id: None,
        sources: None,
        sources_public: Some(public),
    };

    match crate::models::post::update(&state.db, id, &update).await {
        Ok(_) => axum::Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => {
            tracing::error!("failed to update sources_public for post {}: {:?}", id, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "Update failed"}))).into_response()
        }
    }
}

/// POST /admin/api/posts/{id}/sources — save just the source URL list (and
/// the public-visibility toggle) without requiring a full post form submit.
/// Mirrors api_set_sources_public but also carries the source URLs — the
/// Sources card sits below the whole post form on the page, so a dedicated
/// save action there is more discoverable than expecting the far-away main
/// Save button to cover it too.
pub async fn api_set_sources(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let sources: Vec<String> = body.get("sources")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let public = body.get("public").and_then(|v| v.as_bool()).unwrap_or(false);

    let post = match crate::models::post::get_by_id(&state.db, id).await {
        Ok(p) => p,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Not found"}))).into_response(),
    };
    if !admin.caps.is_global_admin && post.site_id != admin.site_id {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
    }
    if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && post.author_id != admin.user.id {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden"}))).into_response();
    }

    let update = UpdatePost {
        title: None,
        slug: None,
        content: None,
        content_format: None,
        excerpt: None,
        status: None,
        featured_image_id: None,
        clear_featured_image: false,
        published_at: None,
        template: None,
        clear_post_password: false,
        new_post_password_hash: None,
        comments_enabled: None,
        parent_id: None,
        sources: Some(sources),
        sources_public: Some(public),
    };

    match crate::models::post::update(&state.db, id, &update).await {
        Ok(_) => axum::Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => {
            tracing::error!("failed to update sources for post {}: {:?}", id, e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "Update failed"}))).into_response()
        }
    }
}

pub async fn delete_post(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let post = crate::models::post::get_by_id(&state.db, id).await.ok();
    if !admin.caps.is_global_admin {
        match &post {
            Some(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to("/admin/posts").into_response();
                }
                if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.author_id != admin.user.id {
                    return Redirect::to("/admin/posts").into_response();
                }
                if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.status == "published" {
                    return Redirect::to("/admin/posts").into_response();
                }
            }
            None => return Redirect::to("/admin/posts").into_response(),
        }
    }
    if let Err(e) = crate::models::post::delete(&state.db, id).await {
        tracing::error!("failed to delete post {}: {:?}", id, e);
    } else if let Some(p) = &post {
        super::audit(&state, &admin, "post.deleted", "post", Some(id), &p.title, p.site_id).await;
    }
    crate::search::indexer::delete_post(&state.search_index, &id.to_string());
    Redirect::to("/admin/posts").into_response()
}

pub async fn delete_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    let page = crate::models::post::get_by_id(&state.db, id).await.ok();
    if !admin.caps.is_global_admin {
        match &page {
            Some(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to("/admin/pages").into_response();
                }
                if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.author_id != admin.user.id {
                    return Redirect::to("/admin/pages").into_response();
                }
                if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.status == "published" {
                    return Redirect::to("/admin/pages").into_response();
                }
            }
            None => return Redirect::to("/admin/pages").into_response(),
        }
    }
    if let Err(e) = crate::models::post::delete(&state.db, id).await {
        tracing::error!("failed to delete page {}: {:?}", id, e);
    } else if let Some(p) = &page {
        super::audit(&state, &admin, "page.deleted", "page", Some(id), &p.title, p.site_id).await;
    }
    crate::search::indexer::delete_post(&state.search_index, &id.to_string());
    Redirect::to("/admin/pages").into_response()
}

#[derive(Deserialize)]
pub struct BulkDeleteForm {
    #[serde(default)]
    pub ids: String, // comma-separated UUIDs
}

pub async fn bulk_delete_posts(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<BulkDeleteForm>,
) -> impl IntoResponse {
    let ids: Vec<String> = form.ids.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    bulk_delete_type(state, admin, ids, "/admin/posts").await
}

pub async fn bulk_delete_pages(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<BulkDeleteForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_pages {
        return Redirect::to("/admin").into_response();
    }
    let ids: Vec<String> = form.ids.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    bulk_delete_type(state, admin, ids, "/admin/pages").await.into_response()
}

async fn bulk_delete_type(state: AppState, admin: AdminUser, ids: Vec<String>, redirect: &str) -> impl IntoResponse {
    let kind = if redirect == "/admin/pages" { "page" } else { "post" };
    for raw_id in &ids {
        let id = match raw_id.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => continue,
        };
        let post = crate::models::post::get_by_id(&state.db, id).await.ok();
        // Apply same per-post permission checks as single delete.
        if !admin.caps.is_global_admin {
            match &post {
                Some(p) => {
                    if p.site_id != admin.site_id { continue; }
                    if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.author_id != admin.user.id { continue; }
                    if admin.site_role == Some(crate::models::site_user::SiteRole::Author) && p.status == "published" { continue; }
                }
                None => continue,
            }
        }
        if let Err(e) = crate::models::post::delete(&state.db, id).await {
            tracing::error!("bulk delete: failed to delete post {}: {:?}", id, e);
        } else {
            crate::search::indexer::delete_post(&state.search_index, &id.to_string());
            if let Some(p) = &post {
                super::audit(&state, &admin, &format!("{kind}.deleted"), kind, Some(id), &p.title, p.site_id).await;
            }
        }
    }
    Redirect::to(redirect).into_response()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Fetch (id, title) pairs of published pages for the parent selector dropdown.
/// Excludes the page being edited (exclude_id) to prevent a page being its own parent.
async fn fetch_parent_options(
    state: &AppState,
    site_id: Option<Uuid>,
    exclude_id: Option<Uuid>,
) -> Vec<(String, String)> {
    let pages = crate::models::post::get_published_pages_by_site(&state.db, site_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to fetch parent page options: {:?}", e);
            vec![]
        });
    pages.into_iter()
        .filter(|p| exclude_id.map_or(true, |ex| p.id != ex))
        .map(|p| (p.id.to_string(), p.title.clone()))
        .collect()
}

async fn fetch_term_options(state: &AppState, site_id: Option<Uuid>) -> (Vec<TermOption>, Vec<TermOption>) {
    let cats = crate::models::taxonomy::list(&state.db, site_id, TaxonomyType::Category).await.unwrap_or_else(|e| {
        tracing::warn!("failed to fetch category options: {:?}", e);
        vec![]
    });
    let tags = crate::models::taxonomy::list(&state.db, site_id, TaxonomyType::Tag).await.unwrap_or_else(|e| {
        tracing::warn!("failed to fetch tag options: {:?}", e);
        vec![]
    });
    let cat_opts = cats.iter().map(|t| TermOption { id: t.id.to_string(), name: t.name.clone() }).collect();
    let tag_opts = tags.iter().map(|t| TermOption { id: t.id.to_string(), name: t.name.clone() }).collect();
    (cat_opts, tag_opts)
}

/// (slug, name) pairs for every saved form on this site — powers the
/// editor's "Insert Form" picker. Empty (not an error) for global/private
/// theme contexts with no site_id, or sites with no forms defined yet.
async fn fetch_saved_forms(state: &AppState, site_id: Option<Uuid>) -> Vec<(String, String)> {
    let Some(site_id) = site_id else { return vec![] };
    crate::models::form_def::list_for_site(&state.db, site_id).await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to fetch saved forms: {:?}", e);
            vec![]
        })
        .into_iter()
        .map(|f| (f.slug, f.name))
        .collect()
}

/// (slug, name) pairs for every saved poll on this site — powers the
/// editor's "Insert Poll" picker. Mirrors `fetch_saved_forms`.
async fn fetch_saved_polls(state: &AppState, site_id: Option<Uuid>) -> Vec<(String, String)> {
    let Some(site_id) = site_id else { return vec![] };
    crate::models::poll_def::list_for_site(&state.db, site_id).await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to fetch saved polls: {:?}", e);
            vec![]
        })
        .into_iter()
        .map(|p| (p.slug, p.name))
        .collect()
}

/// (form slug, form name, submission count) for every distinct saved-form
/// embed found in `content` — powers the editor sidebar's "Form Analytics"
/// section and the Publish Options pill's "view results" link (only shown
/// when non-empty, see render_editor). slug (not id) is what the results
/// page at /admin/form-data-analytics/{slug} actually takes as its path
/// param — same identifier form_submissions.form_name stores, despite that
/// column being misleadingly named "name". A form embed referencing a
/// deleted/missing form is silently skipped, same convention as
/// form_def::expand_embeds.
async fn fetch_form_analytics(state: &AppState, site_id: Option<Uuid>, content: &str) -> Vec<(String, String, i64)> {
    let Some(site_id) = site_id else { return vec![] };
    if !content.contains("<ss-form") {
        return vec![];
    }
    let Ok(re) = regex_lite::Regex::new(r#"<ss-form\b[^>]*data-slug="([^"]*)"[^>]*>"#) else {
        return vec![];
    };
    let mut slugs: Vec<String> = re.captures_iter(content).map(|c| c[1].to_string()).collect();
    slugs.sort();
    slugs.dedup();

    let mut results = Vec::with_capacity(slugs.len());
    for slug in slugs {
        let Ok(Some(form)) = crate::models::form_def::get_by_slug(&state.db, site_id, &slug).await else {
            continue;
        };
        let count = crate::models::form_submission::count_for_form(&state.db, site_id, &slug)
            .await
            .unwrap_or(0);
        results.push((form.slug, form.name, count));
    }
    results
}

/// Scan the active theme's templates/ directory for available templates.
/// Returns paths relative to templates/ without the .html extension,
/// e.g. ["forms/contact", "forms/newsletter", "landing"].
/// Excludes base.html (layout file, not a standalone template).
fn scan_templates(state: &AppState, site_id: Option<Uuid>) -> Vec<String> {
    let theme = state.active_theme_for_site(site_id);
    let themes_dir = &state.config.themes_dir;
    let sites_dir  = &state.config.sites_dir;

    // Check site-specific theme dir first, then global.
    let theme_dir = if let Some(sid) = site_id {
        let site_path = std::path::Path::new(sites_dir).join(sid.to_string()).join("themes").join(&theme);
        if site_path.is_dir() {
            site_path
        } else {
            std::path::Path::new(themes_dir).join("global").join(&theme)
        }
    } else {
        std::path::Path::new(themes_dir).join("global").join(&theme)
    };

    let templates_dir = theme_dir.join("templates");
    if !templates_dir.is_dir() {
        return vec![];
    }

    // Walk recursively, collect all .html files except reserved theme templates.
    // Standard theme templates (index, archive, single, search, 404, page, base, partials/*)
    // require Tera context variables that the page renderer does not supply, so they must
    // not appear as selectable page template overrides.
    const EXCLUDED: &[&str] = &[
        "base", "page", "index", "single", "archive", "search", "404",
    ];
    let mut results = Vec::new();
    fn walk(dir: &std::path::Path, base: &std::path::Path, results: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, results);
            } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
                if let Ok(rel) = path.strip_prefix(base) {
                    let s = rel.to_string_lossy();
                    let without_ext = s.trim_end_matches(".html").to_string();
                    let normalized = without_ext.replace('\\', "/");
                    // Skip reserved templates and anything inside partials/.
                    if !EXCLUDED.contains(&normalized.as_str()) && !normalized.starts_with("partials/") {
                        results.push(normalized);
                    }
                }
            }
        }
    }
    walk(&templates_dir, &templates_dir, &mut results);
    results.sort();
    results
}

async fn save_post_terms(state: &AppState, post_id: Uuid, site_id: Option<Uuid>, category_ids: &[String], tag_ids: &[String]) {
    let categories: Vec<Uuid> = category_ids.iter().filter_map(|s| s.parse().ok()).collect();
    let tags: Vec<Uuid> = tag_ids.iter().filter_map(|s| s.parse().ok()).collect();
    if let Err(e) = crate::models::taxonomy::replace_for_post(&state.db, post_id, site_id, &categories, &tags).await {
        tracing::error!("failed to replace terms for post {}: {:?}", post_id, e);
    }
}

fn friendly_save_error(e: &crate::errors::AppError) -> String {
    let s = e.to_string();
    if s.contains("duplicate key") || s.contains("unique") {
        "A post with that slug already exists. Please choose a different slug.".to_string()
    } else {
        "Failed to save post. Please try again.".to_string()
    }
}

async fn validate_editor_references(
    state: &AppState,
    site_id: Option<Uuid>,
    featured_image_id: Option<&str>,
    parent_id: Option<Uuid>,
    exclude_parent_id: Option<Uuid>,
    is_page: bool,
) -> Result<(), &'static str> {
    if let Some(raw) = featured_image_id.filter(|s| !s.is_empty()) {
        let Ok(id) = raw.parse::<Uuid>() else { return Err("The selected featured image is invalid."); };
        let Ok(media) = crate::models::media::get_by_id(&state.db, id).await else { return Err("The selected featured image could not be found."); };
        if media.site_id != site_id { return Err("The selected featured image does not belong to this site."); }
    }
    if let Some(parent_id) = parent_id {
        if !is_page || Some(parent_id) == exclude_parent_id { return Err("The selected parent page is invalid."); }
        let Ok(parent) = crate::models::post::get_by_id(&state.db, parent_id).await else { return Err("The selected parent page could not be found."); };
        if parent.site_id != site_id || parent.post_type != "page" || parent.status != "published" {
            return Err("The selected parent page does not belong to this site or is not published.");
        }
    }
    Ok(())
}

/// Returns true when the content is empty or contains only whitespace / blank
/// HTML tags (e.g. Quill's default `<p><br></p>`).
fn content_is_empty(html: &str) -> bool {
    // A saved-form embed (see FormEmbedBlot in posts.rs) is a self-closing
    // <ss-form data-slug="..."> tag with no text between it and its close —
    // stripping tags below would leave nothing behind and wrongly call the
    // page empty, even though it expands into a real form at render time.
    // Matches the exact same well-formed-embed pattern form_def::expand_embeds
    // looks for (not just a loose substring check), so typing the bare text
    // "<ss-form" without a real embed doesn't count as content.
    if let Ok(re) = regex_lite::Regex::new(r#"<ss-form\b[^>]*data-slug="[^"]*"[^>]*></ss-form>"#) {
        if re.is_match(html) {
            return false;
        }
    }
    // Strip every HTML tag and check if anything meaningful remains.
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().is_empty()
}

fn parse_status(s: &str) -> PostStatus {
    match s {
        "pending"   => PostStatus::Pending,
        "published" => PostStatus::Published,
        "scheduled" => PostStatus::Scheduled,
        "trashed"   => PostStatus::Trashed,
        _ => PostStatus::Draft,
    }
}

fn parse_datetime(s: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    s.filter(|s| !s.is_empty())
        .and_then(|s| {
            // datetime-local format: "2026-01-15T10:30"
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
                .ok()
                .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
        })
}
