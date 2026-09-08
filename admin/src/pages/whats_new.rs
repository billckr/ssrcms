//! Admin "What's New" page — GET /admin/whats-new.
//!
//! Shows the latest known SynapCMS release in-app (most admins won't know
//! or care what GitHub is), with a link out to GitHub kept deliberately
//! secondary/opt-in rather than the primary way to see what changed. When
//! an update is available and self-update is enabled, offers a one-click
//! "Update now" action via a progress-bar modal that POSTs to
//! /admin/self-update (core/src/handlers/admin/self_update.rs, JSON
//! request/response) — gated behind re-entering the current password.

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

/// "Update now" button + its confirm/progress modal. A single dialog with
/// two inner panels toggled by JS (`#su-confirm` / `#su-progress`) rather
/// than two separate dialogs, so there's one open/close lifecycle to manage.
fn update_button_html(tag: &str) -> String {
    let tag_escaped = html_escape(tag);
    format!(
        r##"<div style="margin-top:1rem">
  <button type="button" class="btn btn-primary" onclick="document.getElementById('self-update-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">Update to {tag_escaped}</button>
</div>

<dialog id="self-update-dialog" class="modal-card">
  <div class="modal-card-header">Update SynapCMS</div>
  <div class="modal-card-body">
    <div id="su-confirm">
      <p>Update to <strong>{tag_escaped}</strong>? Downloads and verifies the release, then restarts the server — a few seconds of downtime.</p>
      <div class="form-group">
        <label for="su-password">Confirm your password</label>
        <input type="password" id="su-password" required autocomplete="current-password">
      </div>
      <p id="su-error" class="error" hidden style="margin-top:.5rem"></p>
      <div style="display:flex;justify-content:flex-end;gap:.5rem;margin-top:1rem">
        <button type="button" class="btn" onclick="document.getElementById('self-update-dialog').close()">Cancel</button>
        <button type="button" class="btn btn-primary" onclick="startSelfUpdate('{tag_escaped}')">Update now</button>
      </div>
    </div>
    <div id="su-progress" hidden style="text-align:center;padding:1rem 0 .5rem">
      <div class="progress-bar"><div class="progress-bar-fill"></div></div>
      <p id="su-status" style="margin-top:1rem;color:var(--muted);font-size:.85rem">Downloading and verifying update&hellip;</p>
    </div>
  </div>
</dialog>

<script>
(function() {{
  var dialog = document.getElementById('self-update-dialog');
  // Don't let ESC/backdrop dismiss the dialog once an update is in flight —
  // the request keeps running server-side regardless, but closing the
  // modal mid-update would just be confusing.
  dialog.addEventListener('cancel', function(e) {{
    if (!document.getElementById('su-progress').hidden) e.preventDefault();
  }});
  dialog.addEventListener('close', function() {{
    document.querySelector('.admin-content').style.filter = '';
  }});
}})();

var SELF_UPDATE_MESSAGES = {{
  wrong_password: 'Current password is incorrect.',
  forbidden: "You don't have permission to do that.",
  disabled: 'Self-update is disabled on this install.',
  source_build: 'This is a source build — self-update needs a release-tarball install.',
  up_to_date: 'Already running the latest version.',
  unsupported_arch: "This host's architecture has no matching release build.",
  apply_failed: 'Update failed — nothing was changed. Check the server logs for details.',
  network: 'Could not reach the server. Check your connection and try again.'
}};

function startSelfUpdate(tag) {{
  var password = document.getElementById('su-password').value;
  document.getElementById('su-error').hidden = true;
  document.getElementById('su-confirm').hidden = true;
  document.getElementById('su-progress').hidden = false;

  fetch('/admin/self-update', {{
    method: 'POST',
    headers: {{'Content-Type': 'application/x-www-form-urlencoded'}},
    body: 'current_password=' + encodeURIComponent(password)
  }}).then(function(r) {{ return r.json(); }}).then(function(data) {{
    if (data.ok) {{
      document.getElementById('su-status').textContent = 'Update applied — restarting server…';
      pollUntilBackUp();
    }} else {{
      showSelfUpdateError(data.error);
    }}
  }}).catch(function() {{
    showSelfUpdateError('network');
  }});
}}

function showSelfUpdateError(code) {{
  document.getElementById('su-progress').hidden = true;
  document.getElementById('su-confirm').hidden = false;
  var el = document.getElementById('su-error');
  el.textContent = SELF_UPDATE_MESSAGES[code] || 'Update failed.';
  el.hidden = false;
}}

function pollUntilBackUp() {{
  var attempts = 0;
  var timer = setInterval(function() {{
    attempts++;
    fetch('/admin/whats-new', {{method: 'HEAD', cache: 'no-store'}}).then(function(r) {{
      if (r.ok) {{
        clearInterval(timer);
        window.location.reload();
      }}
    }}).catch(function() {{ /* still restarting — keep polling */ }});
    if (attempts >= 30) {{
      clearInterval(timer);
      document.getElementById('su-status').textContent =
        "This is taking longer than expected — the server may need a manual restart.";
      var reload = document.createElement('button');
      reload.type = 'button';
      reload.className = 'btn btn-primary';
      reload.style.marginTop = '.75rem';
      reload.textContent = 'Reload page';
      reload.onclick = function() {{ window.location.reload(); }};
      document.getElementById('su-progress').appendChild(reload);
    }}
  }}, 2000);
}}
</script>"##,
        tag_escaped = tag_escaped,
    )
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

    admin_page("What's New", "/admin/whats-new", None, &content, ctx)
}
