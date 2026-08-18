//! GET /admin/designer — consolidated Forms/Polls hub (tabbed). Renames and
//! replaces `/admin/form-designer` as the nav entry point; the old list
//! route now redirects here (see `form_designer::list`'s doc comment).

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use std::collections::HashMap;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::{form_def, poll_def};

use admin::pages::form_designer::{forms_list_fragment, FormRow};
use admin::pages::poll_designer::{polls_list_fragment, PollRow};

pub async fn list(State(state): State<AppState>, admin: AdminUser, Query(_params): Query<HashMap<String, String>>) -> Response {
    if !admin.caps.can_manage_forms {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Some(site_id) = admin.site_id else {
        return (axum::http::StatusCode::BAD_REQUEST, "No site selected.").into_response();
    };

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let forms = form_def::list_for_site(&state.db, site_id).await.unwrap_or_default();
    let blocked = crate::models::form_submission::blocked_names(&state.db, site_id).await;
    let form_rows: Vec<FormRow> = forms.into_iter().map(|f| FormRow {
        id: f.id.to_string(),
        blocked: blocked.contains(&f.name),
        name: f.name,
        slug: f.slug,
        field_count: f.fields.len(),
        updated_at: f.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    }).collect();

    let polls = poll_def::list_for_site(&state.db, site_id).await.unwrap_or_default();
    let poll_rows: Vec<PollRow> = polls.into_iter().map(|p| PollRow {
        id: p.id.to_string(),
        name: p.name,
        slug: p.slug,
        option_count: p.options.len(),
        total_votes: p.total_votes,
        updated_at: p.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    }).collect();

    let forms_fragment = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:flex-end;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">
    <a href="/admin/form-designer/new" class="icon-btn" title="New Form" aria-label="New Form"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
</div>
{table}"#,
        table = forms_list_fragment(&form_rows, 1, 1, "", "", ""),
    );

    let polls_fragment = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:flex-end;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">
    <a href="/admin/designer/polls/new" class="icon-btn" title="New Poll" aria-label="New Poll"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
</div>
{table}"#,
        table = polls_list_fragment(&poll_rows, ""),
    );

    Html(admin::pages::designer_hub::render(&forms_fragment, &polls_fragment, &ctx, None)).into_response()
}
