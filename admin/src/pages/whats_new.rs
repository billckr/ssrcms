//! Admin "What's New" page — GET /admin/whats-new.
//!
//! Shows the latest known SynapCMS release in-app (most admins won't know
//! or care what GitHub is), with a link out to GitHub kept deliberately
//! secondary/opt-in rather than the primary way to see what changed. When
//! an update is available and self-update is enabled, offers a one-click
//! "Update now" action (POST /admin/self-update, core/src/handlers/admin/
//! self_update.rs) gated behind re-entering the current password.

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
    /// True when there's an update to offer AND the viewer is allowed to
    /// trigger it (super-admin, not impersonating, self-update not disabled
    /// via config). Drives whether the "Update now" button renders at all.
    pub can_self_update: bool,
}

/// Password-confirm form + toggle button for triggering the self-updater.
/// Kept collapsed until clicked so a stray click can't submit it by accident.
fn update_button_html(tag: &str) -> String {
    format!(
        r##"<div style="margin-top:1rem">
  <button type="button" id="update-now-btn" onclick="this.hidden=true;var f=document.getElementById('update-now-form');f.hidden=false;f.style.display='flex';" class="btn btn-primary">Update to {tag}</button>
  <form id="update-now-form" method="post" action="/admin/self-update" hidden style="margin-top:.75rem;gap:.5rem;align-items:center">
    <input type="password" name="current_password" placeholder="Confirm your password" required autocomplete="current-password" style="flex:1;max-width:240px">
    <button type="submit" class="btn btn-primary">Confirm update</button>
  </form>
  <p style="margin-top:.5rem;font-size:.75rem;color:var(--muted)">Downloads and verifies the release, then restarts the server — a few seconds of downtime.</p>
</div>"##,
        tag = html_escape(tag),
    )
}

pub fn render(data: &WhatsNewData, flash: Option<&str>, ctx: &PageContext) -> String {
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
            let update_button = if data.can_self_update {
                update_button_html(tag)
            } else {
                String::new()
            };
            format!(
                r#"<div class="card" style="padding:1.5rem">
  <p style="margin:0"><strong>{headline}</strong></p>
  {body_html}
  {update_button}
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

    admin_page("What's New", "/admin/whats-new", flash, &content, ctx)
}
