use axum::{extract::State, response::Html};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::{DashboardData, RecentPost};

pub async fn dashboard(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let site_id = admin.site_id;

    // Fetch counts.
    let published_posts = crate::models::post::count(
        &state.db, site_id, Some(crate::models::post::PostStatus::Published), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard published count error: {:?}", e); 0 });

    let draft_posts = crate::models::post::count(
        &state.db, site_id, Some(crate::models::post::PostStatus::Draft), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard draft count error: {:?}", e); 0 });

    let total_pages = crate::models::post::count(
        &state.db, site_id, None, Some(crate::models::post::PostType::Page)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard pages count error: {:?}", e); 0 });

    let total_users = crate::models::user::count(&state.db).await
        .unwrap_or_else(|e| { tracing::warn!("dashboard users count error: {:?}", e); 0 });

    // Fetch recent posts.
    let filter = crate::models::post::ListFilter {
        site_id,
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

    // Current site info for the switcher.
    let current_site_name = site_id
        .and_then(|sid| state.get_site_by_id(sid))
        .map(|(s, _)| s.hostname)
        .unwrap_or_else(|| "Default".to_string());

    let data = DashboardData {
        published_posts,
        draft_posts,
        total_pages,
        total_users,
        recent_posts,
        current_site_name,
    };

    Html(admin::pages::dashboard::render(&data, None))
}
