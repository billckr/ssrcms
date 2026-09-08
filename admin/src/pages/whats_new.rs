//! Admin "What's New" page — GET /admin/whats-new.
//!
//! Shows the latest known SynapCMS release in-app (most admins won't know
//! or care what GitHub is), with a link out to GitHub kept deliberately
//! secondary/opt-in rather than the primary way to see what changed.

use pulldown_cmark::{html as cm_html, Options, Parser};

use crate::{admin_page, html_escape, PageContext};

fn render_markdown(md: &str) -> String {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(md, opts);
    let mut html = String::new();
    cm_html::push_html(&mut html, parser);
    html
}

pub struct WhatsNewData {
    /// This process's own version — see `synaptic_core::version::current_version`.
    pub current_version: String,
    /// Latest release known to the background release checker, if any has
    /// been fetched yet.
    pub latest_tag: Option<String>,
    pub latest_url: Option<String>,
    pub latest_body: Option<String>,
}

pub fn render(data: &WhatsNewData, ctx: &PageContext) -> String {
    let is_source_build = data.current_version.ends_with("-source");

    let status_card = match &data.latest_tag {
        None => r#"<div class="card" style="padding:1.5rem;color:var(--muted)">
  <p>No release information yet — this instance hasn't checked in with GitHub, or the check is disabled. Check back shortly.</p>
</div>"#.to_string(),
        Some(tag) if !is_source_build && tag == &data.current_version => format!(
            r#"<div class="card" style="padding:1.5rem">
  <p style="margin:0"><strong>You're up to date</strong> — running {version}.</p>
</div>"#,
            version = html_escape(&data.current_version),
        ),
        Some(tag) => {
            let headline = if is_source_build {
                format!("Latest known release: {}", html_escape(tag))
            } else {
                format!("A newer version is available: {}", html_escape(tag))
            };
            let body_html = match data.latest_body.as_deref().map(str::trim) {
                Some(b) if !b.is_empty() => format!(
                    r#"<div class="doc-content" style="margin-top:1rem;padding-top:1rem;border-top:1px solid var(--border)">{}</div>"#,
                    render_markdown(b)
                ),
                _ => String::new(),
            };
            let github_link = data
                .latest_url
                .as_deref()
                .map(|url| {
                    format!(
                        r#"<p style="margin-top:1rem"><a href="{url}" target="_blank" rel="noopener" style="font-size:.85rem">View this release on GitHub &rarr;</a></p>"#,
                        url = html_escape(url)
                    )
                })
                .unwrap_or_default();
            format!(
                r#"<div class="card" style="padding:1.5rem">
  <p style="margin:0"><strong>{headline}</strong></p>
  {body_html}
  {github_link}
</div>"#
            )
        }
    };

    let content = format!(
        r#"<div style="max-width:640px">
  <p style="color:var(--muted);margin:0 0 1.25rem">Running {version}.</p>
  {status_card}
  <p style="margin-top:1.5rem;font-size:.8rem;color:var(--muted)">
    <a href="https://github.com/billckr/ssrcms/releases" target="_blank" rel="noopener">See all releases on GitHub &rarr;</a>
  </p>
</div>"#,
        version = html_escape(&data.current_version),
    );

    admin_page("What's New", "/admin/whats-new", None, &content, ctx)
}
