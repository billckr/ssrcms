use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
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
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let author_filter = if admin.site_role == "author" { Some(admin.user.id) } else { None };
    list_type(state, "post", q.page, admin.site_id, author_filter, ctx).await
}

pub async fn list_pages(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let author_filter = if admin.site_role == "author" { Some(admin.user.id) } else { None };
    list_type(state, "page", q.page, admin.site_id, author_filter, ctx).await
}

async fn list_type(state: AppState, post_type: &str, page: Option<i64>, site_id: Option<Uuid>, author_id: Option<Uuid>, ctx: admin::PageContext) -> Html<String> {
    let per_page = 20i64;
    let page = page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    // Count uses the same site_id + author_id filters so per-user totals are accurate.
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM posts
           WHERE ($1::uuid IS NULL OR site_id = $1)
             AND post_type = $2
             AND ($3::uuid IS NULL OR author_id = $3)"#,
    )
    .bind(site_id)
    .bind(post_type)
    .bind(author_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let total_pages = ((total + per_page - 1) / per_page).max(1);

    let filter = ListFilter {
        site_id,
        status: None,
        post_type: Some(if post_type == "page" { PostType::Page } else { PostType::Post }),
        author_id,
        limit: per_page,
        offset,
        ..Default::default()
    };

    let raw = crate::models::post::list(&state.db, &filter).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list {} items: {:?}", post_type, e);
        vec![]
    });
    let mut rows: Vec<PostRow> = Vec::new();

    for p in raw.iter() {
        let author_name = crate::models::user::get_by_id(&state.db, p.author_id)
            .await
            .map(|u| u.display_name)
            .unwrap_or_else(|e| {
                tracing::warn!("failed to fetch author {}: {:?}", p.author_id, e);
                "Unknown".to_string()
            });

        rows.push(PostRow {
            id: p.id.to_string(),
            title: p.title.clone(),
            status: p.status.clone(),
            slug: p.slug.clone(),
            post_type: p.post_type.clone(),
            author_name,
            published_at: p.published_at.map(|d| d.format("%Y-%m-%d").to_string()),
        });
    }

    Html(admin::pages::posts::render_list(&rows, post_type, page, total_pages, None, &ctx))
}

pub async fn new_post(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    new_post_type(state, "post", admin.site_id, ctx).await
}

pub async fn new_page(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    new_post_type(state, "page", admin.site_id, ctx).await
}

async fn new_post_type(state: AppState, post_type: &str, site_id: Option<Uuid>, ctx: admin::PageContext) -> Html<String> {
    let (categories, tags) = fetch_term_options(&state, site_id).await;
    let available_templates = if post_type == "page" { scan_templates(&state, site_id) } else { vec![] };
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
    };
    Html(admin::pages::posts::render_editor(&edit, None, &ctx))
}

pub async fn edit_post(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    edit_post_type(state, id, admin.site_id, admin.site_role == "author", admin.user.id, ctx).await
}

pub async fn edit_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    edit_post_type(state, id, admin.site_id, admin.site_role == "author", admin.user.id, ctx).await
}

async fn edit_post_type(state: AppState, id: Uuid, site_id: Option<Uuid>, is_author: bool, user_id: Uuid, ctx: admin::PageContext) -> impl IntoResponse {
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

    // Author restriction: authors can only edit their own unpublished content.
    if is_author {
        if post.author_id != user_id {
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            return Redirect::to(redirect).into_response();
        }
        if post.status == "published" {
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

    let featured_image_url = if let Some(img_id) = post.featured_image_id {
        crate::models::media::get_by_id(&state.db, img_id).await
            .ok()
            .map(|m| format!("/uploads/{}", m.path))
    } else {
        None
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
        available_templates,
        featured_image_id: post.featured_image_id.map(|id| id.to_string()),
        featured_image_url,
    };

    Html(admin::pages::posts::render_editor(&edit, None, &ctx)).into_response()
}

/// HTML forms send repeated keys for multiple checkboxes, but only a bare
/// string when exactly one box is checked.  This deserializer accepts both.
fn deserialize_string_or_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{SeqAccess, Visitor};
    use std::fmt;

    struct SovVisitor;

    impl<'de> Visitor<'de> for SovVisitor {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "a string or a sequence of strings")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_owned()])
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                out.push(s);
            }
            Ok(out)
        }
    }

    d.deserialize_any(SovVisitor)
}

