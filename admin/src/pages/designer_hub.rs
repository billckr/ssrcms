//! `/admin/designer` — the consolidated Forms/Polls hub. Renders both
//! lists server-side and shows/hides them with the same `?tab=` +
//! sessionStorage tab convention used by `pages::settings` and the Form
//! Designer editor's own tabs, rather than lazy-loading each tab's content.
//!
//! Tabs + search + New live in one row, same layout as the Posts list
//! page (`pages::posts`) — `.page-tabs` and `.icon-pill` as flex siblings
//! in a shared row, with only the active tab's controls visible.

use crate::PageContext;

pub fn render(forms_fragment: &str, polls_fragment: &str, ctx: &PageContext, flash: Option<&str>) -> String {
    let forms_search = crate::pill_search_toggle("designer-forms-search", "Search forms\u{2026}", "");
    let polls_search = crate::pill_search_toggle("designer-polls-search", "Search polls\u{2026}", "");
    let forms_live_search = crate::live_search_script("designer-forms-search", "designer-forms-list", "/admin/form-designer?partial=1");
    let polls_live_search = crate::live_search_script("designer-polls-search", "designer-polls-list", "/admin/designer/polls?partial=1");

    let content = format!(
        r#"<style>
.settings-panel {{ display: none; }}
.settings-panel.active {{ display: block; }}
</style>
<div style="display:flex;align-items:flex-end;justify-content:space-between;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  <div class="page-tabs" style="margin-bottom:0" role="tablist">
    <button type="button" class="page-tab active" role="tab" aria-selected="true" aria-controls="tab-forms" data-tab="forms">Forms</button>
    <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-polls" data-tab="polls">Polls</button>
  </div>
  <div class="icon-pill tab-controls active" data-tab-controls="forms" style="align-self:flex-end;margin-top:0">
    {forms_search}
    <a href="/admin/form-designer/new" class="icon-btn" title="New Form" aria-label="New Form"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
  <div class="icon-pill tab-controls" data-tab-controls="polls" style="align-self:flex-end;margin-top:0;display:none">
    {polls_search}
    <a href="/admin/designer/polls/new" class="icon-btn" title="New Poll" aria-label="New Poll"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
</div>

<div id="tab-forms" class="settings-panel active" role="tabpanel">
<div id="designer-forms-list">{forms_fragment}</div>
</div>
<div id="tab-polls" class="settings-panel" role="tabpanel">
<div id="designer-polls-list">{polls_fragment}</div>
</div>

{forms_live_search}
{polls_live_search}
{pill_search_init}

<script>
(function () {{
  var tabs   = document.querySelectorAll('.page-tab[data-tab]');
  var panels = document.querySelectorAll('.settings-panel');
  var controls = document.querySelectorAll('.tab-controls[data-tab-controls]');

  function activate(tabName) {{
    tabs.forEach(function (btn) {{
      var on = btn.dataset.tab === tabName;
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-selected', on ? 'true' : 'false');
    }});
    panels.forEach(function (panel) {{
      panel.classList.toggle('active', panel.id === 'tab-' + tabName);
    }});
    controls.forEach(function (el) {{
      var on = el.dataset.tabControls === tabName;
      el.classList.toggle('active', on);
      el.style.display = on ? '' : 'none';
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
        forms_search = forms_search,
        polls_search = polls_search,
        forms_live_search = forms_live_search,
        polls_live_search = polls_live_search,
        pill_search_init = crate::pill_search_init_script(),
    );

    crate::admin_page("Designer", "/admin/designer", flash, &content, ctx)
}
