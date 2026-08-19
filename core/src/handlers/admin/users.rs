use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use crate::models::user::{CreateUser, UpdateUser, UserRole};
use admin::pages::users::{SiteOption, UserEdit, UserRow};

#[derive(Deserialize, Default)]
pub struct UsersTabQuery {
    #[serde(default)]
    pub tab: String,
    /// Optional site UUID to filter the user list (super_admin only).
    #[serde(default)]
    pub site: String,
    /// Search term matched against display name, username, and email (case-insensitive).
    #[serde(default)]
    pub search: String,
    /// When set (any value), return only the table rows HTML for JS live-search.
    #[serde(default)]
    pub partial: String,
    /// 1-indexed page number for the active tab's table.
    pub page: Option<i64>,
    /// Column to sort by: "name" | "username" | "email" | "role".
    #[serde(default)]
    pub sort: String,
    /// Sort direction: "asc" or "desc".
    #[serde(default)]
    pub dir: String,
}

/// Sort key for a given column — lowercased so ordering is case-insensitive.
fn user_sort_key(u: &UserRow, sort: &str) -> String {
    match sort {
        "username" => u.username.to_lowercase(),
        "email"    => u.email.to_lowercase(),
        "role"     => u.role.to_lowercase(),
        _          => u.display_name.to_lowercase(),
    }
}

/// Sort a user row list in place by the whitelisted column/direction from the query string.
/// No-op when `sort` is empty (keeps the existing DB fetch order).
fn sort_user_rows(rows: &mut [UserRow], sort: &str, dir: &str) {
    if sort.is_empty() {
        return;
    }
    let asc = dir != "desc";
    rows.sort_by(|a, b| {
        let (ka, kb) = (user_sort_key(a, sort), user_sort_key(b, sort));
        if asc { ka.cmp(&kb) } else { kb.cmp(&ka) }
    });
}

const USERS_PER_PAGE: i64 = 20;

/// Split a flat list of UserRows into (staff, subscribers).
/// Staff = any role that is not "subscriber".
fn split_by_role(rows: Vec<UserRow>) -> (Vec<UserRow>, Vec<UserRow>) {
    let mut staff = Vec::new();
    let mut subs  = Vec::new();
    for r in rows {
        if r.role == "subscriber" { subs.push(r); } else { staff.push(r); }
    }
    (staff, subs)
}

/// Builds `UserRow.role` from a user's (possibly several) roles on a site:
/// the raw role slug unchanged when there's exactly one (so callers like
/// `split_by_role`, which does an exact `== "subscriber"` check, keep
/// working — role_display()/role_badge_class() also expect a raw slug),
/// or a comma-joined human-readable label when there's more than one
/// (e.g. "Author, Editor" — a combination that was never a real role slug
/// to begin with, so nothing downstream depends on it being one).
fn multi_role_display(roles: &[String]) -> String {
    match roles {
        [single] => single.clone(),
        many => many.iter().map(|r| role_label(r)).collect::<Vec<_>>().join(", "),
    }
}

/// Short site-role label — mirrors `admin::pages::users::role_display` but
/// for `site_users.role` values (no `super_admin` case, "admin" -> "Admin"
/// rather than "Site Admin") since this is used to build comma-joined
/// multi-role labels (e.g. "Author, Editor") for a single site.
fn role_label(role: &str) -> &str {
    match role {
        "admin" => "Admin",
        "editor" => "Editor",
        "author" => "Author",
        "subscriber" => "Subscriber",
        other => other,
    }
}

