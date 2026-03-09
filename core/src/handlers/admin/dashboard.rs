use axum::{extract::{Query, State}, response::Html};
use chrono::{Duration, Local};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::DashboardData;

#[derive(serde::Deserialize, Default)]
pub struct DashboardQuery {
    #[serde(default)]
    pub range: Option<String>,
    #[serde(default)]
    pub views_range: Option<String>,
}

pub async fn dashboard(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<DashboardQuery>,
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

    // Author view totals and chart data (only for author role).
    let (author_total_views, author_views_labels, author_views_values, views_range) = if is_author {
        let aid = admin.user.id;

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM post_views pv
             JOIN posts p ON p.id = pv.post_id
             WHERE p.author_id = $1 AND ($2::uuid IS NULL OR p.site_id = $2)"
        ).bind(aid).bind(site_id).fetch_one(&state.db).await.unwrap_or(0);

        let range_str = query.views_range
            .as_deref()
            .map(|r| r.to_ascii_lowercase())
            .filter(|r| r == "month" || r == "year")
            .unwrap_or_else(|| "week".to_string());

        let (labels, values) = match range_str.as_str() {
            "month" => {
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        'Wk ' || TO_CHAR(DATE_TRUNC('week', pv.viewed_date), 'IW') as label,
                        COUNT(*)::int as count
                    FROM post_views pv
                    JOIN posts p ON p.id = pv.post_id
                    WHERE p.author_id = $1
                      AND ($2::uuid IS NULL OR p.site_id = $2)
                      AND pv.viewed_date >= CURRENT_DATE - INTERVAL '27 days'
                    GROUP BY DATE_TRUNC('week', pv.viewed_date), label
                    ORDER BY DATE_TRUNC('week', pv.viewed_date)"#
                ).bind(aid).bind(site_id).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views month chart error: {:?}", e); vec![] });
                (
                    rows.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>(),
                    rows.iter().map(|(_, c)| *c as f32).collect::<Vec<_>>(),
                )
            }
            "year" => {
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        TO_CHAR(DATE_TRUNC('month', pv.viewed_date), 'Mon') as label,
                        COUNT(*)::int as count
                    FROM post_views pv
                    JOIN posts p ON p.id = pv.post_id
                    WHERE p.author_id = $1
                      AND ($2::uuid IS NULL OR p.site_id = $2)
                      AND pv.viewed_date >= CURRENT_DATE - INTERVAL '11 months'
                    GROUP BY DATE_TRUNC('month', pv.viewed_date), label
                    ORDER BY DATE_TRUNC('month', pv.viewed_date)"#
                ).bind(aid).bind(site_id).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views year chart error: {:?}", e); vec![] });
                (
                    rows.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>(),
                    rows.iter().map(|(_, c)| *c as f32).collect::<Vec<_>>(),
                )
            }
            _ => {
                // Week: last 7 days by day, zero-filled.
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        TO_CHAR(pv.viewed_date, 'Dy') as label,
                        COUNT(*)::int as count
                    FROM post_views pv
                    JOIN posts p ON p.id = pv.post_id
                    WHERE p.author_id = $1
                      AND ($2::uuid IS NULL OR p.site_id = $2)
                      AND pv.viewed_date >= CURRENT_DATE - INTERVAL '6 days'
                    GROUP BY pv.viewed_date, label
                    ORDER BY pv.viewed_date"#
                ).bind(aid).bind(site_id).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views week chart error: {:?}", e); vec![] });

                let result_map: std::collections::HashMap<String, f32> = rows
                    .into_iter().map(|(l, c)| (l, c as f32)).collect();
                let today = Local::now().date_naive();
                let labels: Vec<String> = (0..7i64).rev()
                    .map(|i| format!("{}", (today - Duration::days(i)).format("%a")))
                    .collect();
                let values: Vec<f32> = labels.iter()
                    .map(|l| *result_map.get(l).unwrap_or(&0.0))
                    .collect();
                (labels, values)
            }
        };

        (total, labels, values, range_str)
    } else {
        (0, vec![], vec![], "week".to_string())
    };

    // Author chart: query published posts per time bucket.
    let (author_chart_labels, author_chart_values, chart_range) = if is_author {
        let aid = admin.user.id;
        let range_str = query.range
            .as_deref()
            .map(|r| r.to_ascii_lowercase())
            .filter(|r| r == "month" || r == "year")
            .unwrap_or_else(|| "week".to_string());

        match range_str.as_str() {
            "month" => {
                // Last 28 days, one bar per week (4 bars).
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        'Wk ' || TO_CHAR(DATE_TRUNC('week', published_at AT TIME ZONE 'UTC'), 'IW') as label,
                        COUNT(*)::int as count
                    FROM posts
                    WHERE author_id = $1
                      AND ($2::uuid IS NULL OR site_id = $2)
                      AND status = 'published'
                      AND post_type = 'post'
                      AND published_at >= NOW() - INTERVAL '28 days'
                    GROUP BY DATE_TRUNC('week', published_at AT TIME ZONE 'UTC'), label
                    ORDER BY DATE_TRUNC('week', published_at AT TIME ZONE 'UTC')"#
                )
                .bind(aid)
                .bind(site_id)
                .fetch_all(&state.db)
                .await
                .unwrap_or_else(|e| { tracing::warn!("dashboard month chart error: {:?}", e); vec![] });

                let labels: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values, range_str)
            }
            "year" => {
                // Last 12 months, one bar per month.
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        TO_CHAR(DATE_TRUNC('month', published_at AT TIME ZONE 'UTC'), 'Mon') as label,
                        COUNT(*)::int as count
                    FROM posts
                    WHERE author_id = $1
                      AND ($2::uuid IS NULL OR site_id = $2)
                      AND status = 'published'
                      AND post_type = 'post'
                      AND published_at >= NOW() - INTERVAL '12 months'
                    GROUP BY DATE_TRUNC('month', published_at AT TIME ZONE 'UTC'), label
                    ORDER BY DATE_TRUNC('month', published_at AT TIME ZONE 'UTC')"#
                )
                .bind(aid)
                .bind(site_id)
                .fetch_all(&state.db)
                .await
                .unwrap_or_else(|e| { tracing::warn!("dashboard year chart error: {:?}", e); vec![] });

                let labels: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values, range_str)
            }
            _ => {
                // Week (default): last 7 days, one bar per day, zero-filled.
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        TO_CHAR(DATE(published_at AT TIME ZONE 'UTC'), 'Dy') as label,
                        COUNT(*)::int as count
                    FROM posts
                    WHERE author_id = $1
                      AND ($2::uuid IS NULL OR site_id = $2)
                      AND status = 'published'
                      AND post_type = 'post'
                      AND published_at >= NOW() - INTERVAL '7 days'
                    GROUP BY DATE(published_at AT TIME ZONE 'UTC'), label
                    ORDER BY DATE(published_at AT TIME ZONE 'UTC')"#
                )
                .bind(aid)
                .bind(site_id)
                .fetch_all(&state.db)
                .await
                .unwrap_or_else(|e| { tracing::warn!("dashboard week chart error: {:?}", e); vec![] });

                // Build a map from 3-letter day abbrev -> count.
                let result_map: std::collections::HashMap<String, f32> = rows
                    .into_iter()
                    .map(|(label, count)| (label, count as f32))
                    .collect();

                let today = Local::now().date_naive();
                let labels: Vec<String> = (0..7i64)
                    .rev()
                    .map(|i| {
                        let d = today - Duration::days(i);
                        format!("{}", d.format("%a"))
                    })
                    .collect();
                let values: Vec<f32> = labels
                    .iter()
                    .map(|l| *result_map.get(l).unwrap_or(&0.0))
                    .collect();

                (labels, values, range_str)
            }
        }
    } else {
        (vec![], vec![], "week".to_string())
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
        author_chart_labels,
        author_chart_values,
        chart_range,
        author_views_labels,
        author_views_values,
        views_range,
        author_total_views,
    };

    Html(admin::pages::dashboard::render(&data, None, &ctx))
}
