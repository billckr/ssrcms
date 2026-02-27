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
    can_create: bool,
    ctx: &crate::PageContext,
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
                          data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
                      <button type="submit" class="icon-btn icon-danger" title="Delete site">
                        <img src="/admin/static/icons/trash-2.svg" alt="Delete">
                      </button>
                    </form>"#,
                    id = crate::html_escape(&s.id),
                    confirm_msg = crate::html_escape(&confirm_msg),
                )
            };
            format!(
                r#"<a href="/admin/sites/{id}/settings" class="icon-btn" title="Site Settings">
                  <img src="/admin/static/icons/edit.svg" alt="Site Settings">
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

    crate::admin_page("Sites", "/admin/sites", flash, &content, ctx)
}

pub struct SiteSettingsData {
    pub id: String,
    pub hostname: String,
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let confirm_msg = format!(
        "Are you sure you want to change the hostname? All links and routes for this site will use the new hostname immediately.",
    );
    let content = format!(
        r#"<p class="site-context-banner">Settings for: <strong>{hostname}</strong></p>
<form method="post" action="/admin/sites/{id}/settings" class="edit-form"
      data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
  <div class="form-group">
    <label for="hostname">Hostname</label>
    <input type="text" id="hostname" name="hostname" value="{hostname}" required>
  </div>
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Save</button>
    <a href="/admin/sites" class="btn btn-secondary">Cancel</a>
  </div>
</form>

<p class="site-context-banner" style="margin-top:2rem">Site Settings</p>
<form method="post" action="/admin/sites/{id}/site-config" class="edit-form">
  <div class="form-group">
    <label for="site_name">Site Name</label>
    <input type="text" id="site_name" name="site_name" value="{site_name}" required>
    <small>The display name shown in the browser tab, header, and footer.</small>
  </div>
  <div class="form-group">
    <label for="site_description">Site Description</label>
    <textarea id="site_description" name="site_description" rows="3">{site_description}</textarea>
  </div>
  <div class="form-group">
    <label for="language">Language</label>
    <input type="text" id="language" name="language" value="{language}">
  </div>
  <div class="form-group">
    <label for="posts_per_page">Posts Per Page</label>
    <input type="number" id="posts_per_page" name="posts_per_page" value="{posts_per_page}" min="1" max="100">
  </div>
  <div class="form-group">
    <label for="date_format">Date Format</label>
    <input type="text" id="date_format" name="date_format" value="{date_format}">
    <small>Uses chrono format strings, e.g. "%B %-d, %Y" &rarr; January 1, 2026</small>
  </div>
  <button type="submit" class="btn btn-primary">Save Settings</button>
</form>"#,
        id = crate::html_escape(&data.id),
        hostname = crate::html_escape(&data.hostname),
        confirm_msg = crate::html_escape(&confirm_msg),
        site_name = crate::html_escape(&data.site_name),
        site_description = crate::html_escape(&data.site_description),
        language = crate::html_escape(&data.language),
        posts_per_page = data.posts_per_page,
        date_format = crate::html_escape(&data.date_format),
    );

    crate::admin_page("Edit Hostname", "/admin/sites", flash, &content, ctx)
}

pub fn render_new(flash: Option<&str>, ctx: &crate::PageContext) -> String {
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

    crate::admin_page("New Site", "/admin/sites", flash, content, ctx)
}