pub async fn list(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<UsersTabQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);

    // Fetch available sites for the filter dropdown (global admin only).
    // When visiting a foreign site (impersonating), scope to sites owned by
    // that site's owner — never expose the super admin's own sites.
    let available_sites = if admin.caps.is_global_admin {
        if admin.caps.is_impersonating {
            if let Some(sid) = admin.site_id {
                fetch_site_options_for_site_owner(&state, sid).await
            } else {
                fetch_site_options(&state).await
            }
        } else {
            fetch_site_options(&state).await
        }
    } else {
        vec![]
    };

    // When visiting a foreign site with no explicit ?site= filter, default to
    // the current site so the user list is scoped correctly out of the box.
    let effective_site_filter: Option<Uuid> = if !q.site.is_empty() {
        q.site.parse::<Uuid>().ok()
    } else if admin.caps.is_impersonating {
        admin.site_id
    } else {
        None
    };
    let selected_site_id = effective_site_filter
        .map(|id| id.to_string())
        .unwrap_or_default();

    let rows: Vec<UserRow> = if admin.caps.is_global_admin {
        if let Some(filter_site_id) = effective_site_filter {
            // Filtered: show only users assigned to this specific site. A
            // user can hold more than one role on the same site (multi-role
            // support), so list_for_site returns one row per role — grouped
            // here into one UserRow per user with every role joined, rather
            // than one duplicated row per role.
            let raw = crate::models::site_user::list_for_site(&state.db, filter_site_id)
                .await.unwrap_or_else(|e| {
                    tracing::warn!("failed to list site users for filter: {:?}", e);
                    vec![]
                });
            let mut grouped: Vec<(crate::models::user::User, Vec<String>)> = Vec::new();
            for (u, role) in raw {
                match grouped.iter_mut().find(|(existing, _)| existing.id == u.id) {
                    Some((_, roles)) => roles.push(role),
                    None => grouped.push((u, vec![role])),
                }
            }
            grouped.into_iter().map(|(u, roles)| UserRow {
                id: u.id.to_string(),
                username: u.username.clone(),
                email: u.email.clone(),
                role: multi_role_display(&roles),
                display_name: u.display_name.clone(),
                is_protected: u.is_protected,
                is_active: u.is_active,
                is_super_admin: u.role == "super_admin",
                site_hostnames: vec![],
                site_ids: vec![],
                site_role_labels: vec![],
                default_site_id: None,
                sole_admin_hostnames: vec![],
                personal_data_erased: u.personal_data_erased_at.is_some(),
            }).collect()
        } else {
            // All sites: show every user, with site-context role when available.
            let users = crate::models::user::list_all(&state.db).await.unwrap_or_else(|e| {
                tracing::warn!("failed to list users: {:?}", e);
                vec![]
            });
            // A user can hold more than one role on the current site
            // (multi-role support) — collect all of them per user rather
            // than overwriting to just the last one list_for_site happens
            // to return.
            let site_role_map: std::collections::HashMap<uuid::Uuid, Vec<String>> =
                if let Some(sid) = admin.site_id {
                    let mut map: std::collections::HashMap<uuid::Uuid, Vec<String>> = std::collections::HashMap::new();
                    for (u, r) in crate::models::site_user::list_for_site(&state.db, sid).await.unwrap_or_default() {
                        map.entry(u.id).or_default().push(r);
                    }
                    map
                } else {
                    std::collections::HashMap::new()
                };
            users.iter().map(|u| UserRow {
                id: u.id.to_string(),
                username: u.username.clone(),
                email: u.email.clone(),
                role: site_role_map.get(&u.id)
                    .map(|roles| multi_role_display(roles))
                    .unwrap_or_else(|| u.role.clone()),
                display_name: u.display_name.clone(),
                is_protected: u.is_protected,
                    is_active: u.is_active,
                is_super_admin: u.role == "super_admin",
                site_hostnames: vec![],
                site_ids: vec![],
                site_role_labels: vec![],
                    default_site_id: None,
                sole_admin_hostnames: vec![],
                personal_data_erased: u.personal_data_erased_at.is_some(),
            }).collect()
        }
    } else if let Some(site_id) = admin.site_id {
        let raw = crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_else(|e| {
            tracing::warn!("failed to list site users: {:?}", e);
            vec![]
        });
        let mut grouped: Vec<(crate::models::user::User, Vec<String>)> = Vec::new();
        for (u, role) in raw {
            if u.role == "super_admin" { continue; }
            match grouped.iter_mut().find(|(existing, _)| existing.id == u.id) {
                Some((_, roles)) => roles.push(role),
                None => grouped.push((u, vec![role])),
            }
        }
        grouped.into_iter().map(|(u, roles)| UserRow {
            id: u.id.to_string(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: multi_role_display(&roles),
            display_name: u.display_name.clone(),
            is_protected: u.is_protected,
                    is_active: u.is_active,
            is_super_admin: false,
            site_hostnames: vec![],
            site_ids: vec![],
            site_role_labels: vec![],
                    default_site_id: None,
            sole_admin_hostnames: vec![],
            personal_data_erased: u.personal_data_erased_at.is_some(),
        }).collect()
    } else {
        vec![]
    };

    let current_user_id = admin.user.id.to_string();
    // Exclude the currently logged-in user — they manage their own account via /admin/profile.
    let rows: Vec<_> = rows.into_iter().filter(|u| u.id != current_user_id).collect();

    // Search: match display name, username, or email (case-insensitive substring).
    let search_term = q.search.trim().to_lowercase();
    let rows: Vec<_> = if search_term.is_empty() {
        rows
    } else {
        rows.into_iter().filter(|u| {
            u.display_name.to_lowercase().contains(&search_term)
                || u.username.to_lowercase().contains(&search_term)
                || u.email.to_lowercase().contains(&search_term)
        }).collect()
    };

    let can_manage_access = admin.caps.can_manage_users;
    let active_tab = if q.tab == "subscribers" { "subscribers" } else { "site-users" };
    let (mut staff, mut subscribers) = split_by_role(rows);
    sort_user_rows(&mut staff, &q.sort, &q.dir);
    sort_user_rows(&mut subscribers, &q.sort, &q.dir);
    let staff_total = staff.len() as i64;
    let sub_total = subscribers.len() as i64;

    // Only the active tab is ever displayed, so slice that list down to one page
    // (per-tab totals above are kept for the tab-bar badge counts).
    let active_total = if active_tab == "subscribers" { sub_total } else { staff_total };
    let total_pages = ((active_total + USERS_PER_PAGE - 1) / USERS_PER_PAGE).max(1);
    let page = q.page.unwrap_or(1).max(1).min(total_pages);
    let offset = ((page - 1) * USERS_PER_PAGE) as usize;
    let (mut staff, mut subscribers) = if active_tab == "subscribers" {
        (vec![], subscribers.into_iter().skip(offset).take(USERS_PER_PAGE as usize).collect())
    } else {
        (staff.into_iter().skip(offset).take(USERS_PER_PAGE as usize).collect(), vec![])
    };

    // Populate site_hostnames and default_site_id for all users in one pass.
    let all_ids: Vec<Uuid> = staff.iter().chain(subscribers.iter())
        .filter_map(|u| u.id.parse::<Uuid>().ok())
        .collect();
    if !all_ids.is_empty() {
        // su.role included so a user with multiple roles on the same site
        // collapses to one domain badge with all roles in its tooltip,
        // rather than one duplicated badge per role (see membership_map below).
        let membership_rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
            "SELECT su.user_id, s.id, s.hostname, su.role \
             FROM site_users su \
             JOIN sites s ON s.id = su.site_id \
             WHERE su.user_id = ANY($1) \
             ORDER BY s.created_at ASC, su.role ASC",
        )
        .bind(&all_ids)
        .fetch_all(&state.db)
        .await
        .unwrap_or_else(|e| { tracing::warn!("failed to fetch user sites: {:?}", e); vec![] });

        // Fetch each user's default_site_id so the primary domain badge can be shown.
        let default_site_rows: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT id, default_site_id FROM users WHERE id = ANY($1)",
        )
        .bind(&all_ids)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
        let default_site_map: std::collections::HashMap<String, Option<String>> = default_site_rows
            .into_iter()
            .map(|(uid, dsid)| (uid.to_string(), dsid.map(|id| id.to_string())))
            .collect();

        // (site_id, hostname) -> roles held on that site, in insertion order,
        // deduped per site so a user with multiple roles there gets one entry.
        let mut membership_map: std::collections::HashMap<String, Vec<(String, String, Vec<String>)>> = std::collections::HashMap::new();
        for (uid, site_id, hostname, role) in membership_rows {
            let entries = membership_map.entry(uid.to_string()).or_default();
            let site_id = site_id.to_string();
            match entries.iter_mut().find(|(sid, _, _)| *sid == site_id) {
                Some((_, _, roles)) => roles.push(role),
                None => entries.push((site_id, hostname, vec![role])),
            }
        }
        // Preflight which of these users are a site's sole admin, so Delete
        // can be disabled up front instead of round-tripping to find out.
        let sole_admin_map = crate::models::site_user::sole_admin_hostnames_batch(&state.db, &all_ids)
            .await
            .unwrap_or_default();

        for u in staff.iter_mut().chain(subscribers.iter_mut()) {
            if let Some(memberships) = membership_map.get(&u.id) {
                u.site_hostnames = memberships.iter().map(|(_, h, _)| h.clone()).collect();
                u.site_ids       = memberships.iter().map(|(id, _, _)| id.clone()).collect();
                u.site_role_labels = memberships.iter()
                    .map(|(_, _, roles)| roles.iter().map(|r| role_label(r)).collect::<Vec<_>>().join(", "))
                    .collect();
            }
            u.default_site_id = default_site_map.get(&u.id).and_then(|v| v.clone());
            if let Ok(uid) = u.id.parse::<Uuid>() {
                u.sole_admin_hostnames = sole_admin_map.get(&uid).cloned().unwrap_or_default();
            }
        }
    }

    // `partial=<anything>` means the JS live-search is requesting only the table
    // rows HTML so the browser can swap tbody#users-tbody without a full reload.
    if !q.partial.is_empty() {
        return Html(admin::pages::users::users_list_fragment(
            &staff, &subscribers, &current_user_id, can_manage_access, active_tab, &q.search, page, total_pages, &q.sort, &q.dir,
        )).into_response();
    }

    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    Html(admin::pages::users::render_list(
        &staff, &subscribers, staff_total, sub_total, page, total_pages, None, &current_user_id,
        can_manage_access, active_tab, &available_sites, &selected_site_id, &q.search, &q.sort, &q.dir, &ctx,
    )).into_response()
}

pub async fn new_user(
    State(state): State<AppState>,
    admin: AdminUser,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let sites = fetch_sites_for_admin(&state, &admin).await;
    let edit = UserEdit {
        id: None,
        username: String::new(),
        email: String::new(),
        display_name: String::new(),
        role: "author".into(),
        bio: String::new(),
        sites,
        is_super_admin_target: false,
        site_roles: vec![],
        is_active: true,
        is_protected: false,
    };
    Html(admin::pages::users::render_editor(&edit, None, &ctx)).into_response()
}

#[derive(Deserialize, Default)]
pub struct EditUserQuery {
    pub success: Option<String>,
    pub error: Option<String>,
}

pub async fn edit_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Query(q): Query<EditUserQuery>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let flash = match (q.success.as_deref(), q.error.as_deref()) {
        (Some("user_updated"), _) => Some("User updated successfully."),
        (Some("personal_data_erased"), _) => Some("This account's personal data has been erased."),
        (Some("already_erased"), _) => Some("This account's personal data was already erased."),
        (_, Some("erase_failed")) => Some("Could not erase this account's personal data — see the server log for details."),
        _ => None,
    };

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::has_any_role(&state.db, sid, id)
                .await.unwrap_or(false),
            None => false,
        };
        if !allowed {
            return Redirect::to("/admin/users").into_response();
        }
    }

    let user = match crate::models::user::get_by_id_include_inactive(&state.db, id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("user {} not found for editing: {:?}", id, e);
            return Redirect::to("/admin/users").into_response();
        }
    };

    // Site admins may not edit super_admin accounts.
    if !admin.caps.is_global_admin && user.role == "super_admin" {
        return Redirect::to("/admin/users").into_response();
    }

    let is_super_admin_target = user.role.as_str() == "super_admin";

    // For non-super-admin targets, show site role (admin/editor/author/subscriber) in the form.
    let display_role = if is_super_admin_target {
        user.role.clone()
    } else if let Some(sid) = admin.site_id {
        // A user can hold multiple roles on this site now; the single-role edit
        // dropdown shows the first one (alphabetical) and, if changed, replaces
        // ALL of their roles here with the one selected (see update_role).
        crate::models::site_user::list_roles_for_user_and_site(&state.db, sid, id)
            .await
            .ok()
            .and_then(|roles| roles.into_iter().next())
            .map(|r| r.as_str().to_string())
            .unwrap_or_else(|| user.role.clone())
    } else {
        user.role.clone()
    };

    // Current site assignments — display-only, shown in the Role section.
    let site_roles: Vec<(String, String)> = crate::models::site_user::list_for_user(&state.db, id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to list site assignments for user {}: {:?}", id, e);
            vec![]
        })
        .into_iter()
        .map(|(site, role)| (site.hostname, role))
        .collect();

    let edit = UserEdit {
        id: Some(user.id.to_string()),
        username: user.username.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: display_role,
        bio: user.bio.clone(),
        sites: vec![],
        is_super_admin_target,
        site_roles,
        is_active: user.is_active,
        is_protected: user.is_protected,
    };
    Html(admin::pages::users::render_editor(&edit, flash, &ctx)).into_response()
}