#[derive(Deserialize)]
pub struct PostForm {
    pub title: String,
    pub slug: Option<String>,
    pub content: String,
    pub excerpt: Option<String>,
    pub status: String,
    pub post_type: String,
    pub published_at: Option<String>,
    pub template: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub categories: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub tags: Vec<String>,
    pub featured_image_id: Option<String>,
    pub featured_image_url: Option<String>,
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PostForm>,
) -> impl IntoResponse {
    let status = parse_status(&form.status);
    let post_type = if form.post_type == "page" { PostType::Page } else { PostType::Post };
    let published_at = parse_datetime(form.published_at.as_deref());

    // Require content when publishing.
    if matches!(status, PostStatus::Published) && content_is_empty(&form.content) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
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
        };
        return Html(admin::pages::posts::render_editor(&edit, Some("Content is required before publishing."), &ctx)).into_response();
    }

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
    };

    match crate::models::post::create(&state.db, &create).await {
        Ok(post) => {
            save_post_terms(&state, post.id, &form.categories, &form.tags).await;
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
    // Site isolation: verify the post belongs to the admin's site before updating.
    if !admin.caps.is_global_admin {
        let post = crate::models::post::get_by_id(&state.db, id).await;
        match post {
            Ok(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to(redirect).into_response();
                }
                // Author restriction: authors can only edit their own unpublished posts.
                if admin.site_role == "author" {
                    if p.author_id != admin.user.id {
                        return Redirect::to(redirect).into_response();
                    }
                    if p.status == "published" {
                        return Redirect::to(redirect).into_response();
                    }
                }
            }
            Err(_) => return Redirect::to(redirect).into_response(),
        }
    }

    let status = parse_status(&form.status);
    let published_at = parse_datetime(form.published_at.as_deref());

    // Require content when publishing.
    if matches!(status, PostStatus::Published) && content_is_empty(&form.content) {
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        let (categories, tags) = fetch_term_options(&state, admin.site_id).await;
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
        };
        return Html(admin::pages::posts::render_editor(&edit, Some("Content is required before publishing."), &ctx)).into_response();
    }

    let update = UpdatePost {
        title: Some(form.title.clone()),
        slug: form.slug.clone().filter(|s| !s.is_empty()).map(|s| crate::utils::slugify::slugify(&s)),
        content: Some(form.content.clone()),
        content_format: None,
        excerpt: form.excerpt.clone(),
        status: Some(status),
        clear_featured_image: form.featured_image_id.as_deref() == Some(""),
        featured_image_id: form.featured_image_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()),
        published_at,
        template: form.template.clone().filter(|s| !s.is_empty()),
    };

    match crate::models::post::update(&state.db, id, &update).await {
        Ok(post) => {
            save_post_terms(&state, post.id, &form.categories, &form.tags).await;
            if post.status == "published" {
                crate::search::indexer::index_post(&state.search_index, &post);
            } else {
                crate::search::indexer::delete_post(&state.search_index, &post.id.to_string());
            }
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            Redirect::to(redirect).into_response()
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
            };
            let msg = friendly_save_error(&e);
            Html(admin::pages::posts::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn delete_post(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.is_global_admin {
        match crate::models::post::get_by_id(&state.db, id).await {
            Ok(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to("/admin/posts").into_response();
                }
                if admin.site_role == "author" && p.author_id != admin.user.id {
                    return Redirect::to("/admin/posts").into_response();
                }
                if admin.site_role == "author" && p.status == "published" {
                    return Redirect::to("/admin/posts").into_response();
                }
            }
            Err(_) => return Redirect::to("/admin/posts").into_response(),
        }
    }
    if let Err(e) = crate::models::post::delete(&state.db, id).await {
        tracing::error!("failed to delete post {}: {:?}", id, e);
    }
    crate::search::indexer::delete_post(&state.search_index, &id.to_string());
    Redirect::to("/admin/posts").into_response()
}

pub async fn delete_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.is_global_admin {
        match crate::models::post::get_by_id(&state.db, id).await {
            Ok(p) => {
                if p.site_id != admin.site_id {
                    return Redirect::to("/admin/pages").into_response();
                }
                if admin.site_role == "author" && p.author_id != admin.user.id {
                    return Redirect::to("/admin/pages").into_response();
                }
                if admin.site_role == "author" && p.status == "published" {
                    return Redirect::to("/admin/pages").into_response();
                }
            }
            Err(_) => return Redirect::to("/admin/pages").into_response(),
        }
    }
    if let Err(e) = crate::models::post::delete(&state.db, id).await {
        tracing::error!("failed to delete page {}: {:?}", id, e);
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
    let ids: Vec<String> = form.ids.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    bulk_delete_type(state, admin, ids, "/admin/pages").await
}

async fn bulk_delete_type(state: AppState, admin: AdminUser, ids: Vec<String>, redirect: &str) -> impl IntoResponse {
    for raw_id in &ids {
        let id = match raw_id.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => continue,
        };
        // Apply same per-post permission checks as single delete.
        if !admin.caps.is_global_admin {
            match crate::models::post::get_by_id(&state.db, id).await {
                Ok(p) => {
                    if p.site_id != admin.site_id { continue; }
                    if admin.site_role == "author" && p.author_id != admin.user.id { continue; }
                    if admin.site_role == "author" && p.status == "published" { continue; }
                }
                Err(_) => continue,
            }
        }
        if let Err(e) = crate::models::post::delete(&state.db, id).await {
            tracing::error!("bulk delete: failed to delete post {}: {:?}", id, e);
        } else {
            crate::search::indexer::delete_post(&state.search_index, &id.to_string());
        }
    }
    Redirect::to(redirect).into_response()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

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

/// Scan the active theme's templates/ directory for available templates.
/// Returns paths relative to templates/ without the .html extension,
/// e.g. ["forms/contact", "forms/newsletter", "landing"].
/// Excludes base.html (layout file, not a standalone template).
fn scan_templates(state: &AppState, site_id: Option<Uuid>) -> Vec<String> {
    let theme = state.active_theme_for_site(site_id);
    let themes_dir = &state.config.themes_dir;

    // Check site-specific theme dir first, then global.
    let theme_dir = if let Some(sid) = site_id {
        let site_path = std::path::Path::new(themes_dir).join("sites").join(sid.to_string()).join(&theme);
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

async fn save_post_terms(state: &AppState, post_id: Uuid, category_ids: &[String], tag_ids: &[String]) {
    let current = crate::models::taxonomy::for_post(&state.db, post_id).await.unwrap_or_else(|e| {
        tracing::warn!("failed to fetch terms for post {}: {:?}", post_id, e);
        vec![]
    });
    for term in &current {
        if let Err(e) = crate::models::taxonomy::detach_from_post(&state.db, post_id, term.id).await {
            tracing::warn!("failed to detach term {} from post {}: {:?}", term.id, post_id, e);
        }
    }
    for id_str in category_ids {
        if let Ok(id) = id_str.parse::<Uuid>() {
            if let Err(e) = crate::models::taxonomy::attach_to_post(&state.db, post_id, id).await {
                tracing::warn!("failed to attach category {} to post {}: {:?}", id, post_id, e);
            }
        }
    }
    for id_str in tag_ids {
        if let Ok(id) = id_str.parse::<Uuid>() {
            if let Err(e) = crate::models::taxonomy::attach_to_post(&state.db, post_id, id).await {
                tracing::warn!("failed to attach tag {} to post {}: {:?}", id, post_id, e);
            }
        }
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

/// Returns true when the content is empty or contains only whitespace / blank
/// HTML tags (e.g. Quill's default `<p><br></p>`).
fn content_is_empty(html: &str) -> bool {
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
        "published" => PostStatus::Published,
        "scheduled" => PostStatus::Scheduled,
        "trashed" => PostStatus::Trashed,
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
