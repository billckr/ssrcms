use crate::middleware::admin_auth::AdminUser;
use crate::app_state::AppState;

fn role_display_name(role: &str) -> String {
    match role {
        "super_admin" => "Super Admin",
        "admin"       => "Site Admin",
        "editor"      => "Editor",
        "author"      => "Author",
        "subscriber"  => "Subscriber",
        other         => other,
    }.to_string()
}

/// Build a [`admin::PageContext`] synchronously (unread count defaults to 0).
/// Prefer `page_ctx_full` in async handlers to include the live unread badge count.
pub fn page_ctx(state: &AppState, admin: &AdminUser, current_site: &str) -> admin::PageContext {
    let app_name = state.app_settings.read()
        .map(|s| s.app_name.clone())
        .unwrap_or_else(|_| "Synaptic".to_string());

    admin::PageContext {
        current_site: current_site.to_string(),
        user_email: admin.user.email.clone(),
        user_role: if admin.caps.is_global_admin { "Super Admin".to_string() } else { role_display_name(&admin.site_role) },
        is_global_admin: admin.caps.is_global_admin,
        visiting_foreign_site: admin.caps.visiting_foreign_site,
        can_manage_users: admin.caps.can_manage_users,
        can_manage_sites: admin.caps.can_manage_sites,
        can_manage_plugins: admin.caps.can_manage_plugins,
        can_manage_settings: admin.caps.can_manage_settings,
        can_manage_content: admin.caps.can_manage_content,
        can_manage_appearance: admin.caps.can_manage_appearance,
        can_manage_taxonomies: admin.caps.can_manage_taxonomies,
        can_manage_forms: admin.caps.can_manage_forms,
        unread_forms_count: 0,
        app_name,
    }
}

/// Build a [`admin::PageContext`] with a live unread form submissions count.
/// Use this in all standard async admin handlers.
pub async fn page_ctx_full(state: &AppState, admin: &AdminUser, current_site: &str) -> admin::PageContext {
    let mut ctx = page_ctx(state, admin, current_site);
    if admin.caps.can_manage_forms {
        if let Some(site_id) = admin.site_id {
            ctx.unread_forms_count = crate::models::form_submission::count_unread(&state.db, site_id)
                .await
                .unwrap_or(0);
        }
    }
    ctx
}

pub mod appearance;
pub mod dashboard;
pub mod forms;
pub mod media;
pub mod plugins;
pub mod posts;
pub mod profile;
pub mod settings;
pub mod sites;
pub mod taxonomy;
pub mod upload;
pub mod users;