#[derive(Deserialize)]
pub struct UserForm {
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub password: Option<String>,
    pub role: String,
    pub bio: Option<String>,
    /// "existing" or "new" — only present on the new-user form for global admins.
    pub site_assignment: Option<String>,
    pub existing_site_id: Option<String>,
    pub new_hostname: Option<String>,
    /// Checkbox, only meaningful (and only shown in the UI) when role ==
    /// "author" — lets this specific author publish their own posts
    /// directly instead of only draft/pending. Ignored for every other role.
    #[serde(default)]
    pub can_self_publish: Option<String>,
}

pub async fn save_new(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;
    let password = match form.password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => {
            let sites = fetch_sites_for_admin(&state, &admin).await;
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites,
                is_super_admin_target: false,
                site_roles: vec![],
                is_active: true,
                is_protected: false,
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("Password is required for new users."),
                &ctx,
            )).into_response();
        }
    };

    // Validate username: 5-15 chars, lowercase letters, numbers and hyphens only.
    if let Err(msg) = crate::models::user::validate_username(form.username.trim()) {
        let sites = fetch_sites_for_admin(&state, &admin).await;
        let edit = UserEdit {
            id: None,
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites,
            is_super_admin_target: false,
            site_roles: vec![],
            is_active: true,
            is_protected: false,
        };
        return Html(admin::pages::users::render_editor(
            &edit,
            Some(msg),
            &ctx,
        )).into_response();
    }

    // Validate display name length.
    if let Err(msg) = crate::models::user::validate_display_name(
        form.display_name.as_deref().unwrap_or("").trim(),
    ) {
        let sites = fetch_sites_for_admin(&state, &admin).await;
        let edit = UserEdit {
            id: None,
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites,
            is_super_admin_target: false,
            site_roles: vec![],
            is_active: true,
            is_protected: false,
        };
        return Html(admin::pages::users::render_editor(
            &edit,
            Some(msg),
            &ctx,
        )).into_response();
    }

    // Validate hostname format when creating a new site.
    if form.site_assignment.as_deref() == Some("new") {
        let hostname = form.new_hostname.as_deref().unwrap_or("").trim().to_lowercase();
        if hostname.is_empty() || !is_valid_hostname(&hostname) {
            let sites = fetch_sites_for_admin(&state, &admin).await;
            let edit = UserEdit {
                id: None,
                username: form.username.clone(),
                email: form.email.clone(),
                display_name: form.display_name.clone().unwrap_or_default(),
                role: form.role.clone(),
                bio: form.bio.clone().unwrap_or_default(),
                sites,
                is_super_admin_target: false,
                site_roles: vec![],
                is_active: true,
                is_protected: false,
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("Invalid hostname. Use a format like example.com or sub.example.com."),
                &ctx,
            )).into_response();
        }
    }

    // Validate username availability, scoped to whichever site this user is
    // about to be assigned to — usernames are no longer globally unique, see
    // user::username_available's doc comment.
    let check_site_id = resolve_site_for_username_check(&state, &admin, &form).await;
    let check_site_ids: Vec<Uuid> = check_site_id.into_iter().collect();
    match crate::models::user::username_available(&state.db, form.username.trim(), &check_site_ids, None).await {
        Ok(true) => {}
        _ => {
            let sites = fetch_sites_for_admin(&state, &admin).await;
            let edit = UserEdit {
                id: None,
                username: form.username.clone(),
                email: form.email.clone(),
                display_name: form.display_name.clone().unwrap_or_default(),
                role: form.role.clone(),
                bio: form.bio.clone().unwrap_or_default(),
                sites,
                is_super_admin_target: false,
                site_roles: vec![],
                is_active: true,
                is_protected: false,
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("That username is already taken on this site."),
                &ctx,
            )).into_response();
        }
    }

    // Validate password requirements.
    if let Err(msg) = crate::models::user::validate_password(&password) {
        let sites = fetch_sites_for_admin(&state, &admin).await;
        let edit = UserEdit {
            id: None,
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites,
            is_super_admin_target: false,
            site_roles: vec![],
            is_active: true,
            is_protected: false,
        };
        return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
    }

    // requested_role: what the form asked for, after site-admin escalation capping.
    // Site admins cannot assign super_admin; cap to editor.
    let requested_role = if !admin.caps.is_global_admin && form.role == "super_admin" {
        "editor"
    } else {
        form.role.as_str()
    };
    // users_role: what goes into users.role. "admin" is a site_users concept, stored
    // as "site_admin" in users.role. "super_admin" is CLI-only (capped above unless
    // the actor is already a global admin).
    let users_role_str = match requested_role {
        "admin" => "site_admin",
        "super_admin" => "site_admin",
        other => other,
    };
    let role = parse_role(users_role_str);
    // site_role: what goes into site_users.role. `SiteRole` has no super_admin
    // variant at all, so a super_admin target (only reachable by a global admin,
    // per the cap above) deliberately gets None here — they never get a
    // site_users row; super_admin access bypasses site_users entirely.
    let site_role: Option<crate::models::site_user::SiteRole> =
        crate::models::site_user::SiteRole::from_str(requested_role);
    let can_self_publish = site_role == Some(crate::models::site_user::SiteRole::Author)
        && form.can_self_publish.as_deref() == Some("on");
    let create = CreateUser {
        username: form.username.clone(),
        email: form.email.clone(),
        display_name: form.display_name.clone().filter(|s| !s.is_empty()).unwrap_or_default(),
        password,
        role,
    };

    match crate::models::user::create(&state.db, &create).await {
        Ok(new_user) => {
            if admin.caps.is_global_admin {
                // Resolve target site: create new or use existing.
                let site_id = match form.site_assignment.as_deref() {
                    Some("none") | None => None,
                    Some("new") => {
                        let hostname = form.new_hostname.as_deref().unwrap_or("").trim().to_lowercase();
                        if hostname.is_empty() {
                            tracing::warn!("new user {} created but no hostname provided for new site", new_user.id);
                            None
                        } else {
                            match crate::models::site::create(&state.db, &hostname).await {
                                Ok(site) => {
                                    if let Err(e) = state.reload_site_cache().await {
                                        tracing::warn!("site cache reload failed: {:?}", e);
                                    }
                                    // Create sites/{uuid}/themes/ and uploads/{uuid}/ directories.
                                    let sid           = site.id;
                                    let site_hostname = site.hostname.clone();
                                    let sites_dir     = state.config.sites_dir.clone();
                                    let upl_dir       = state.config.uploads_dir.clone();
                                    let thm_dir       = state.config.themes_dir.clone();
                                    tokio::task::spawn_blocking(move || {
                                        let site_themes  = std::path::Path::new(&sites_dir).join(sid.to_string()).join("themes");
                                        let site_uploads = std::path::Path::new(&upl_dir).join(sid.to_string());
                                        let _ = std::fs::create_dir_all(&site_themes);
                                        let _ = std::fs::create_dir_all(&site_uploads);
                                        // Create hostname symlink for public URL aliasing.
                                        crate::handlers::uploads::ensure_hostname_symlink(&upl_dir, &site_hostname, sid);
                                        let src = std::path::Path::new(&thm_dir).join("global").join("default");
                                        let dst = site_themes.join("default");
                                        if src.is_dir() && !dst.exists() {
                                            let _ = crate::handlers::admin::themes::copy_dir_all(&src, &dst);
                                        }
                                    });
                                    // If assigning as admin, claim ownership of the new site.
                                    if site_role == Some(crate::models::site_user::SiteRole::Admin) {
                                        let _ = sqlx::query(
                                            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL",
                                        )
                                        .bind(new_user.id)
                                        .bind(site.id)
                                        .execute(&state.db)
                                        .await;
                                    }
                                    super::audit(&state, &admin, "site.created", "site", Some(site.id), &site.hostname, Some(site.id)).await;
                                    Some(site.id)
                                }
                                Err(e) => {
                                    tracing::error!("failed to create site '{}': {:?}", hostname, e);
                                    None
                                }
                            }
                        }
                    }
                    _ => {
                        // "existing" — use the selected site id.
                        form.existing_site_id
                            .as_deref()
                            .and_then(|s| s.parse::<Uuid>().ok())
                    }
                };
                if let (Some(sid), Some(role)) = (site_id, site_role) {
                    if let Err(e) = crate::models::site_user::add(&state.db, sid, new_user.id, role, None, can_self_publish).await {
                        tracing::warn!("failed to add user {} to site {}: {:?}", new_user.id, sid, e);
                    }
                    // If assigning as admin and the site has no owner yet, claim ownership.
                    if role == crate::models::site_user::SiteRole::Admin {
                        let _ = sqlx::query(
                            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL",
                        )
                        .bind(new_user.id)
                        .bind(sid)
                        .execute(&state.db)
                        .await;
                        // Set the new user's default site.
                        let _ = crate::models::user::set_default_site(&state.db, new_user.id, Some(sid)).await;
                    }
                }
                super::audit(&state, &admin, "user.created", "user", Some(new_user.id), &new_user.username, site_id).await;
            } else {
                // Site admin: handle same assignment options as global admin,
                // but scoped to sites they own.
                let target_site_id: Option<Uuid> = match form.site_assignment.as_deref() {
                    Some("none") => None,
                    // No site_assignment field means the form section was hidden (single-site admin).
                    // Auto-assign to their current site.
                    None => admin.site_id,
                    Some("new") => {
                        let hostname = form.new_hostname.as_deref().unwrap_or("").trim().to_lowercase();
                        if hostname.is_empty() {
                            tracing::warn!("new user {} created but no hostname for new site", new_user.id);
                            None
                        } else {
                            // This whole branch only runs for a site_admin (see the
                            // `else` above), so the new site is always parented under
                            // the site they're currently logged into.
                            match crate::models::site::create_with_defaults(&state.db, &hostname, Some(admin.user.id), admin.site_id).await {
                                Ok(site) => {
                                    if let Err(e) = state.reload_site_cache().await {
                                        tracing::warn!("site cache reload failed: {:?}", e);
                                    }
                                    // Create sites/{uuid}/themes/ and uploads/{uuid}/ directories.
                                    let sid           = site.id;
                                    let site_hostname = site.hostname.clone();
                                    let sites_dir     = state.config.sites_dir.clone();
                                    let upl_dir       = state.config.uploads_dir.clone();
                                    let thm_dir       = state.config.themes_dir.clone();
                                    tokio::task::spawn_blocking(move || {
                                        let site_themes  = std::path::Path::new(&sites_dir).join(sid.to_string()).join("themes");
                                        let site_uploads = std::path::Path::new(&upl_dir).join(sid.to_string());
                                        let _ = std::fs::create_dir_all(&site_themes);
                                        let _ = std::fs::create_dir_all(&site_uploads);
                                        // Create hostname symlink for public URL aliasing.
                                        crate::handlers::uploads::ensure_hostname_symlink(&upl_dir, &site_hostname, sid);
                                        let src = std::path::Path::new(&thm_dir).join("global").join("default");
                                        let dst = site_themes.join("default");
                                        if src.is_dir() && !dst.exists() {
                                            let _ = crate::handlers::admin::themes::copy_dir_all(&src, &dst);
                                        }
                                    });
                                    super::audit(&state, &admin, "site.created", "site", Some(site.id), &site.hostname, Some(site.id)).await;
                                    Some(site.id)
                                }
                                Err(e) => {
                                    tracing::error!("site admin failed to create site '{}': {:?}", hostname, e);
                                    None
                                }
                            }
                        }
                    }
                    _ => {
                        // "existing" — verify the site is owned by this admin.
                        if let Some(Ok(sid)) = form.existing_site_id.as_deref().map(|s| s.parse::<Uuid>()) {
                            let is_owner = crate::models::site::get_by_id(&state.db, sid).await
                                .map(|s| s.owner_user_id == Some(admin.user.id))
                                .unwrap_or(false);
                            if is_owner { Some(sid) } else { admin.site_id }
                        } else {
                            admin.site_id
                        }
                    }
                };
                // Site admins can never produce a super_admin target (capped above),
                // so site_role is always Some here.
                if let (Some(site_id), Some(role)) = (target_site_id, site_role) {
                    if let Err(e) = crate::models::site_user::add(&state.db, site_id, new_user.id, role, Some(admin.user.id), can_self_publish).await {
                        tracing::warn!("failed to add new user {} to site {}: {:?}", new_user.id, site_id, e);
                    }
                    if role == crate::models::site_user::SiteRole::Admin {
                        let _ = crate::models::user::set_default_site(&state.db, new_user.id, Some(site_id)).await;
                    }
                }
                super::audit(&state, &admin, "user.created", "user", Some(new_user.id), &new_user.username, target_site_id).await;
            }
            Redirect::to("/admin/users").into_response()
        }
        Err(e) => {
            tracing::error!("create user error: {:?}", e);
            let sites = fetch_sites_for_admin(&state, &admin).await;
            let edit = UserEdit {
                id: None,
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites,
                is_super_admin_target: false,
                site_roles: vec![],
                is_active: true,
                is_protected: false,
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn save_edit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    // Site isolation: non-global admins may only edit users on their site.
    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::has_any_role(&state.db, sid, id)
                .await.unwrap_or(false),
            None => false,
        };
        if !allowed {
            return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
        }
    }

    // Fetch target to know their current role (preserve super_admin, update site_users.role)
    // and current suspend state (for re-rendering the edit form on a validation error).
    let target = crate::models::user::get_by_id_include_inactive(&state.db, id).await.ok();
    let target_role = target.as_ref().map(|u| u.role.clone()).unwrap_or_default();
    let is_super_admin_target = target_role == "super_admin";
    let target_is_active = target.as_ref().map(|u| u.is_active).unwrap_or(true);
    let target_is_protected = target.as_ref().map(|u| u.is_protected).unwrap_or(false);

    // Site admins may not edit super_admin accounts.
    if !admin.caps.is_global_admin && is_super_admin_target {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    // Validate username format.
    if let Err(msg) = crate::models::user::validate_username(form.username.trim()) {
        let edit = UserEdit {
            id: Some(id.to_string()),
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites: vec![],
            is_super_admin_target,
            site_roles: vec![],
            is_active: target_is_active,
            is_protected: target_is_protected,
        };
        return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
    }

    // Validate username availability against every site this user belongs
    // to — a rename must not collide with a co-member on any of them (see
    // user::username_available's doc comment for why this is per-site, not
    // a plain global uniqueness check).
    let member_site_ids: Vec<Uuid> = crate::models::site_user::list_for_user(&state.db, id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(site, _)| site.id)
        .collect();
    match crate::models::user::username_available(&state.db, form.username.trim(), &member_site_ids, Some(id)).await {
        Ok(true) => {}
        _ => {
            let edit = UserEdit {
                id: Some(id.to_string()),
                username: form.username.clone(),
                email: form.email.clone(),
                display_name: form.display_name.clone().unwrap_or_default(),
                role: form.role.clone(),
                bio: form.bio.clone().unwrap_or_default(),
                sites: vec![],
                is_super_admin_target,
                site_roles: vec![],
                is_active: target_is_active,
                is_protected: target_is_protected,
            };
            return Html(admin::pages::users::render_editor(
                &edit,
                Some("That username is already taken on one of this user's sites."),
                &ctx,
            )).into_response();
        }
    }

    // Validate display name length.
    if let Err(msg) = crate::models::user::validate_display_name(
        form.display_name.as_deref().unwrap_or("").trim(),
    ) {
        let edit = UserEdit {
            id: Some(id.to_string()),
            username: form.username.clone(),
            email: form.email.clone(),
            display_name: form.display_name.clone().unwrap_or_default(),
            role: form.role.clone(),
            bio: form.bio.clone().unwrap_or_default(),
            sites: vec![],
            is_super_admin_target,
            site_roles: vec![],
            is_active: target_is_active,
            is_protected: target_is_protected,
        };
        return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
    }

    // Validate password requirements if a new password was supplied.
    if let Some(pw) = form.password.as_deref().filter(|p| !p.is_empty()) {
        if let Err(msg) = crate::models::user::validate_password(pw) {
            let edit = UserEdit {
                id: Some(id.to_string()),
                username: form.username.clone(),
                email: form.email.clone(),
                display_name: form.display_name.clone().unwrap_or_default(),
                role: form.role.clone(),
                bio: form.bio.clone().unwrap_or_default(),
                sites: vec![],
                is_super_admin_target,
                site_roles: vec![],
                is_active: target_is_active,
                is_protected: target_is_protected,
            };
            return Html(admin::pages::users::render_editor(&edit, Some(msg), &ctx)).into_response();
        }
    }

    let new_password_hash = if let Some(pw) = form.password.as_deref().filter(|p| !p.is_empty()) {
        match crate::models::user::hash_password(pw) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::error!("password hashing error for user {}: {:?}", id, e);
                let edit = UserEdit {
                    id: Some(id.to_string()),
                    username: form.username,
                    email: form.email,
                    display_name: form.display_name.unwrap_or_default(),
                    role: form.role,
                    bio: form.bio.unwrap_or_default(),
                    sites: vec![],
                    is_super_admin_target,
                    site_roles: vec![],
                    is_active: target_is_active,
                    is_protected: target_is_protected,
                };
                return Html(admin::pages::users::render_editor(
                    &edit,
                    Some("Failed to process password. Please try again."),
                    &ctx,
                )).into_response();
            }
        }
    } else {
        None
    };

    // Role is read-only on this form — /admin/users/:id/edit no longer changes
    // either users.role or site_users.role. All role changes go through
    // /site-access, which is explicit about which site is affected and warns
    // before demoting a site's current admin/owner. Keep the target's existing
    // global role unchanged here.
    let new_users_role = parse_role(&target_role);

    let update = UpdateUser {
        username: Some(form.username.clone()),
        email: Some(form.email.clone()),
        display_name: form.display_name.clone(),
        password_hash: new_password_hash,
        role: Some(new_users_role),
        bio: form.bio.clone(),
    };

    match crate::models::user::update(&state.db, id, &update).await {
        // Redirect back to the same edit page (rather than the list) with a
        // success flash — PRG pattern matching /admin/profile, so a refresh
        // doesn't resubmit and the admin isn't bounced away mid-edit.
        Ok(_) => Redirect::to(&format!("/admin/users/{}/edit?success=user_updated", id)).into_response(),
        Err(e) => {
            tracing::error!("update user {} error: {:?}", id, e);
            let edit = UserEdit {
                id: Some(id.to_string()),
                username: form.username,
                email: form.email,
                display_name: form.display_name.unwrap_or_default(),
                role: form.role,
                bio: form.bio.unwrap_or_default(),
                sites: vec![],
                is_super_admin_target,
                site_roles: vec![],
                is_active: target_is_active,
                is_protected: target_is_protected,
            };
            let msg = friendly_user_error(&e);
            Html(admin::pages::users::render_editor(&edit, Some(&msg), &ctx)).into_response()
        }
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let tab = form.get("tab").map(|s| s.as_str()).unwrap_or("site-users");
    let redirect_url = format!("/admin/users?tab={}", tab);
    let cs = state.site_hostname(admin.site_id);
    let current_user_id = admin.user.id.to_string();
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    macro_rules! deny {
        ($msg:expr) => {{
            tracing::warn!("delete_user denied for target={} actor={}: {}", id, admin.user.id, $msg);
            let rows: Vec<UserRow> = if admin.caps.is_global_admin {
                crate::models::user::list_all(&state.db).await.unwrap_or_default()
                    .iter().map(|u| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: u.role.clone(),
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                    is_active: u.is_active,
                        is_super_admin: u.role == "super_admin",
                        site_hostnames: vec![],
                        site_ids: vec![],
                        site_role_labels: vec![],
                    default_site_id: None,
                    sole_admin_hostnames: vec![],
                    personal_data_erased: u.personal_data_erased_at.is_some(),
                    }).collect()
            } else if let Some(site_id) = admin.site_id {
                crate::models::site_user::list_for_site(&state.db, site_id).await.unwrap_or_default()
                    .into_iter().filter(|(u, _)| u.role != "super_admin").map(|(u, site_role)| UserRow {
                        id: u.id.to_string(),
                        username: u.username.clone(),
                        email: u.email.clone(),
                        role: site_role,
                        display_name: u.display_name.clone(),
                        is_protected: u.is_protected,
                    is_active: u.is_active,
                        is_super_admin: false,
                        site_hostnames: vec![],
                        site_ids: vec![],
                        site_role_labels: vec![],
                    default_site_id: None,
                    sole_admin_hostnames: vec![],
                    personal_data_erased: u.personal_data_erased_at.is_some(),
                    }).collect()
            } else {
                vec![]
            };
            let can_manage_access = admin.caps.can_manage_users;
            let (mut staff, subscribers) = split_by_role(rows);
            let staff_total = staff.len() as i64;
            let sub_total = subscribers.len() as i64;
            let total_pages = ((staff_total + USERS_PER_PAGE - 1) / USERS_PER_PAGE).max(1);
            staff.truncate(USERS_PER_PAGE as usize);
            return Html(admin::pages::users::render_list(
                &staff,
                &subscribers,
                staff_total,
                sub_total,
                1,
                total_pages,
                Some($msg),
                &current_user_id,
                can_manage_access,
                "site-users",
                &[],
                "",
                "",
                "",
                "",
                &ctx,
            )).into_response();
        }};
    }

    // Guard 1: no self-deletion.
    if id == admin.user.id {
        deny!("You cannot delete your own account.");
    }

    // Guard 2: cannot delete a protected account.
    let target = crate::models::user::get_by_id(&state.db, id).await;
    if let Ok(ref t) = target {
        if t.is_protected {
            deny!("This account is protected and cannot be deleted.");
        }
    }

    // Guard 3: only a global admin may delete another global admin.
    if let Ok(ref t) = target {
        if t.role == "super_admin" && !admin.caps.is_global_admin {
            deny!("Only a global admin can delete another global admin account.");
        }
    }

    // Guard 4: never delete the last global admin.
    if let Ok(ref t) = target {
        if t.role == "super_admin" {
            let remaining = crate::models::user::count_global_admins(&state.db)
                .await
                .unwrap_or(2);
            if remaining <= 1 {
                deny!("Cannot delete the last global admin account.");
            }
        }
    }

    // Guard 5: never delete a user who is the sole admin of a site — every
    // site must always have an admin. Reassign ownership first.
    let sole_admin_of = crate::models::site_user::sole_admin_hostnames(&state.db, id).await.unwrap_or_default();
    if !sole_admin_of.is_empty() {
        deny!(&format!(
            "Cannot delete: this user is the only Site Admin for {}. Assign a new Site Admin first.",
            sole_admin_of.join(", ")
        ));
    }

    if let Err(e) = crate::models::user::delete_and_reassign(&state.db, id, admin.user.id).await {
        tracing::error!("delete user {} error: {:?}", id, e);
        deny!("Failed to delete user. Please try again.");
    }
    let target_label = target.as_ref().map(|t| t.username.as_str()).unwrap_or("(unknown)");
    super::audit(&state, &admin, "user.deleted", "user", Some(id), target_label, admin.site_id).await;
    Redirect::to(&redirect_url).into_response()
}

