use crate::middleware::admin_auth::AdminUser;
use crate::app_state::AppState;

/// Records an account/site lifecycle event to the persistent audit_log
/// table (who did what, to what, when) — see core/src/models/audit_log.rs.
/// Never fails the caller's action: logs a warning and swallows the error
/// instead of propagating it, since audit logging is a side effect, not a
/// precondition of the action it's recording.
pub async fn audit(
    state: &AppState,
    admin: &AdminUser,
    action: &str,
    target_type: &str,
    target_id: Option<uuid::Uuid>,
    target_label: &str,
    site_id: Option<uuid::Uuid>,
) {
    let actor_role = if admin.caps.is_global_admin { "super_admin" } else { "site_admin" };
    if let Err(e) = crate::models::audit_log::record(&state.db, crate::models::audit_log::NewAuditLog {
        actor_user_id: Some(admin.user.id),
        actor_email: &admin.user.email,
        actor_role,
        action,
        target_type,
        target_id,
        target_label,
        site_id,
        details: None,
    }).await {
        tracing::warn!("audit log failed: action={} target_type={} target_id={:?}: {:?}", action, target_type, target_id, e);
    }
}

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
    let (mut app_name, default_theme) = {
        let s = state.app_settings.read();
        match s {
            Ok(s) => (s.app_name.clone(), s.default_theme.clone()),
            Err(_) => ("Synaptic".to_string(), "system".to_string()),
        }
    };

    let mut logo_url = state.logo_url.read().ok().and_then(|g| g.clone());

    // A site's own admin branding (set at /admin/site-settings) overrides
    // the global app_name/logo for anyone viewing that site's admin —
    // "consistent branding" regardless of which role they're logged in as.
    if let Some(site_id) = admin.site_id {
        if let Some((site, settings)) = state.get_site_by_id(site_id) {
            match site.parent_site_id {
                Some(parent_id) => {
                    // Child site: it can't set its own branding (see
                    // can_manage_site_settings), so it never shows this site's
                    // own agency-wide fallback either — only its immediate
                    // parent's branding, or the parent's hostname as plain
                    // text if the parent hasn't customized anything. Never
                    // falls through to the global default beyond that, so
                    // the agency's own logo can't leak onto a client's
                    // sub-site.
                    logo_url = None;
                    match state.get_site_by_id(parent_id) {
                        Some((parent_site, parent_settings)) => {
                            app_name = parent_settings.admin_brand_name.unwrap_or(parent_site.hostname);
                            if let Some(parent_logo) = crate::app_state::detect_site_admin_logo(parent_id) {
                                logo_url = Some(parent_logo);
                            }
                        }
                        None => app_name = site.hostname,
                    }
                }
                None => {
                    if let Some(name) = settings.admin_brand_name {
                        app_name = name;
                    }
                    if let Some(site_logo) = crate::app_state::detect_site_admin_logo(site_id) {
                        logo_url = Some(site_logo);
                    }
                }
            }
        }
    }

    admin::PageContext {
        current_site: current_site.to_string(),
        user_email: admin.user.email.clone(),
        user_role: if admin.caps.is_global_admin {
            "Super Admin".to_string()
        } else {
            match admin.site_role {
                Some(r) => role_display_name(r.as_str()),
                None => role_display_name(&admin.user.role),
            }
        },
        is_global_admin: admin.caps.is_global_admin,
        is_impersonating: admin.caps.is_impersonating,
        can_manage_users: admin.caps.can_manage_users,
        can_manage_sites: admin.caps.can_manage_sites,
        can_manage_plugins: admin.caps.can_manage_plugins,
        can_manage_settings: admin.caps.can_manage_settings,
        can_manage_content: admin.caps.can_manage_content,
        can_manage_themes: admin.caps.can_manage_themes,
        can_manage_taxonomies: admin.caps.can_manage_taxonomies,
        can_manage_forms: admin.caps.can_manage_forms,
        can_manage_pages: admin.caps.can_manage_pages,
        can_manage_site_settings: admin.caps.can_manage_site_settings,
        unread_forms_count: 0,
        app_name,
        logo_url,
        default_theme,
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

pub mod activity_log;
pub mod analytics;
pub mod themes;
pub mod themes_editor;
pub mod themes_publish;
pub mod themes_upload;
pub mod builder;
pub mod comments;
pub mod dashboard;
pub mod dev_tools;
pub mod documentation;
pub mod email_providers;
pub mod form_designer;
pub mod forms;
pub mod poll_designer;
pub mod poll_results;
pub mod designer_hub;
pub mod media;
pub mod menus;
pub mod plugins;
pub mod posts;
pub mod profile;
pub mod role_picker;
pub mod settings;
pub mod site_settings;
pub mod logo_upload;
pub mod sites;
pub mod taxonomy;
pub mod upload;
pub mod users;
