use axum::{extract::{Query, State}, response::Html};
use chrono::{Datelike, Local};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::dashboard::DashboardData;

#[derive(serde::Deserialize, Default)]
pub struct DashboardQuery {
    #[serde(default)]
    pub range: Option<String>,
    #[serde(default)]
    pub views_range: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub views_year: Option<i32>,
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
    let (author_total_views, author_views_labels, author_views_values, views_range,
         available_views_years, selected_views_year) = if is_author {
        let aid = admin.user.id;
        let current_year = Local::now().year();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM post_views pv
             JOIN posts p ON p.id = pv.post_id
             WHERE p.author_id = $1 AND ($2::uuid IS NULL OR p.site_id = $2)"
        ).bind(aid).bind(site_id).fetch_one(&state.db).await.unwrap_or(0);

        let avail_years: Vec<i32> = sqlx::query_scalar::<_, i32>(
            "SELECT DISTINCT EXTRACT(YEAR FROM pv.viewed_date)::int \
             FROM post_views pv \
             JOIN posts p ON p.id = pv.post_id \
             WHERE p.author_id = $1 AND ($2::uuid IS NULL OR p.site_id = $2) \
             ORDER BY 1 DESC"
        ).bind(aid).bind(site_id).fetch_all(&state.db).await.unwrap_or_default();

        let default_year = avail_years.first().copied().unwrap_or(current_year);
        let sel_year = query.views_year.unwrap_or(default_year);

        let range_str = query.views_range
            .as_deref()
            .map(|r| r.to_ascii_lowercase())
            .filter(|r| r == "week" || r == "month" || r == "year")
            .unwrap_or_else(|| "month".to_string());