/// POST /admin/users/:id/suspend — block login without touching the
/// account's content, unlike delete. Guards mirror `delete_user`'s: no
/// self-suspend, no suspending a protected account, only a global admin may
/// suspend another global admin, and the last global admin can never be
/// suspended (same lockout risk as deleting them).
pub async fn suspend_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    _form: Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    // Only caller is the toggle on the Edit User page — stay there after the action.
    let redirect_url = format!("/admin/users/{}/edit", id);

    if id == admin.user.id {
        tracing::warn!("suspend_user denied: actor {} tried to suspend self", admin.user.id);
        return Redirect::to(&redirect_url).into_response();
    }

    let target = match crate::models::user::get_by_id_include_inactive(&state.db, id).await {
        Ok(t) => t,
        Err(_) => return Redirect::to(&redirect_url).into_response(),
    };

    if target.is_protected {
        tracing::warn!("suspend_user denied: target {} is protected", id);
        return Redirect::to(&redirect_url).into_response();
    }
    if target.role == "super_admin" {
        if !admin.caps.is_global_admin {
            tracing::warn!("suspend_user denied: actor {} is not a global admin", admin.user.id);
            return Redirect::to(&redirect_url).into_response();
        }
        let remaining = crate::models::user::count_global_admins(&state.db).await.unwrap_or(2);
        if remaining <= 1 {
            tracing::warn!("suspend_user denied: {} is the last global admin", id);
            return Redirect::to(&redirect_url).into_response();
        }
    }

    if let Err(e) = crate::models::user::deactivate(&state.db, id).await {
        tracing::error!("suspend user {} error: {:?}", id, e);
    } else {
        super::audit(&state, &admin, "user.suspended", "user", Some(id), &target.username, admin.site_id).await;
    }
    Redirect::to(&redirect_url).into_response()
}

