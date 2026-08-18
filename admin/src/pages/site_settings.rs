//! Site admin's own "System Settings" page — their site's admin branding
//! (sidebar name/logo). Distinct from admin/src/pages/settings.rs, which is
//! the agency-wide System Settings page super_admin sees instead (never
//! both at once — see AdminCaps::can_manage_site_settings's doc comment).

pub fn render(
    flash: Option<&str>,
    brand_name: &str,
    has_site_logo: bool,
    ctx: &crate::PageContext,
) -> String {
    let brand_name_escaped = crate::html_escape(brand_name);
    let app_name_escaped = crate::html_escape(&ctx.app_name);

    // ctx.logo_url may be showing the system-wide fallback logo, not one this
    // site has set itself — that fallback belongs to the provider/agency
    // account, not this site, so it must never be displayed here. has_site_logo
    // (whether admin/static/branding/{site_id}/logo.* actually exists) is the
    // only thing this preview keys off; ctx.logo_url is only used when it's
    // actually this site's own upload.
    let current_logo_preview = if has_site_logo {
        format!(
            r#"<img src="{url}" alt="Current logo" style="height:38px;max-width:100%">"#,
            url = crate::html_escape(ctx.logo_url.as_deref().unwrap_or_default()),
        )
    } else {
        r#"<span class="field-hint" style="color:#94a3b8">No logo uploaded</span>"#.to_string()
    };
    let reset_logo_btn = if has_site_logo {
        r#"<button type="button" class="icon-btn" title="Reset to Text" aria-label="Reset to Text" onclick="resetLogoConfirm()">
              <img src="/admin/static/icons/rotate-ccw.svg" alt="">
            </button>"#.to_string()
    } else {
        String::new()
    };

    let content = format!(r#"
<style>
.settings-panel {{ display: none; max-width: 720px; }}
.settings-panel.active {{ display: block; }}
</style>

<!-- Tab bar -->
<div class="page-tabs" role="tablist">
  <button type="button" class="page-tab active" role="tab" aria-selected="true" aria-controls="tab-general" data-tab="general">General</button>
</div>

<!-- General -->
<div id="tab-general" class="settings-panel active" role="tabpanel">
  <div class="card-boxed">
    <h2 class="card-boxed-header">Site Branding</h2>
    <div class="card-boxed-body">
    <form method="post" action="/admin/site-settings" class="edit-form general-settings-form">
      <input type="hidden" name="tab" value="general">

      <div class="card-boxed-section">
        <div class="form-group" style="max-width:360px">
          <label for="sg-brand-name">Admin Sidebar Name</label>
          <input type="text" id="sg-brand-name" name="brand_name" value="{brand_name}" placeholder="{app_name_placeholder}">
          <small>Shown in the admin sidebar top-left for everyone logged into this site — keeps their branding consistent with the site they're managing. Leave blank to use the system-wide default ({app_name_placeholder}).</small>
        </div>
        <div class="icon-pill" style="margin-top:1rem">
          <button type="submit" id="general-save-btn" class="icon-btn" title="Save General" aria-label="Save General" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </form>
    </div>
  </div>

  <div class="card-boxed">
    <h2 class="card-boxed-header">Sidebar Logo</h2>
    <div class="card-boxed-body">
      <div class="card-boxed-section card-boxed-section-hidden">
        <label>Current</label>
        <div style="display:flex;align-items:center;padding:.75rem 1rem;margin-top:.4rem;background:#1e293b;border-radius:6px;max-width:360px">
          {current_logo_preview}
        </div>
      </div>
      <div class="card-boxed-section">
        <form method="post" action="/admin/site-settings/logo" enctype="multipart/form-data" id="logo-upload-form">
          <div class="form-group" style="max-width:360px">
            <label for="sg-logo-file">Replace with a new image</label>
            <input type="file" id="sg-logo-file" name="file" accept=".svg,.png,.webp,image/svg+xml,image/png,image/webp">
            <small>SVG, PNG, or WebP — max 2 MB. Renders at a fixed 38px sidebar height, so any aspect ratio works. Only affects this site's admin — the system-wide logo (if any) is unaffected.</small>
          </div>
          <div class="icon-pill" style="margin-top:1rem">
            <button type="submit" id="logo-upload-btn" class="icon-btn" title="Upload Logo" aria-label="Upload Logo" disabled>
              <img src="/admin/static/icons/save.svg" alt="">
            </button>
            {reset_logo_btn}
          </div>
        </form>
      </div>
    </div>
  </div>
</div>

<script>
function dtEnableOnChange(formSelector, btnId) {{
  var form = document.querySelector(formSelector);
  var btn  = document.getElementById(btnId);
  function snapshot() {{
    return Array.from(new FormData(form).entries()).map(function (e) {{ return e[0] + '=' + e[1]; }}).join('&');
  }}
  var initialSnapshot = snapshot();
  function checkChanged() {{
    btn.disabled = snapshot() === initialSnapshot;
  }}
  form.addEventListener('input', checkChanged);
  form.addEventListener('change', checkChanged);
}}
dtEnableOnChange('.general-settings-form', 'general-save-btn');

(function() {{
  var fileInput = document.getElementById('sg-logo-file');
  var uploadBtn = document.getElementById('logo-upload-btn');
  if (fileInput && uploadBtn) {{
    fileInput.addEventListener('change', function() {{
      uploadBtn.disabled = fileInput.files.length === 0;
    }});
  }}
}})();
window.resetLogoConfirm = function() {{
  if (!confirm('Remove this site\'s custom logo and go back to showing its name as text?')) return;
  fetch('/admin/site-settings/logo/reset', {{ method: 'POST' }}).then(function(r) {{
    window.location.href = r.url || '/admin/site-settings';
  }});
}};
</script>
"#,
        brand_name = brand_name_escaped,
        app_name_placeholder = app_name_escaped,
        current_logo_preview = current_logo_preview,
        reset_logo_btn = reset_logo_btn,
    );

    crate::admin_page("System Settings", "/admin/site-settings", flash, &content, ctx)
}
