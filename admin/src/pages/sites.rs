//! Admin sites management page.

pub struct SiteRow {
    pub id: String,
    pub hostname: String,
    pub post_count: i64,
    /// True for the first site created during CLI install — cannot be deleted.
    pub is_default: bool,
    /// True when the current user may edit settings / delete this site.
    pub can_manage: bool,
}

pub fn render_list(
    sites: &[SiteRow],
    flash: Option<&str>,
    current_site: &str,
    can_create: bool,
    is_global_admin: bool,
    visiting_foreign_site: bool,
    user_email: &str,
) -> String {
    let rows = sites.iter().map(|s| {
        let manage_html = if s.can_manage {
            let delete_html = if s.is_default {
                String::new()
            } else {
                let confirm_msg = format!(
                    "Delete site '{}'? This will permanently delete all its content, media records, settings, and user assignments. This cannot be undone.",
                    s.hostname.replace('\'', "\\'")
                );
                format!(
                    r#"<form method="post" action="/admin/sites/{id}/delete" style="display:inline"
                          onsubmit="return confirm('{confirm_msg}')">
                      <button type="submit" class="icon-btn icon-danger" title="Delete site">
                        <img src="/admin/static/icons/trash-2.svg" alt="Delete">
                      </button>
                    </form>"#,
                    id = crate::html_escape(&s.id),
                    confirm_msg = confirm_msg,
                )
            };
            format!(
                r#"<a href="/admin/sites/{id}/settings" class="icon-btn" title="Site settings">
                  <img src="/admin/static/icons/edit.svg" alt="Settings">
                </a>
                {delete}"#,
                id = crate::html_escape(&s.id),
                delete = delete_html,
            )
        } else {
            String::new()
        };

        format!(
            r#"<tr>
              <td>{hostname}{default_badge}</td>
              <td>{post_count}</td>
              <td class="actions">
                <form method="post" action="/admin/sites/switch" style="display:inline">
                  <input type="hidden" name="site_id" value="{id}">
                  <button type="submit" class="icon-btn" title="Switch to this site">
                    <img src="/admin/static/icons/play.svg" alt="Switch">
                  </button>
                </form>
                {manage}
              </td>
            </tr>"#,
            id = crate::html_escape(&s.id),
            hostname = crate::html_escape(&s.hostname),
            default_badge = if s.is_default { r#" <span class="badge" title="Install site — cannot be deleted">default</span>"# } else { "" },
            post_count = s.post_count,
            manage = manage_html,
        )
    }).collect::<Vec<_>>().join("\n");

    let new_site_btn = if can_create {
        r#"<p style="margin-bottom:1rem"><a href="/admin/sites/new" class="btn btn-primary">New Site</a></p>"#
    } else {
        ""
    };

    let content = format!(
        r#"{new_site_btn}<table class="data-table">
  <thead><tr><th>Hostname</th><th>Posts</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        new_site_btn = new_site_btn,
        rows = rows,
    );

    crate::admin_page("Sites", "/admin/sites", flash, &content, current_site, is_global_admin, visiting_foreign_site, user_email)
}

pub struct SiteSettingsData {
    pub id: String,
    pub hostname: String,
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>, current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str) -> String {
    let confirm_msg = format!(
        "Delete site '{}'? This will permanently delete all its content, media records, settings, and user assignments. This cannot be undone.",
        data.hostname
    );
    let content = format!(
        r#"<form method="post" action="/admin/sites/{id}/settings" class="edit-form">
  <div class="form-group">
    <label for="hostname">Hostname</label>
    <input type="text" id="hostname" name="hostname" value="{hostname}" required>
    <small>The domain this site responds to (e.g. example.com)</small>
  </div>
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Save</button>
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
</form>
<hr style="margin:2rem 0">
<form method="post" action="/admin/sites/{id}/delete" data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
  <button type="submit" class="btn btn-danger">Delete This Site</button>
</form>"#,
        id = crate::html_escape(&data.id),
        hostname = crate::html_escape(&data.hostname),
        confirm_msg = crate::html_escape(&confirm_msg),
    );

    crate::admin_page("Site Settings", "/admin/sites", flash, &content, current_site, is_global_admin, visiting_foreign_site, user_email)
}

pub fn render_new(flash: Option<&str>, current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str) -> String {
    let content = r#"<form method="post" action="/admin/sites" class="edit-form">
  <div class="form-group">
    <label for="hostname">Hostname</label>
    <input type="text" id="hostname" name="hostname" required placeholder="example.com">
    <small>The domain this site will respond to</small>
  </div>
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Create Site</button>
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
</form>"#;

    crate::admin_page("New Site", "/admin/sites", flash, content, current_site, is_global_admin, visiting_foreign_site, user_email)
}
