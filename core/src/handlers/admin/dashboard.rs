use axum::{extract::State, response::Html};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::DashboardData;

pub async fn dashboard(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let site_id = admin.site_id;
    let is_author = admin.site_role == "author";

    // Fetch site-wide counts (used by admin/editor/super_admin views).
    let published_posts = crate::models::post::count(
        &state.db, site_id, Some(crate::models::post::PostStatus::Published), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard published count error: {:?}", e); 0 });

    let draft_posts = crate::models::post::count(
        &state.db, site_id, Some(crate::models::post::PostStatus::Draft), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard draft count error: {:?}", e); 0 });

    let pending_posts = crate::models::post::count(
        &state.db, site_id, Some(crate::models::post::PostStatus::Pending), Some(crate::models::post::PostType::Post)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard pending count error: {:?}", e); 0 });

    let total_pages = crate::models::post::count(
        &state.db, site_id, None, Some(crate::models::post::PostType::Page)
    ).await.unwrap_or_else(|e| { tracing::warn!("dashboard pages count error: {:?}", e); 0 });

    let total_users = if admin.caps.is_global_admin {
        crate::models::user::count(&state.db).await
            .unwrap_or_else(|e| { tracing::warn!("dashboard users count error: {:?}", e); 0 })
    } else if let Some(sid) = admin.site_id {
        crate::models::user::count_for_site(&state.db, sid).await
            .unwrap_or_else(|e| { tracing::warn!("dashboard site users count error: {:?}", e); 0 })
    } else {
        0
    };

    // Author-scoped stats: only their own posts.
    let (author_published_posts, author_draft_posts, author_pending_posts) = if is_author {
        let aid = admin.user.id;
        let ap = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM posts WHERE author_id = $1 AND ($2::uuid IS NULL OR site_id = $2) AND status = 'published' AND post_type = 'post'"
        ).bind(aid).bind(site_id).fetch_one(&state.db).await.unwrap_or(0);
        let ad = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM posts WHERE author_id = $1 AND ($2::uuid IS NULL OR site_id = $2) AND status = 'draft' AND post_type = 'post'"
        ).bind(aid).bind(site_id).fetch_one(&state.db).await.unwrap_or(0);
        let an = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM posts WHERE author_id = $1 AND ($2::uuid IS NULL OR site_id = $2) AND status = 'pending' AND post_type = 'post'"
        ).bind(aid).bind(site_id).fetch_one(&state.db).await.unwrap_or(0);
        (ap, ad, an)
    } else {
        (0, 0, 0)
    };

    let cs = state.site_hostname(site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let data = DashboardData {
        published_posts,
        draft_posts,
        pending_posts,
        total_pages,
        total_users,
        author_published_posts,
        author_draft_posts,
        author_pending_posts,
    };

    Html(admin::pages::dashboard::render(&data, None, &ctx))
}