/// POST /admin/users/:id/reactivate — restore login access.
pub async fn reactivate_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    _form: Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    // Only caller is the toggle on the Edit User page — stay there after the action.
    let redirect_url = format!("/admin/users/{}/edit", id);

    if let Err(e) = crate::models::user::reactivate(&state.db, id).await {
        tracing::error!("reactivate user {} error: {:?}", id, e);
    } else {
        let label = crate::models::user::get_by_id_include_inactive(&state.db, id).await
            .map(|u| u.username).unwrap_or_else(|_| "(unknown)".to_string());
        super::audit(&state, &admin, "user.reactivated", "user", Some(id), &label, admin.site_id).await;
    }
    Redirect::to(&redirect_url).into_response()
}

/// GET /admin/users/{id}/erase-personal-data — review page for GDPR
/// erasure. Shows what erasing this subscriber's account will do, plus any
/// form_submissions/mail_log rows found by searching for their email (both
/// tables lack a user_id FK, so this is a best-effort surface for the
/// admin to confirm, not an automatic match).
pub async fn erase_personal_data_review(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let target = match crate::models::user::get_by_id_include_inactive(&state.db, id).await {
        Ok(u) => u,
        Err(_) => return Html("<h1>User not found</h1>".to_string()).into_response(),
    };

    if target.role != "subscriber" {
        return Html("<h1>Personal data erasure is only available for subscriber accounts.</h1>".to_string()).into_response();
    }

    // Site isolation: non-global admins may only act on subscribers assigned to their site.
    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::has_any_role(&state.db, sid, id).await.unwrap_or(false),
            None => false,
        };
        if !allowed {
            return Redirect::to("/admin/users?tab=subscribers").into_response();
        }
    }

    if target.personal_data_erased_at.is_some() {
        return Redirect::to(&format!("/admin/users/{}/edit?success=already_erased", id)).into_response();
    }

    // Which sites this subscriber holds a role on — search form_submissions/
    // mail_log on each, since neither table has a user_id FK to search by.
    let sites = crate::models::site_user::list_for_user(&state.db, id).await.unwrap_or_default();

    let mut form_matches = Vec::new();
    let mut mail_matches = Vec::new();
    for (site, _role) in &sites {
        if let Ok(subs) = crate::models::form_submission::find_by_email(&state.db, site.id, &target.email).await {
            for s in subs {
                form_matches.push(admin::pages::users::ErasureMatch {
                    id: s.id.to_string(),
                    site_id: site.id.to_string(),
                    hostname: site.hostname.clone(),
                    label: s.form_name.clone(),
                    detail: format!("submitted {}", s.submitted_at.format("%Y-%m-%d %H:%M UTC")),
                });
            }
        }
        if let Ok(entries) = crate::models::mail_log::find_by_email(&state.db, site.id, &target.email).await {
            for e in entries {
                mail_matches.push(admin::pages::users::ErasureMatch {
                    id: e.id.to_string(),
                    site_id: site.id.to_string(),
                    hostname: site.hostname.clone(),
                    label: e.subject.clone(),
                    detail: format!("sent {}", e.created_at.format("%Y-%m-%d %H:%M UTC")),
                });
            }
        }
    }

    let data = admin::pages::users::ErasureReviewData {
        user_id: id.to_string(),
        display_name: target.display_name.clone(),
        email: target.email.clone(),
        form_matches,
        mail_matches,
    };

    Html(admin::pages::users::render_erase_review(&data, None, &ctx)).into_response()
}

