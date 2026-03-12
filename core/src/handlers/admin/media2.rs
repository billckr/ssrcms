use axum::{extract::State, response::Html};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Html<String> {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::media2::render_list(&ctx))
}
