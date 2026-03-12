use crate::{admin_page, PageContext};

pub fn render_list(ctx: &PageContext) -> String {
    let content = r#"<p>Coming soon.</p>"#;
    admin_page("Media 2", "/admin/media2", None, content, ctx)
}
