//! Admin documentation viewer — displays docs written by the document-changes skill.

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    http::StatusCode,
};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::documentation::{DocEntry, render_list};

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    // Any logged-in admin can view docs.
    if !admin.caps.can_manage_settings && !admin.caps.is_global_admin {
        return (StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>".to_string())).into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let rows: Result<Vec<(String, String, String, Option<String>, Option<String>, Option<String>)>, _> =
        sqlx::query_as(
            r#"SELECT slug, title, content,
                      to_char(last_updated AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI UTC'),
                      updated_by,
                      grp
               FROM documentation
               ORDER BY
                 CASE grp WHEN 'system' THEN 0 WHEN 'feature' THEN 1 ELSE 2 END,
                 title ASC"#,
        )
        .fetch_all(&state.db)
        .await;

    match rows {
        Ok(rows) => {
            let entries: Vec<DocEntry> = rows
                .into_iter()
                .map(|(slug, title, content, last_updated, updated_by, grp)| DocEntry {
                    slug,
                    title,
                    content,
                    last_updated: last_updated.unwrap_or_default(),
                    updated_by,
                    grp: grp.unwrap_or_else(|| "feature".to_string()),
                })
                .collect();
            Html(render_list(&entries, None, &ctx)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to load documentation: {e}");
            let msg = "Failed to load documentation. The table may not exist yet — run the migration first.";
            Html(render_list(&[], Some(msg), &ctx)).into_response()
        }
    }
}
