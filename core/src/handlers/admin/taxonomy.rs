use axum::{
    extract::{Path, State},
    http::StatusCode,
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
) -> impl IntoResponse {
    if !admin.caps.can_manage_taxonomies {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    list_terms(state, "category", admin.site_id, ctx).await.into_response()
}

pub async fn tags(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_taxonomies {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    list_terms(state, "tag", admin.site_id, ctx).await.into_response()
}

async fn list_terms(state: AppState, taxonomy: &str, site_id: Option<Uuid>, ctx: admin::PageContext) -> Html<String> {
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
    Html(admin::pages::taxonomy::render(&items, taxonomy, None, &ctx))
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
    if !admin.caps.can_manage_taxonomies {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let tax_type = if form.taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
    let slug = form.slug
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::utils::slugify::slugify(&form.name));

    if !crate::utils::slugify::is_valid_slug(&slug) {
        let tax_type2 = if form.taxonomy == "category" { TaxonomyType::Category } else { TaxonomyType::Tag };
        let raw = crate::models::taxonomy::list(&state.db, admin.site_id, tax_type2).await.unwrap_or_default();
        let mut items: Vec<TermItem> = Vec::new();
        for t in &raw {
            let count = crate::models::taxonomy::post_count(&state.db, t.id).await.unwrap_or(0);
            items.push(TermItem { id: t.id.to_string(), name: t.name.clone(), slug: t.slug.clone(), post_count: count });
        }
        let cs = state.site_hostname(admin.site_id);
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return Html(admin::pages::taxonomy::render(&items, &form.taxonomy, Some("Slug must be lowercase letters, numbers, and hyphens only — no spaces."), &ctx)).into_response();
    }
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
        let ctx = super::page_ctx_full(&state, &admin, &cs).await;
        return Html(admin::pages::taxonomy::render(&items, &form.taxonomy, Some(&msg), &ctx)).into_response();
    }
    Redirect::to(redirect).into_response()
}

pub async fn delete_category(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_taxonomies {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if !admin.caps.is_global_admin {
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
    if !admin.caps.can_manage_taxonomies {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if !admin.caps.is_global_admin {
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
