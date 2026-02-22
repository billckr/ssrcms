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
    _admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> Html<String> {
    list_type(state, "post", q.page).await
}

pub async fn list_pages(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<PostsQuery>,
) -> Html<String> {
    list_type(state, "page", q.page).await
}

async fn list_type(state: AppState, post_type: &str, page: Option<i64>) -> Html<String> {
    let per_page = 20i64;
    let page = page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    let filter = ListFilter {
        status: None,
        post_type: Some(if post_type == "page" { PostType::Page } else { PostType::Post }),
        limit: per_page,
        offset,
        ..Default::default()
    };

    let raw = crate::models::post::list(&state.db, &filter).await.unwrap_or_default();
    let rows: Vec<PostRow> = raw.iter().map(|p| PostRow {
        id: p.id.to_string(),
        title: p.title.clone(),
        status: p.status.clone(),
        slug: p.slug.clone(),
        post_type: p.post_type.clone(),
        published_at: p.published_at.map(|d| d.format("%Y-%m-%d").to_string()),
    }).collect();

    Html(admin::pages::posts::render_list(&rows, post_type, None))
}

pub async fn new_post(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    new_post_type(state, "post").await
}

pub async fn new_page(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    new_post_type(state, "page").await
}

async fn new_post_type(state: AppState, post_type: &str) -> Html<String> {
    let (categories, tags) = fetch_term_options(&state).await;
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
    };
    Html(admin::pages::posts::render_editor(&edit, None))
}

pub async fn edit_post(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    edit_post_type(state, id).await
}

pub async fn edit_page(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    edit_post_type(state, id).await
}

async fn edit_post_type(state: AppState, id: Uuid) -> impl IntoResponse {
    let post = match crate::models::post::get_by_id(&state.db, id).await {
        Ok(p) => p,
        Err(_) => return Redirect::to("/admin/posts").into_response(),
    };

    let (categories, tags) = fetch_term_options(&state).await;

    // Fetch current taxonomies for this post.
    let post_terms = crate::models::taxonomy::for_post(&state.db, id).await.unwrap_or_default();
    let selected_categories: Vec<String> = post_terms.iter()
        .filter(|t| t.taxonomy == "category")
        .map(|t| t.id.to_string())
        .collect();
    let selected_tags: Vec<String> = post_terms.iter()
        .filter(|t| t.taxonomy == "tag")
        .map(|t| t.id.to_string())
        .collect();

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
    };

    Html(admin::pages::posts::render_editor(&edit, None)).into_response()
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
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub categories: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub tags: Vec<String>,
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<PostForm>,
) -> impl IntoResponse {
    let status = parse_status(&form.status);
    let post_type = if form.post_type == "page" { PostType::Page } else { PostType::Post };
    let published_at = parse_datetime(form.published_at.as_deref());

    let create = CreatePost {
        title: form.title,
        slug: form.slug.filter(|s| !s.is_empty()),
        content: form.content,
        content_format: Some("html".into()),
        excerpt: form.excerpt.filter(|s| !s.is_empty()),
        status,
        post_type,
        author_id: admin.user.id,
        featured_image_id: None,
        published_at,
    };

    match crate::models::post::create(&state.db, &create).await {
        Ok(post) => {
            // Save taxonomies.
            save_post_terms(&state, post.id, &form.categories, &form.tags).await;
            // Index in Tantivy if published.
            if post.status == "published" {
                crate::search::indexer::index_post(&state.search_index, &post);
            }
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            Redirect::to(redirect).into_response()
        }
        Err(e) => {
            tracing::error!("create post error: {}", e);
            Html(format!("<p>Error: {}</p>", e)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<PostForm>,
) -> impl IntoResponse {
    let status = parse_status(&form.status);
    let published_at = parse_datetime(form.published_at.as_deref());

    let update = UpdatePost {
        title: Some(form.title),
        slug: form.slug.filter(|s| !s.is_empty()),
        content: Some(form.content),
        content_format: None,
        excerpt: form.excerpt,
        status: Some(status),
        featured_image_id: None,
        published_at,
    };

    match crate::models::post::update(&state.db, id, &update).await {
        Ok(post) => {
            // Update taxonomies: clear then re-attach.
            save_post_terms(&state, post.id, &form.categories, &form.tags).await;
            // Re-index if published, remove from index otherwise.
            if post.status == "published" {
                crate::search::indexer::index_post(&state.search_index, &post);
            } else {
                crate::search::indexer::delete_post(&state.search_index, &post.id.to_string());
            }
            let redirect = if post.post_type == "page" { "/admin/pages" } else { "/admin/posts" };
            Redirect::to(redirect).into_response()
        }
        Err(e) => {
            tracing::error!("update post error: {}", e);
            Html(format!("<p>Error: {}</p>", e)).into_response()
        }
    }
}

pub async fn delete_post(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = crate::models::post::delete(&state.db, id).await;
    crate::search::indexer::delete_post(&state.search_index, &id.to_string());
    Redirect::to("/admin/posts")
}

pub async fn delete_page(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = crate::models::post::delete(&state.db, id).await;
    crate::search::indexer::delete_post(&state.search_index, &id.to_string());
    Redirect::to("/admin/pages")
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn fetch_term_options(state: &AppState) -> (Vec<TermOption>, Vec<TermOption>) {
    let cats = crate::models::taxonomy::list(&state.db, TaxonomyType::Category).await.unwrap_or_default();
    let tags = crate::models::taxonomy::list(&state.db, TaxonomyType::Tag).await.unwrap_or_default();
    let cat_opts = cats.iter().map(|t| TermOption { id: t.id.to_string(), name: t.name.clone() }).collect();
    let tag_opts = tags.iter().map(|t| TermOption { id: t.id.to_string(), name: t.name.clone() }).collect();
    (cat_opts, tag_opts)
}

async fn save_post_terms(state: &AppState, post_id: Uuid, category_ids: &[String], tag_ids: &[String]) {
    // Detach all current terms first.
    let current = crate::models::taxonomy::for_post(&state.db, post_id).await.unwrap_or_default();
    for term in &current {
        let _ = crate::models::taxonomy::detach_from_post(&state.db, post_id, term.id).await;
    }
    // Attach selected.
    for id_str in category_ids {
        if let Ok(id) = id_str.parse::<Uuid>() {
            let _ = crate::models::taxonomy::attach_to_post(&state.db, post_id, id).await;
        }
    }
    for id_str in tag_ids {
        if let Ok(id) = id_str.parse::<Uuid>() {
            let _ = crate::models::taxonomy::attach_to_post(&state.db, post_id, id).await;
        }
    }
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
