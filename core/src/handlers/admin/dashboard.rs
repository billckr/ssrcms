use axum::{extract::State, response::Html};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::DashboardData;

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

    let total_users = if admin.is_global_admin {
        crate::models::user::count(&state.db).await
            .unwrap_or_else(|e| { tracing::warn!("dashboard users count error: {:?}", e); 0 })
    } else if let Some(sid) = admin.site_id {
        crate::models::user::count_for_site(&state.db, sid).await
            .unwrap_or_else(|e| { tracing::warn!("dashboard site users count error: {:?}", e); 0 })
    } else {
        0
    };

    let cs = state.site_hostname(site_id);

    let data = DashboardData {
        published_posts,
        draft_posts,
        total_pages,
        total_users,
    };

    Html(admin::pages::dashboard::render(&data, None, &cs, admin.is_global_admin, &admin.user.email))
}
