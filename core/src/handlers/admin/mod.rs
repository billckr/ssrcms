use crate::middleware::admin_auth::AdminUser;

/// Build a [`admin::PageContext`] from an authenticated admin user and the current site name.
/// Call this once at the top of each handler; pass `&ctx` to every render function.
pub fn page_ctx(admin: &AdminUser, current_site: &str) -> admin::PageContext {
    admin::PageContext {
        current_site: current_site.to_string(),
        user_email: admin.user.email.clone(),
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
    }
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
