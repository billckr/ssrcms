use axum::{extract::State, response::Html};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::{DashboardData, RecentPost};

pub async fn dashboard(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    // Fetch counts.
    let published_posts = crate::models::post::count(
        &state.db, Some(crate::models::post::PostStatus::Published), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard published count error: {:?}", e); 0 });

    let draft_posts = crate::models::post::count(
        &state.db, Some(crate::models::post::PostStatus::Draft), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard draft count error: {:?}", e); 0 });

    let total_pages = crate::models::post::count(
        &state.db, None, Some(crate::models::post::PostType::Page)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard pages count error: {:?}", e); 0 });

    let total_users = crate::models::user::count(&state.db).await
        .unwrap_or_else(|e| { tracing::warn!("dashboard users count error: {:?}", e); 0 });

    // Fetch recent posts.
    let filter = crate::models::post::ListFilter {
        post_type: Some(crate::models::post::PostType::Post),
        limit: 10,
        ..Default::default()
    };
    let recent_raw = crate::models::post::list(&state.db, &filter).await.unwrap_or_else(|e| {
        tracing::warn!("dashboard recent posts error: {:?}", e);
        vec![]
    });
    let recent_posts: Vec<RecentPost> = recent_raw.iter().map(|p| RecentPost {
        id: p.id.to_string(),
        title: p.title.clone(),
        status: p.status.clone(),
        slug: p.slug.clone(),
    }).collect();

    let data = DashboardData {
        published_posts,
        draft_posts,
        total_pages,
        total_users,
        recent_posts,
    };

    Html(admin::pages::dashboard::render(&data, None))
}
