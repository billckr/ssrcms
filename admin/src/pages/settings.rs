//! Admin site settings page.

pub struct SettingsData {
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
}

pub fn render(data: &SettingsData, flash: Option<&str>, current_site: &str, is_global_admin: bool, visiting_foreign_site: bool, user_email: &str, can_manage_users: bool) -> String {
    let site_context_note = if current_site.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="form-context-note">You are editing settings for: <strong>{}</strong></p>"#,
            crate::html_escape(current_site)
        )
    };
    let content = format!(
        r#"{site_context_note}<form method="POST" action="/admin/settings">
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
        site_context_note = site_context_note,
        site_name = crate::html_escape(&data.site_name),
        site_description = crate::html_escape(&data.site_description),
        language = crate::html_escape(&data.language),
        posts_per_page = data.posts_per_page,
        date_format = crate::html_escape(&data.date_format),
    );

    crate::admin_page("Site Settings", "/admin/settings", flash, &content, current_site, is_global_admin, visiting_foreign_site, user_email, can_manage_users)
}
