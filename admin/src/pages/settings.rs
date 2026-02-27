//! Admin system settings page.

pub fn render(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = r#"<p>System-wide settings will appear here.</p>"#;
    crate::admin_page("System Settings", "/admin/settings", flash, content, ctx)
}
