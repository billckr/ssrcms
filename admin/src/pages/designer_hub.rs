//! `/admin/designer` — the consolidated Forms/Polls hub. Renders both
//! lists server-side and shows/hides them with the same `?tab=` +
//! sessionStorage tab convention used by `pages::settings` and the Form
//! Designer editor's own tabs, rather than lazy-loading each tab's content.

use crate::PageContext;

pub fn render(forms_fragment: &str, polls_fragment: &str, ctx: &PageContext, flash: Option<&str>) -> String {
    let content = format!(
        r#"<style>
.settings-panel {{ display: none; }}
.settings-panel.active {{ display: block; }}
</style>
<div class="page-tabs" role="tablist">
  <button type="button" class="page-tab active" role="tab" aria-selected="true" aria-controls="tab-forms" data-tab="forms">Forms</button>
  <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-polls" data-tab="polls">Polls</button>
</div>

<div id="tab-forms" class="settings-panel active" role="tabpanel">
{forms_fragment}
</div>
<div id="tab-polls" class="settings-panel" role="tabpanel">
{polls_fragment}
</div>

<script>
(function () {{
  var tabs   = document.querySelectorAll('.page-tab[data-tab]');
  var panels = document.querySelectorAll('.settings-panel');

  function activate(tabName) {{
    tabs.forEach(function (btn) {{
      var on = btn.dataset.tab === tabName;
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-selected', on ? 'true' : 'false');
    }});
    panels.forEach(function (panel) {{
      panel.classList.toggle('active', panel.id === 'tab-' + tabName);
    }});
    try {{ sessionStorage.setItem('designer-tab', tabName); }} catch (e) {{}}
  }}

  tabs.forEach(function (btn) {{
    btn.addEventListener('click', function () {{ activate(btn.dataset.tab); }});
  }});

  var wantedTab = new URLSearchParams(window.location.search).get('tab');
  if (wantedTab && document.querySelector('.page-tab[data-tab="' + wantedTab + '"]')) {{
    activate(wantedTab);
  }} else {{
    try {{
      var saved = sessionStorage.getItem('designer-tab');
      if (saved) activate(saved);
    }} catch (e) {{}}
  }}
}}());
</script>"#,
        forms_fragment = forms_fragment,
        polls_fragment = polls_fragment,
    );

    crate::admin_page("Designer", "/admin/designer", flash, &content, ctx)
}
