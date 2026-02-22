use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::taxonomy::{CreateTaxonomy, TaxonomyType};
use admin::pages::taxonomy::TermItem;

pub async fn categories(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    list_terms(state, "category").await
}

pub async fn tags(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    list_terms(state, "tag").await
}

async fn list_terms(state: AppState, taxonomy: &str) -> Html<String> {
    let tax_type = if taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
    let raw = crate::models::taxonomy::list(&state.db, tax_type).await.unwrap_or_default();
    let mut items: Vec<TermItem> = Vec::new();
    for t in &raw {
        let count = crate::models::taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
        items.push(TermItem {
            id: t.id.to_string(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            post_count: count,
        });
    }
    Html(admin::pages::taxonomy::render(&items, taxonomy, None))
}

#[derive(Deserialize)]
pub struct TermForm {
    pub name: String,
    pub slug: Option<String>,
    pub taxonomy: String,
}

pub async fn create(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<TermForm>,
) -> impl IntoResponse {
    let tax_type = if form.taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
    let slug = form.slug
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slug::slugify(&form.name));
    let create = CreateTaxonomy {
        name: form.name,
        slug,
        taxonomy: tax_type,
        description: None,
    };
    let _ = crate::models::taxonomy::create(&state.db, &create).await;
    let redirect = if form.taxonomy == "category" { "/admin/categories" } else { "/admin/tags" };
    Redirect::to(redirect)
}

pub async fn delete_category(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = crate::models::taxonomy::delete(&state.db, id).await;
    Redirect::to("/admin/categories")
}

pub async fn delete_tag(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = crate::models::taxonomy::delete(&state.db, id).await;
    Redirect::to("/admin/tags")
}