/// POST /admin/users/{id}/erase-personal-data — performs the erasure:
/// anonymizes the account (see `user::erase_personal_data`), clears
/// comment IPs, deletes saved posts and pending password-reset tokens, and
/// deletes whichever form_submissions/mail_log rows the admin checked on
/// the review page. Checkbox names encode both the target site and record
/// id (`fs_<site_id>_<record_id>` / `ml_<site_id>_<record_id>`) rather
/// than using repeated-name array fields, which serde_urlencoded (what
/// axum's Form extractor uses) doesn't support.
pub async fn erase_personal_data(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if crate::models::user::get_by_id_include_inactive(&state.db, id).await.is_err() {
        return Redirect::to("/admin/users?tab=subscribers").into_response();
    }

    if !admin.caps.is_global_admin {
        let allowed = match admin.site_id {
            Some(sid) => crate::models::site_user::has_any_role(&state.db, sid, id).await.unwrap_or(false),
            None => false,
        };
        if !allowed {
            return Redirect::to("/admin/users?tab=subscribers").into_response();
        }
    }

    let mut fs_by_site: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    let mut ml_by_site: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    for key in form.keys() {
        let (prefix, rest) = match key.split_once('_') {
            Some(pair) => pair,
            None => continue,
        };
        let mut parts = rest.splitn(2, '_');
        let (Some(site_str), Some(record_str)) = (parts.next(), parts.next()) else { continue };
        let (Ok(site_id), Ok(record_id)) = (site_str.parse::<Uuid>(), record_str.parse::<Uuid>()) else { continue };
        match prefix {
            "fs" => fs_by_site.entry(site_id).or_default().push(record_id),
            "ml" => ml_by_site.entry(site_id).or_default().push(record_id),
            _ => {}
        }
    }

    match crate::models::user::erase_personal_data(&state.db, id).await {
        Ok(original_email) => {
            if let Err(e) = crate::models::comment::clear_ip_for_author(&state.db, id).await {
                tracing::warn!("erase_personal_data: failed to clear comment IPs for {}: {:?}", id, e);
            }
            if let Err(e) = crate::models::saved_post::delete_all_for_user(&state.db, id).await {
                tracing::warn!("erase_personal_data: failed to delete saved posts for {}: {:?}", id, e);
            }
            if let Err(e) = crate::models::password_reset::delete_all_for_user(&state.db, id).await {
                tracing::warn!("erase_personal_data: failed to delete password resets for {}: {:?}", id, e);
            }
            for (site_id, ids) in &fs_by_site {
                if let Err(e) = crate::models::form_submission::delete_many(&state.db, *site_id, ids).await {
                    tracing::warn!("erase_personal_data: failed to delete form submissions on site {}: {:?}", site_id, e);
                }
            }
            for (site_id, ids) in &ml_by_site {
                if let Err(e) = crate::models::mail_log::delete_many(&state.db, *site_id, ids).await {
                    tracing::warn!("erase_personal_data: failed to delete mail log entries on site {}: {:?}", site_id, e);
                }
            }

            super::audit(&state, &admin, "user.personal_data_erased", "user", Some(id), &original_email, admin.site_id).await;
            Redirect::to(&format!("/admin/users/{}/edit?success=personal_data_erased", id)).into_response()
        }
        Err(e) => {
            tracing::warn!("erase_personal_data failed for {}: {:?}", id, e);
            Redirect::to(&format!("/admin/users/{}/edit?error=erase_failed", id)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct BulkDeleteUsersForm {
    pub ids: String,
    pub tab: Option<String>,
}

pub async fn bulk_delete_users(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<BulkDeleteUsersForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Redirect::to("/admin/users").into_response();
    }
    let tab = form.tab.as_deref().unwrap_or("site-users");
    let redirect_url = format!("/admin/users?tab={}", tab);
    let ids: Vec<String> = form.ids.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for raw_id in &ids {
        let id = match raw_id.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => continue,
        };
        // Never self-delete.
        if id == admin.user.id { continue; }
        let target = match crate::models::user::get_by_id(&state.db, id).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        if target.is_protected { continue; }
        if target.role == "super_admin" && !admin.caps.is_global_admin { continue; }
        if target.role == "super_admin" {
            let remaining = crate::models::user::count_global_admins(&state.db).await.unwrap_or(2);
            if remaining <= 1 { continue; }
        }
        // Never delete a user who is the sole admin of a site.
        let sole_admin_of = crate::models::site_user::sole_admin_hostnames(&state.db, id).await.unwrap_or_default();
        if !sole_admin_of.is_empty() { continue; }
        if let Err(e) = crate::models::user::delete_and_reassign(&state.db, id, admin.user.id).await {
            tracing::error!("bulk delete users: failed to delete {}: {:?}", id, e);
        } else {
            super::audit(&state, &admin, "user.deleted", "user", Some(id), &target.username, admin.site_id).await;
        }
    }
    Redirect::to(&redirect_url).into_response()
}

/// Determines which site (if any) a new user's username should be checked
/// for availability against, mirroring the site-assignment resolution later
/// in `save_new` — but without that logic's side effects (creating a new
/// site, claiming ownership, etc). A "new" site assignment always checks
/// clean since nothing exists there yet to collide with.
async fn resolve_site_for_username_check(
    state: &AppState,
    admin: &AdminUser,
    form: &UserForm,
) -> Option<Uuid> {
    if admin.caps.is_global_admin {
        match form.site_assignment.as_deref() {
            Some("none") | None | Some("new") => None,
            _ => form.existing_site_id.as_deref().and_then(|s| s.parse::<Uuid>().ok()),
        }
    } else {
        match form.site_assignment.as_deref() {
            Some("none") | Some("new") => None,
            None => admin.site_id,
            _ => {
                if let Some(Ok(sid)) = form.existing_site_id.as_deref().map(|s| s.parse::<Uuid>()) {
                    let is_owner = crate::models::site::get_by_id(&state.db, sid).await
                        .map(|s| s.owner_user_id == Some(admin.user.id))
                        .unwrap_or(false);
                    if is_owner { Some(sid) } else { admin.site_id }
                } else {
                    admin.site_id
                }
            }
        }
    }
}

fn friendly_user_error(e: &crate::errors::AppError) -> String {
    let s = e.to_string();
    if s.contains("duplicate key") || s.contains("unique") {
        "A user with that username or email already exists.".to_string()
    } else {
        "Failed to save user. Please try again.".to_string()
    }
}

/// Returns true if `hostname` looks like a valid domain (e.g. example.com, sub.my-site.org).
/// Requires at least one dot, alphabetic-only TLD of 2+ chars, and valid labels.
fn is_valid_hostname(hostname: &str) -> bool {
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() < 2 { return false; }
    let tld = parts.last().unwrap();
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) { return false; }
    for label in &parts[..parts.len() - 1] {
        if label.is_empty() { return false; }
        if label.starts_with('-') || label.ends_with('-') { return false; }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return false; }
    }
    true
}

