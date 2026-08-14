//! Read-only admin view over the `audit_log` table — who created/deleted
//! which site or user, and when. See core/src/models/audit_log.rs.

use crate::{admin_page, html_escape, PageContext};

/// Pre-formatted row for display — built by the caller (core crate, which
/// has the `audit_log::AuditLogEntry` model and site-hostname lookups) from
/// [`humanize_action`] and [`role_display`] plus its own data. Kept as
/// plain strings, not the model type, the same way `mail_log`'s admin view
/// (`form_designer::MailLogRow`) doesn't depend on core — admin has no
/// dependency on the core crate (core depends on admin, not vice versa).
pub struct ActivityLogRow {
    pub created_at: String,
    pub actor_label: String,
    pub action_label: String,
    pub target_type: String,
    pub target_label: String,
    pub site_label: String,
}

pub fn role_display(role: &str) -> &str {
    match role {
        "super_admin" => "Super Admin",
        "site_admin" => "Site Admin",
        "cli" => "CLI",
        other => other,
    }
}

/// "site.created" -> "Site created", "site_user.added" -> "Added to site".
/// Falls back to a generic dot/underscore-to-space conversion for any
/// action not explicitly named here, so a new event type still reads fine
/// without needing this list updated in lockstep.
pub fn humanize_action(action: &str) -> String {
    match action {
        "site.created" => "Site created".to_string(),
        "site.deleted" => "Site deleted".to_string(),
        "user.created" => "User created".to_string(),
        "user.deleted" => "User deleted".to_string(),
        "user.suspended" => "User suspended".to_string(),
        "user.reactivated" => "User reactivated".to_string(),
        "site_user.added" => "Added to site".to_string(),
        "site_user.removed" => "Removed from site".to_string(),
        "post.deleted" => "Post deleted".to_string(),
        "page.deleted" => "Page deleted".to_string(),
        "category.deleted" => "Category deleted".to_string(),
        "tag.deleted" => "Tag deleted".to_string(),
        "media.deleted" => "Media deleted".to_string(),
        "media_folder.deleted" => "Media folder deleted".to_string(),
        "auth.login_succeeded" => "Login succeeded".to_string(),
        "auth.login_failed" => "Login failed".to_string(),
        other => {
            let s = other.replace(['.', '_'], " ");
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => s,
            }
        }
    }
}

fn action_badge_class(action: &str) -> &'static str {
    if action.ends_with("deleted") || action.ends_with("removed") || action.ends_with("suspended") || action.ends_with("failed") {
        "badge-danger"
    } else if action.ends_with("created") || action.ends_with("added") || action.ends_with("reactivated") || action.ends_with("succeeded") {
        "badge-published"
    } else {
        "badge"
    }
}

fn pagination(page: i64, total_pages: i64, site_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let base = "/admin/activity-log";
    let prev = if page > 1 {
        format!(r#"<a href="{base}?page={}{site_qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="{base}?page={}{site_qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(r#"<a href="{base}?page={p}{site_qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

pub fn render_list(
    rows: &[ActivityLogRow],
    page: i64,
    total_pages: i64,
    site_options: &[(String, String)],
    selected_site_id: &str,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    let site_qs = if selected_site_id.is_empty() {
        String::new()
    } else {
        format!("&site={}", html_escape(selected_site_id))
    };

    let site_filter = if site_options.is_empty() {
        String::new()
    } else {
        let all_selected = if selected_site_id.is_empty() { " selected" } else { "" };
        let opts: String = site_options.iter().map(|(id, hostname)| {
            let sel = if id == selected_site_id { " selected" } else { "" };
            format!(r#"<option value="{id}"{sel}>{hostname}</option>"#,
                id = html_escape(id), sel = sel, hostname = html_escape(hostname))
        }).collect();
        format!(
            r#"<select class="appearance-filter-select" aria-label="Filter by site"
                       onchange="window.location = this.value ? '/admin/activity-log?site=' + this.value : '/admin/activity-log'">
    <option value=""{all_selected}>All sites</option>
    {opts}
  </select>"#,
            all_selected = all_selected,
            opts = opts,
        )
    };

    let rows_html = if rows.is_empty() {
        r#"<tr><td colspan="5" class="empty-state">No activity recorded yet.</td></tr>"#.to_string()
    } else {
        rows.iter().map(|r| {
            format!(
                r#"<tr>
  <td style="white-space:nowrap;color:var(--muted);font-size:0.875rem">{created_at}</td>
  <td style="color:var(--muted);font-size:0.875rem">{actor}</td>
  <td><span class="badge {badge_class}">{action}</span></td>
  <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{target_type}</span> {target}</td>
  <td>{site}</td>
</tr>"#,
                created_at = html_escape(&r.created_at),
                actor = html_escape(&r.actor_label),
                badge_class = action_badge_class(&r.action_label.to_lowercase()),
                action = html_escape(&r.action_label),
                target_type = html_escape(&r.target_type),
                target = html_escape(&r.target_label),
                site = html_escape(&r.site_label),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let pag = pagination(page, total_pages, &site_qs);

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:flex-end;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  {site_filter}
</div>
<table class="data-table">
  <thead><tr><th>When</th><th>Who</th><th>Action</th><th>Target</th><th>Site</th></tr></thead>
  <tbody>{rows_html}</tbody>
</table>
{pag}"#,
        site_filter = site_filter,
        rows_html = rows_html,
        pag = pag,
    );

    admin_page("Activity Log", "/admin/activity-log", flash, &content, ctx)
}
