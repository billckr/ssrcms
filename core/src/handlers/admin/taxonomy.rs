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
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    list_terms(state, "category", admin.site_id, cs, admin.is_global_admin, admin.user.email.clone()).await
}

pub async fn tags(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    list_terms(state, "tag", admin.site_id, cs, admin.is_global_admin, admin.user.email.clone()).await
}

async fn list_terms(state: AppState, taxonomy: &str, site_id: Option<Uuid>, current_site: String, is_global_admin: bool, user_email: String) -> Html<String> {
    let tax_type = if taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
    let raw = crate::models::taxonomy::list(&state.db, site_id, tax_type).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list {} terms: {:?}", taxonomy, e);
        vec![]
    });
    let mut items: Vec<TermItem> = Vec::new();
    for t in &raw {
        let count = crate::models::taxonomy::post_count(&state.db, t.id).await.unwrap_or_else(|e| {
            tracing::warn!("failed to get post count for term {}: {:?}", t.id, e);
            0
        });
        items.push(TermItem {
            id: t.id.to_string(),
            name: t.name.clone(),
            slug: t.slug.clone(),
            post_count: count,
        });
    }
    Html(admin::pages::taxonomy::render(&items, taxonomy, None, &current_site, is_global_admin, &user_email))
}

#[derive(Deserialize)]
pub struct TermForm {
    pub name: String,
    pub slug: Option<String>,
    pub taxonomy: String,
}

pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<TermForm>,
) -> impl IntoResponse {
    let tax_type = if form.taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
    let slug = form.slug
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slug::slugify(&form.name));
    let create = CreateTaxonomy {
        site_id: admin.site_id,
        name: form.name,
        slug,
        taxonomy: tax_type,
        description: None,
    };
    let redirect = if form.taxonomy == "category" { "/admin/categories" } else { "/admin/tags" };
    if let Err(e) = crate::models::taxonomy::create(&state.db, &create).await {
        tracing::error!("failed to create {} '{}': {:?}", form.taxonomy, create.name, e);
        // Re-render the list with a flash message rather than losing the user's input context
        let tax_type2 = if form.taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
        let raw = crate::models::taxonomy::list(&state.db, admin.site_id, tax_type2).await.unwrap_or_default();
        let mut items: Vec<TermItem> = Vec::new();
        for t in &raw {
            let count = crate::models::taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
            items.push(TermItem { id: t.id.to_string(), name: t.name.clone(), slug: t.slug.clone(), post_count: count });
        }
        let msg = if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
            format!("A {} with that name or slug already exists.", form.taxonomy)
        } else {
            format!("Failed to create {}. Please try again.", form.taxonomy)
        };
        let cs = state.site_hostname(admin.site_id);
        return Html(admin::pages::taxonomy::render(&items, &form.taxonomy, Some(&msg), &cs, admin.is_global_admin, &admin.user.email)).into_response();
    }
    Redirect::to(redirect).into_response()
}

pub async fn delete_category(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.is_global_admin {
        let belongs = crate::models::taxonomy::get_by_id(&state.db, id).await
            .map(|t| t.site_id == admin.site_id)
            .unwrap_or(false);
        if !belongs {
            return Redirect::to("/admin/categories").into_response();
        }
    }
    if let Err(e) = crate::models::taxonomy::delete(&state.db, id).await {
        tracing::error!("failed to delete category {}: {:?}", id, e);
    }
    Redirect::to("/admin/categories").into_response()
}

pub async fn delete_tag(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.is_global_admin {
        let belongs = crate::models::taxonomy::get_by_id(&state.db, id).await
            .map(|t| t.site_id == admin.site_id)
            .unwrap_or(false);
        if !belongs {
            return Redirect::to("/admin/tags").into_response();
        }
    }
    if let Err(e) = crate::models::taxonomy::delete(&state.db, id).await {
        tracing::error!("failed to delete tag {}: {:?}", id, e);
    }
    Redirect::to("/admin/tags").into_response()
}