fn parse_role(s: &str) -> UserRole {
    match s {
        "super_admin" => UserRole::SuperAdmin,
        "site_admin" => UserRole::SiteAdmin,
        "editor" => UserRole::Editor,
        "author" => UserRole::Author,
        _ => UserRole::Subscriber,
    }
}

/// Fetch site options for the new-user form.
/// Global admin: all sites with admin info.
/// Site admin: only their owned sites, and only when they own more than one
/// (a single-site owner gets no selector — the backend auto-assigns).
async fn fetch_sites_for_admin(state: &AppState, admin: &AdminUser) -> Vec<SiteOption> {
    if admin.caps.is_global_admin {
        fetch_site_options(state).await
    } else {
        let owned = crate::models::site::list_by_owner(&state.db, admin.user.id)
            .await
            .unwrap_or_default();
        if owned.len() > 1 {
            owned.into_iter().map(|s| SiteOption {
                id: s.id.to_string(),
                hostname: s.hostname,
                existing_admin_id:   None,
                existing_admin_name: None,
                sole_admin_id:       None,
                sole_admin_name:     None,
            }).collect()
        } else {
            vec![]
        }
    }
}

async fn fetch_site_options(state: &AppState) -> Vec<SiteOption> {
    // Left-join with users to surface any existing non-super_admin site owner.
    // If owner_user_id points to a super_admin we treat the site as having no
    // dedicated site admin yet (the slot is open for a real site_admin).
    let rows: Vec<(uuid::Uuid, String, Option<uuid::Uuid>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT s.id, s.hostname,
               u.id            AS owner_id,
               u.display_name  AS owner_name
        FROM   sites s
        LEFT JOIN users u
               ON u.id = s.owner_user_id
              AND u.role != 'super_admin'
              AND u.deleted_at IS NULL
        ORDER BY s.created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_else(|e| { tracing::warn!("failed to list sites for user form: {:?}", e); vec![] });

    rows.into_iter()
        .map(|(id, hostname, owner_id, owner_name)| SiteOption {
            id: id.to_string(),
            hostname,
            existing_admin_id:   owner_id.map(|uid| uid.to_string()),
            existing_admin_name: owner_name,
            sole_admin_id:       None,
            sole_admin_name:     None,
        })
        .collect()
}

/// Fetch site options scoped to the owner of the given site.
/// Used when a super_admin is visiting a foreign site (impersonating) so the
/// dropdown only shows sites belonging to that site's owner, not all sites.
async fn fetch_site_options_for_site_owner(state: &AppState, site_id: Uuid) -> Vec<SiteOption> {
    let owner_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT owner_user_id FROM sites WHERE id = $1",
    )
    .bind(site_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match owner_id {
        Some(owner) => {
            crate::models::site::list_by_owner(&state.db, owner)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| SiteOption {
                    id: s.id.to_string(),
                    hostname: s.hostname,
                    existing_admin_id: None,
                    existing_admin_name: None,
                    sole_admin_id: None,
                    sole_admin_name: None,
                })
                .collect()
        }
        None => {
            // No dedicated owner — show just this site.
            match crate::models::site::get_by_id(&state.db, site_id).await {
                Ok(s) => vec![SiteOption {
                    id: s.id.to_string(),
                    hostname: s.hostname,
                    existing_admin_id: None,
                    existing_admin_name: None,
                    sole_admin_id: None,
                    sole_admin_name: None,
                }],
                Err(_) => vec![],
            }
        }
    }
}

// ── Site access management ────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct SiteAccessQuery {
    pub error: Option<String>,
    pub success: Option<String>,
}

/// GET /admin/users/:id/site-access — manage which sites a user can access.
/// Accessible to super_admin and site_admin only.
pub async fn site_access_page(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<SiteAccessQuery>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return Html("<h1>403 Forbidden</h1>".to_string()).into_response();
    }
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let target_user = match crate::models::user::get_by_id_include_inactive(&state.db, user_id).await {
        Ok(u) => u,
        Err(_) => return Html("<h1>User not found</h1>".to_string()).into_response(),
    };

    // Super admin cannot be assigned to individual sites.
    if target_user.role == "super_admin" {
        return Html("<h1>Super admins have global access and cannot be assigned to individual sites.</h1>".to_string()).into_response();
    }

    // Current site assignments for the target user.
    let raw_assignments = crate::models::site_user::list_for_user(&state.db, user_id)
        .await
        .unwrap_or_default();
    let mut assignments = Vec::with_capacity(raw_assignments.len());
    for (s, role) in raw_assignments {
        let is_last_admin = role == "admin"
            && crate::models::site_user::count_admins(&state.db, s.id).await.unwrap_or(0) <= 1;
        let can_self_publish = role == "author"
            && crate::models::site_user::get_can_self_publish(&state.db, s.id, user_id).await.unwrap_or(false);
        assignments.push(admin::pages::users::SiteAssignmentRow {
            site_id: s.id.to_string(),
            hostname: s.hostname.clone(),
            role,
            is_last_admin,
            can_self_publish,
        });
    }

    // Available sites for this admin to assign to: all for super_admin, owned for site_admin.
    let mut available_sites: Vec<SiteOption> = if admin.caps.is_global_admin {
        fetch_site_options(&state).await
    } else {
        crate::models::site::list_by_owner(&state.db, admin.user.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| SiteOption {
                id: s.id.to_string(),
                hostname: s.hostname,
                // site_admin can't assign the site_admin role so the modal never fires.
                existing_admin_id:   None,
                existing_admin_name: None,
                sole_admin_id:       None,
                sole_admin_name:     None,
            })
            .collect()
    };

    // Fill in each site's sole admin (if it has exactly one) so the Add-form
    // can warn before demoting them, even when they aren't `sites.owner_user_id`
    // (e.g. an "additional" Site Admin, or one left over after the owner was
    // removed from the site without a new owner being assigned).
    for opt in available_sites.iter_mut() {
        if let Ok(site_uuid) = opt.id.parse::<Uuid>() {
            if let Ok(Some(admin_id)) = crate::models::site_user::sole_admin(&state.db, site_uuid).await {
                if let Ok(admin_user) = crate::models::user::get_by_id(&state.db, admin_id).await {
                    opt.sole_admin_id = Some(admin_id.to_string());
                    opt.sole_admin_name = Some(admin_user.display_name);
                }
            }
        }
    }

    let data = admin::pages::users::SiteAccessData {
        user_id: user_id.to_string(),
        display_name: target_user.display_name.clone(),
        email: target_user.email.clone(),
        assignments,
        available_sites,
    };

    let flash = match query.error.as_deref() {
        Some("site_admin_exists") => Some("Please choose what to do about the site's existing Site Admin."),
        Some("db_error") => Some("Failed to update site access. Please try again."),
        Some("invalid_role") => Some("Please select a role before assigning this user to a site."),
        Some("sole_admin") => Some("This user is the site's only Site Admin. Assign a new Site Admin before removing or demoting them."),
        _ => match query.success.as_deref() {
            Some("assigned") => Some("User added to site successfully."),
            _ => None,
        },
    };

    Html(admin::pages::users::render_site_access(
        &data,
        flash,
        &ctx,
    )).into_response()
}

