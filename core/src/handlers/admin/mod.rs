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
        is_impersonating: admin.caps.is_impersonating,
        can_manage_users: admin.caps.can_manage_users,
        can_manage_sites: admin.caps.can_manage_sites,
        can_manage_plugins: admin.caps.can_manage_plugins,
        can_manage_settings: admin.caps.can_manage_settings,
        can_manage_content: admin.caps.can_manage_content,
        can_manage_appearance: admin.caps.can_manage_appearance,
        can_manage_taxonomies: admin.caps.can_manage_taxonomies,
        can_manage_forms: admin.caps.can_manage_forms,
        can_manage_pages: admin.caps.can_manage_pages,
        unread_forms_count: 0,
        pending_review_count: 0,
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
    // Pending review badge: editors/admins see all site pending posts; authors see their own.
    if admin.caps.can_manage_content {
        let author_filter = if admin.site_role == "author" { Some(admin.user.id) } else { None };
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM posts
               WHERE status = 'pending'
                 AND ($1::uuid IS NULL OR site_id = $1)
                 AND ($2::uuid IS NULL OR author_id = $2)"#
        )
        .bind(admin.site_id)
        .bind(author_filter)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
        ctx.pending_review_count = count;
    }
    ctx
}

/// Strip HTML tags and disallowed characters from media metadata fields
/// (alt text, title, caption), then trim whitespace and enforce the 35-char
/// limit. Shared by the upload handler and the metadata update API so that
/// server-side enforcement is identical regardless of which route is used.
pub fn sanitize_media_text(input: &str) -> String {
    let no_tags = {
        let mut out = String::with_capacity(input.len());
        let mut in_tag = false;
        for ch in input.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(ch),
                _ => {}
            }
        }
        out
    };
    let clean: String = no_tags
        .chars()
        .filter(|&c| c != '&' && c != '"' && c != '`')
        .collect();
    clean.trim().chars().take(35).collect()
}

pub mod appearance;
pub mod comments;
pub mod dashboard;
pub mod documentation;
pub mod forms;
pub mod media;
pub mod menus;
pub mod plugins;
pub mod posts;
pub mod profile;
pub mod settings;
pub mod sites;
pub mod taxonomy;
pub mod upload;
pub mod users;
