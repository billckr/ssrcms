//! Admin sites management page.

pub struct SiteRow {
    pub id: String,
    pub hostname: String,
    pub post_count: i64,
}

pub fn render_list(sites: &[SiteRow], flash: Option<&str>) -> String {
    let rows = sites.iter().map(|s| {
        format!(
            r#"<tr>
              <td>{hostname}</td>
              <td>{post_count}</td>
              <td class="actions">
                <form method="post" action="/admin/sites/switch" style="display:inline">
                  <input type="hidden" name="site_id" value="{id}">
                  <button type="submit" class="btn btn-secondary btn-sm">Switch</button>
                </form>
                <a href="/admin/sites/{id}/settings" class="btn btn-secondary btn-sm">Settings</a>
              </td>
            </tr>"#,
            id = crate::html_escape(&s.id),
            hostname = crate::html_escape(&s.hostname),
            post_count = s.post_count,
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<p style="margin-bottom:1rem"><a href="/admin/sites/new" class="btn btn-primary">New Site</a></p>
<table class="data-table">
  <thead><tr><th>Hostname</th><th>Posts</th><th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows,
    );

    crate::admin_page("Sites", "/admin/sites", flash, &content)
}

pub struct SiteSettingsData {
    pub id: String,
    pub hostname: String,
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>) -> String {
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
</form>"#,
        id = crate::html_escape(&data.id),
        hostname = crate::html_escape(&data.hostname),
    );

    crate::admin_page("Site Settings", "/admin/sites", flash, &content)
}

pub fn render_new(flash: Option<&str>) -> String {
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

    crate::admin_page("New Site", "/admin/sites", flash, content)
}