        let (labels, values) = match range_str.as_str() {
            "month" => {
                let rows: Vec<(i32, i32)> = sqlx::query_as(
                    "SELECT EXTRACT(MONTH FROM pv.viewed_date)::int AS month_num,
                            COUNT(*)::int AS count
                     FROM post_views pv
                     JOIN posts p ON p.id = pv.post_id
                     WHERE p.author_id = $1
                       AND ($2::uuid IS NULL OR p.site_id = $2)
                       AND EXTRACT(YEAR FROM pv.viewed_date)::int = $3
                     GROUP BY month_num
                     ORDER BY month_num"
                ).bind(aid).bind(site_id).bind(sel_year).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views month chart error: {:?}", e); vec![] });
                let month_map: std::collections::HashMap<i32, f32> =
                    rows.into_iter().map(|(m, c)| (m, c as f32)).collect();
                let month_names = ["Jan","Feb","Mar","Apr","May","Jun",
                                   "Jul","Aug","Sep","Oct","Nov","Dec"];
                let labels: Vec<String> = month_names.iter().map(|s| s.to_string()).collect();
                let values: Vec<f32> = (1..=12i32)
                    .map(|m| *month_map.get(&m).unwrap_or(&0.0))
                    .collect();
                (labels, values)
            }
            "year" => {
                let rows: Vec<(i32, i32)> = sqlx::query_as(
                    "SELECT EXTRACT(YEAR FROM pv.viewed_date)::int AS yr,
                            COUNT(*)::int AS count
                     FROM post_views pv
                     JOIN posts p ON p.id = pv.post_id
                     WHERE p.author_id = $1
                       AND ($2::uuid IS NULL OR p.site_id = $2)
                     GROUP BY yr
                     ORDER BY yr"
                ).bind(aid).bind(site_id).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views year chart error: {:?}", e); vec![] });
                let labels: Vec<String> = rows.iter().map(|(y, _)| y.to_string()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values)
            }
            _ => {
                // Week: sparse — only weeks in selected year that have views.
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        'Wk ' || TO_CHAR(DATE_TRUNC('week', pv.viewed_date), 'IW') AS label,
                        COUNT(*)::int AS count
                    FROM post_views pv
                    JOIN posts p ON p.id = pv.post_id
                    WHERE p.author_id = $1
                      AND ($2::uuid IS NULL OR p.site_id = $2)
                      AND EXTRACT(YEAR FROM pv.viewed_date)::int = $3
                    GROUP BY DATE_TRUNC('week', pv.viewed_date), label
                    ORDER BY DATE_TRUNC('week', pv.viewed_date)"#
                ).bind(aid).bind(site_id).bind(sel_year).fetch_all(&state.db).await
                 .unwrap_or_else(|e| { tracing::warn!("views week chart error: {:?}", e); vec![] });
                let labels: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values)
            }
        };

        (total, labels, values, range_str, avail_years, sel_year)
    } else {
        (0, vec![], vec![], "month".to_string(), vec![], Local::now().year())
    };

    // Author chart: published posts per time bucket (year-scoped).
    let (author_chart_labels, author_chart_values, chart_range,
         available_years, selected_year) = if is_author {
        let aid = admin.user.id;
        let current_year = Local::now().year();

        let avail_years: Vec<i32> = sqlx::query_scalar::<_, i32>(
            "SELECT DISTINCT EXTRACT(YEAR FROM published_at AT TIME ZONE 'UTC')::int \
             FROM posts \
             WHERE author_id = $1 AND ($2::uuid IS NULL OR site_id = $2) \
               AND status = 'published' AND post_type = 'post' \
             ORDER BY 1 DESC"
        ).bind(aid).bind(site_id).fetch_all(&state.db).await.unwrap_or_default();

        let default_year = avail_years.first().copied().unwrap_or(current_year);
        let sel_year = query.year.unwrap_or(default_year);

        let range_str = query.range
            .as_deref()
            .map(|r| r.to_ascii_lowercase())
            .filter(|r| r == "week" || r == "month" || r == "year")
            .unwrap_or_else(|| "month".to_string());

        match range_str.as_str() {
            "month" => {
                // All 12 months of selected year, zero-filled.
                let rows: Vec<(i32, i32)> = sqlx::query_as(
                    "SELECT EXTRACT(MONTH FROM published_at AT TIME ZONE 'UTC')::int AS month_num,
                            COUNT(*)::int AS count
                     FROM posts
                     WHERE author_id = $1
                       AND ($2::uuid IS NULL OR site_id = $2)
                       AND status = 'published'
                       AND post_type = 'post'
                       AND EXTRACT(YEAR FROM published_at AT TIME ZONE 'UTC')::int = $3
                     GROUP BY month_num
                     ORDER BY month_num"
                ).bind(aid).bind(site_id).bind(sel_year).fetch_all(&state.db).await
                .unwrap_or_else(|e| { tracing::warn!("dashboard month chart error: {:?}", e); vec![] });

                let month_map: std::collections::HashMap<i32, f32> =
                    rows.into_iter().map(|(m, c)| (m, c as f32)).collect();
                let month_names = ["Jan","Feb","Mar","Apr","May","Jun",
                                   "Jul","Aug","Sep","Oct","Nov","Dec"];
                let labels: Vec<String> = month_names.iter().map(|s| s.to_string()).collect();
                let values: Vec<f32> = (1..=12i32)
                    .map(|m| *month_map.get(&m).unwrap_or(&0.0))
                    .collect();
                (labels, values, range_str, avail_years, sel_year)
            }
            "year" => {
                // All years with posts, total per year.
                let rows: Vec<(i32, i32)> = sqlx::query_as(
                    "SELECT EXTRACT(YEAR FROM published_at AT TIME ZONE 'UTC')::int AS yr,
                            COUNT(*)::int AS count
                     FROM posts
                     WHERE author_id = $1
                       AND ($2::uuid IS NULL OR site_id = $2)
                       AND status = 'published'
                       AND post_type = 'post'
                     GROUP BY yr
                     ORDER BY yr"
                ).bind(aid).bind(site_id).fetch_all(&state.db).await
                .unwrap_or_else(|e| { tracing::warn!("dashboard year chart error: {:?}", e); vec![] });

                let labels: Vec<String> = rows.iter().map(|(y, _)| y.to_string()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values, range_str, avail_years, sel_year)
            }
            _ => {
                // Week: sparse — only weeks in selected year that have posts.
                let rows: Vec<(String, i32)> = sqlx::query_as(
                    r#"SELECT
                        'Wk ' || TO_CHAR(DATE_TRUNC('week', published_at AT TIME ZONE 'UTC'), 'IW') AS label,
                        COUNT(*)::int AS count
                    FROM posts
                    WHERE author_id = $1
                      AND ($2::uuid IS NULL OR site_id = $2)
                      AND status = 'published'
                      AND post_type = 'post'
                      AND EXTRACT(YEAR FROM published_at AT TIME ZONE 'UTC')::int = $3
                    GROUP BY DATE_TRUNC('week', published_at AT TIME ZONE 'UTC'), label
                    ORDER BY DATE_TRUNC('week', published_at AT TIME ZONE 'UTC')"#
                ).bind(aid).bind(site_id).bind(sel_year).fetch_all(&state.db).await
                .unwrap_or_else(|e| { tracing::warn!("dashboard week chart error: {:?}", e); vec![] });

                let labels: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
                let values: Vec<f32> = rows.iter().map(|(_, c)| *c as f32).collect();
                (labels, values, range_str, avail_years, sel_year)
            }
        }
    } else {
        (vec![], vec![], "month".to_string(), vec![], Local::now().year())
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
        available_years,
        selected_year,
        author_views_labels,
        author_views_values,
        views_range,
        available_views_years,
        selected_views_year,
        author_total_views,
    };

    Html(admin::pages::dashboard::render(&data, None, &ctx))
}