#[derive(Deserialize)]
pub struct SiteAccessAddForm {
    pub site_id: String,
    pub role: String,
    /// "remove", "demote_author", or "add_additional" — sent by the modal when
    /// the target site already has an existing Site Admin. "remove"/"demote_author"
    /// transfer ownership to the new assignee; "add_additional" just grants them
    /// the same 'admin' site role alongside the existing Site Admin, with no
    /// change to site ownership.
    pub displaced_action: Option<String>,
    /// Checkbox, only meaningful (and only shown in the UI) when role ==
    /// "author" — lets this specific author publish their own posts
    /// directly instead of only draft/pending. Ignored for every other role.
    #[serde(default)]
    pub can_self_publish: Option<String>,
}

/// POST /admin/users/:id/site-access/add — assign a user to a site.
pub async fn add_site_access(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<SiteAccessAddForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Ok(site_uuid) = form.site_id.parse::<Uuid>() else {
        return Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response();
    };

    // For site_admin: verify they own the target site.
    if !admin.caps.is_global_admin {
        let owned = crate::models::site::get_by_id(&state.db, site_uuid).await
            .ok()
            .and_then(|s| s.owner_user_id) == Some(admin.user.id);
        if !owned {
            return (axum::http::StatusCode::FORBIDDEN, "You do not own that site.").into_response();
        }
    }

    // Sanitise role.
    // site_admin may only be assigned by a global_admin. Multiple users may hold
    // the 'admin' site role on the same site — the modal only fires to ask
    // whether adding *this* one should also transfer ownership away from the
    // site's existing owner, not to force a 1-admin-per-site limit.
    let role = match form.role.as_str() {
        "site_admin" if admin.caps.is_global_admin => {
            // Check if site already has a non-super_admin owner.
            let existing_owner: Option<uuid::Uuid> = sqlx::query_scalar(
                r#"SELECT s.owner_user_id
                   FROM sites s
                   JOIN users u ON u.id = s.owner_user_id
                   WHERE s.id = $1
                     AND u.role != 'super_admin'
                     AND u.deleted_at IS NULL"#,
            )
            .bind(site_uuid)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let mut transfer_ownership = true;

            if let Some(old_owner_id) = existing_owner {
                match form.displaced_action.as_deref() {
                    Some("add_additional") => {
                        // Grant admin access alongside the existing Site Admin —
                        // ownership and the existing admin's access are untouched.
                        transfer_ownership = false;
                    }
                    Some("remove") => {
                        // Remove displaced admin from this site entirely.
                        let _ = crate::models::site_user::remove(&state.db, site_uuid, old_owner_id).await;
                    }
                    Some("demote_author") => {
                        // Demote displaced admin to author on this site: drop their
                        // 'admin' role specifically (not a blanket role UPDATE, since
                        // under multi-role they may hold other roles here too) and
                        // ensure an 'author' role exists.
                        let _ = crate::models::site_user::remove_role(
                            &state.db, site_uuid, old_owner_id, crate::models::site_user::SiteRole::Admin,
                        ).await;
                        let _ = crate::models::site_user::add(
                            &state.db, site_uuid, old_owner_id, crate::models::site_user::SiteRole::Author, None, false,
                        )
                        .await;
                    }
                    _ => {
                        // Modal was bypassed somehow — refuse.
                        return Redirect::to(&format!(
                            "/admin/users/{}/site-access?error=site_admin_exists", user_id
                        )).into_response();
                    }
                }
            }

            if transfer_ownership {
                let _ = sqlx::query(
                    "UPDATE sites SET owner_user_id = $1, updated_at = NOW() WHERE id = $2"
                )
                .bind(user_id)
                .bind(site_uuid)
                .execute(&state.db)
                .await;
                let _ = sqlx::query(
                    "UPDATE users SET role = 'site_admin' WHERE id = $1 AND role NOT IN ('super_admin', 'site_admin')"
                )
                .bind(user_id)
                .execute(&state.db)
                .await;
            }
            "admin" // site_users role for a site_admin is 'admin'
        }
        "editor" | "author" | "subscriber" => {
            // Never demote a site's sole admin away from 'admin' — every site
            // must always have one.
            if crate::models::site_user::sole_admin(&state.db, site_uuid).await.ok().flatten() == Some(user_id) {
                return Redirect::to(&format!(
                    "/admin/users/{}/site-access?error=sole_admin", user_id
                )).into_response();
            }

            // If this user currently owns the site, demoting them away from
            // 'admin' must clear ownership too. Otherwise sites.owner_user_id
            // keeps pointing at them while site_users.role no longer says
            // 'admin' — /admin/sites reads owner_user_id for its admin badge
            // independently of site_users.role, so the two views silently
            // disagree about who the site's admin is.
            let _ = sqlx::query(
                "UPDATE sites SET owner_user_id = NULL, updated_at = NOW() WHERE id = $1 AND owner_user_id = $2"
            )
            .bind(site_uuid)
            .bind(user_id)
            .execute(&state.db)
            .await;
            form.role.as_str()
        }
        _ => {
            // Empty/missing/unrecognized role — refuse rather than silently
            // defaulting, since a wrong default here can grant excess access.
            return Redirect::to(&format!(
                "/admin/users/{}/site-access?error=invalid_role", user_id
            )).into_response();
        }
    };

    // `role` above is sanitised to always be one of "admin"/"editor"/"author"/"subscriber".
    let role = crate::models::site_user::SiteRole::from_str(role)
        .expect("add_site_access role is sanitised to a valid SiteRole above");
    let can_self_publish = role == crate::models::site_user::SiteRole::Author
        && form.can_self_publish.as_deref() == Some("on");
    if let Err(e) = crate::models::site_user::add(&state.db, site_uuid, user_id, role, Some(admin.user.id), can_self_publish).await {
        tracing::warn!("failed to add user {} to site {}: {:?}", user_id, site_uuid, e);
        return Redirect::to(&format!("/admin/users/{}/site-access?error=db_error", user_id)).into_response();
    }

    // Reload cache so ownership change is immediately reflected.
    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after site-access add: {:?}", e);
    }

    let target_label = crate::models::user::get_by_id_include_inactive(&state.db, user_id).await
        .map(|u| u.username).unwrap_or_else(|_| "(unknown)".to_string());
    super::audit(&state, &admin, "site_user.added", "user", Some(user_id), &target_label, Some(site_uuid)).await;

    Redirect::to(&format!("/admin/users/{}/site-access?success=assigned", user_id)).into_response()
}

#[derive(Deserialize)]
pub struct SiteAccessRemoveForm {
    pub site_id: String,
}

/// POST /admin/users/:id/site-access/remove — remove a user from a site.
pub async fn remove_site_access(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<SiteAccessRemoveForm>,
) -> impl IntoResponse {
    if !admin.caps.can_manage_users {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    let Ok(site_uuid) = form.site_id.parse::<Uuid>() else {
        return Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response();
    };

    // For site_admin: verify they own the target site.
    if !admin.caps.is_global_admin {
        let owned = crate::models::site::get_by_id(&state.db, site_uuid).await
            .ok()
            .and_then(|s| s.owner_user_id) == Some(admin.user.id);
        if !owned {
            return (axum::http::StatusCode::FORBIDDEN, "You do not own that site.").into_response();
        }
    }

    // Never remove a site's sole admin — every site must always have one.
    if crate::models::site_user::sole_admin(&state.db, site_uuid).await.ok().flatten() == Some(user_id) {
        return Redirect::to(&format!(
            "/admin/users/{}/site-access?error=sole_admin", user_id
        )).into_response();
    }

    if let Err(e) = crate::models::site_user::remove(&state.db, site_uuid, user_id).await {
        tracing::warn!("failed to remove user {} from site {}: {:?}", user_id, site_uuid, e);
    } else {
        let target_label = crate::models::user::get_by_id_include_inactive(&state.db, user_id).await
            .map(|u| u.username).unwrap_or_else(|_| "(unknown)".to_string());
        super::audit(&state, &admin, "site_user.removed", "user", Some(user_id), &target_label, Some(site_uuid)).await;
    }

    // If this user was the site owner, clear owner_user_id so the site
    // can have a new site_admin assigned.
    let _ = sqlx::query(
        "UPDATE sites SET owner_user_id = NULL, updated_at = NOW() WHERE id = $1 AND owner_user_id = $2"
    )
    .bind(site_uuid)
    .bind(user_id)
    .execute(&state.db)
    .await;

    if let Err(e) = state.reload_site_cache().await {
        tracing::warn!("site cache reload failed after site-access remove: {:?}", e);
    }

    Redirect::to(&format!("/admin/users/{}/site-access", user_id)).into_response()
}
