//! Form Designer: build reusable form definitions (fields + settings) that
//! can later be inserted into posts/pages. This module only renders the
//! list/create/edit admin pages — storage lives in `models::form_def`.

use crate::{admin_page, html_escape, PageContext};

/// One row for the list view.
pub struct FormRow {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub field_count: usize,
    pub updated_at: String,
    pub blocked: bool,
}

/// A single field, as rendered into the editor (both for existing fields on
/// edit and, via the same markup shape, what JS builds for new rows).
pub struct FieldRow {
    pub label: String,
    pub name: String,
    pub field_type: String,
    pub required: bool,
    /// One "value|Label" pair per line, only used by select/radio.
    pub options_text: String,
}

/// One site-configured (and verified) email provider, offered as an option
/// in a form's "Send via" dropdown.
pub struct ProviderOption {
    pub id: String,
    pub label: String,
}

/// Data needed to render the create/edit form. `id` is `None` for create.
pub struct FormEditData {
    pub id: Option<String>,
    pub name: String,
    pub fields: Vec<FieldRow>,
    pub success_message: String,
    pub button_label: String,
    pub include_honeypot: bool,
    /// Empty string means "no notification email" — matches how the input
    /// field itself represents "unset".
    pub notify_email: String,
    /// Auto-reply to the submitter using the form's first `email`-type
    /// field's submitted value.
    pub confirm_submitter: bool,
    pub confirm_subject: String,
    pub confirm_body: String,
    /// When true, submissions still save as normal but no admin-notify or
    /// submitter-confirmation email is ever sent for this form.
    pub no_mail: bool,
    /// Empty string means "use the install-wide account" — matches the
    /// dropdown's own empty-option value.
    pub email_provider_id: String,
    /// The site's verified providers, to populate the dropdown.
    pub provider_options: Vec<ProviderOption>,
    /// The site this form belongs to — used to link the "Send via" hint to
    /// that site's Email Settings tab.
    pub site_id: String,
}

impl Default for FormEditData {
    fn default() -> Self {
        FormEditData {
            id: None,
            name: String::new(),
            fields: vec![FieldRow {
                label: String::new(),
                name: String::new(),
                field_type: "text".to_string(),
                required: true,
                options_text: String::new(),
            }],
            success_message: "Thank you for your submission!".to_string(),
            button_label: "Submit".to_string(),
            include_honeypot: true,
            notify_email: String::new(),
            confirm_submitter: false,
            confirm_subject: "We've received your submission".to_string(),
            confirm_body: "Thanks for reaching out! We've received your submission and will follow up soon.".to_string(),
            no_mail: false,
            email_provider_id: String::new(),
            provider_options: Vec::new(),
            site_id: String::new(),
        }
    }
}

const FIELD_TYPES: &[(&str, &str)] = &[
    ("text", "Text"),
    ("email", "Email"),
    ("textarea", "Textarea"),
    ("number", "Number"),
    ("phone", "Phone"),
    ("date", "Date"),
    ("select", "Dropdown"),
    ("radio", "Radio group"),
    ("checkbox", "Checkbox"),
    ("toggle", "Toggle"),
    ("separator", "Separator line"),
    ("note", "Note / callout"),
];

/// `separator` and `note` are visual-only elements — nothing is submitted
/// for them, so "Field name" and "Required" don't apply and are hidden.
fn is_visual_only(field_type: &str) -> bool {
    matches!(field_type, "separator" | "note")
}

/// (label-of-the-label-input, placeholder) for the "Field label" input,
/// which doubles as an optional section title for separators and as the
/// callout text itself for notes.
fn field_label_meta(field_type: &str) -> (&'static str, &'static str) {
    match field_type {
        "separator" => ("Section title (optional)", "e.g. Shipping details"),
        "note" => ("Note text", "e.g. We'll never share your email with anyone."),
        _ => ("Field label", "e.g. Your name"),
    }
}

fn forms_pagination(page: i64, total_pages: i64, search_qs: &str, sort_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let qs = format!("{search_qs}{sort_qs}");
    let prev = if page > 1 {
        format!(r#"<a href="/admin/form-designer?page={}{qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="/admin/form-designer?page={}{qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(r#"<a href="/admin/form-designer?page={p}{qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Table + pagination only — swapped by the live-search JS, and reused for
/// the initial full-page render so both paths render identically.
pub fn forms_list_fragment(rows: &[FormRow], page: i64, total_pages: i64, search: &str, sort: &str, dir: &str) -> String {
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", html_escape(search))
    };
    let sort_qs = if sort.is_empty() { String::new() } else { format!("&sort={}&dir={}", sort, if dir == "desc" { "desc" } else { "asc" }) };
    let asc = dir != "desc";

    // Sortable column header: link toggles asc/desc for that column, preserving
    // the current search filter and resetting to page 1 (a new sort is a new view).
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == key;
        let next_dir = if is_active && asc { "desc" } else { "asc" };
        let arrow = if is_active { if asc { " \u{25B2}" } else { " \u{25BC}" } } else { "" };
        format!(
            r#"<th><a href="/admin/form-designer?sort={key}&dir={next_dir}{search_qs}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    // Edit/Export/Block/Delete all now live on the Forms tab of
    // /admin/analytics instead — this list is just for managing form
    // structure (create/rename/reorder fields) is reached via the Edit
    // icon rather than the name itself now, matching the Forms tab's
    // Edit/Analytics pattern on /admin/analytics?tab=forms.
    let body = if rows.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">No forms yet. Create one to get started. <a href="/admin/form-designer/new">Create</a></td></tr>"#.to_string()
    } else {
        rows.iter().map(|f| {
            format!(
                r#"<tr>
  <td>{name}</td>
  <td><code>{slug}</code></td>
  <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{count}</span></td>
  <td class="actions">
    <div class="icon-pill-actionbuttons">
      <a href="/admin/form-designer/{id}" class="icon-btn" title="Edit" aria-label="Edit">
        <img src="/admin/static/icons/edit.svg" alt="">
      </a>
    </div>
  </td>
</tr>"#,
                id = html_escape(&f.id),
                name = html_escape(&f.name),
                slug = html_escape(&f.slug),
                count = f.field_count,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    format!(
        r#"<table class="data-table">
  <thead><tr>{name_th}{slug_th}{fields_th}<th>Actions</th></tr></thead>
  <tbody>{body}</tbody>
</table>
{pagination}"#,
        body = body,
        pagination = forms_pagination(page, total_pages, &search_qs, &sort_qs),
        name_th = sort_th("Name", "name"),
        slug_th = sort_th("Slug", "slug"),
        fields_th = sort_th("Fields", "fields"),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_list(rows: &[FormRow], page: i64, total_pages: i64, search: &str, sort: &str, dir: &str, ctx: &PageContext, flash: Option<&str>) -> String {
    let fragment = forms_list_fragment(rows, page, total_pages, search, sort, dir);
    let sort_qs = if sort.is_empty() { String::new() } else { format!("&sort={}&dir={}", sort, if dir == "desc" { "desc" } else { "asc" }) };
    let fetch_prefix = format!("/admin/form-designer?partial=1{}", sort_qs);
    let live_search = crate::live_search_script("form-search", "form-designer-list", &fetch_prefix);
    let search_toggle = crate::pill_search_toggle("form-search", "Search forms&hellip;", search);

    let content = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:flex-end;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">
    {search_toggle}
    <a href="/admin/form-designer/new" class="icon-btn" title="New Form" aria-label="New Form"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
  </div>
</div>
<div id="form-designer-list">{fragment}</div>
{live_search}
{pill_search_init}"#,
        search_toggle = search_toggle,
        fragment = fragment,
        live_search = live_search,
        pill_search_init = crate::pill_search_init_script(),
    );

    admin_page("Form Designer", "/admin/form-designer", flash, &content, ctx)
}

/// (has_options, options label text, hint, placeholder) for a field type.
/// `toggle` reuses the same value|Label options mechanism as select/radio,
/// but means something different: exactly two lines, off state then on
/// state — e.g. `off|Off` / `on|On`, or `no|Disabled` / `yes|Enabled`.
fn options_meta(field_type: &str) -> (bool, &'static str, &'static str, &'static str) {
    match field_type {
        "toggle" => (
            true,
            "Off / On labels",
            "exactly two lines — off state, then on state",
            "e.g.&#10;off|Off&#10;on|On",
        ),
        "select" | "radio" => (
            true,
            "Options",
            "one per line — \"value|Label\", or just \"Label\"",
            "e.g.&#10;yes|Yes&#10;no|No",
        ),
        _ => (false, "Options", "", ""),
    }
}

fn field_row_html(f: &FieldRow, index: usize) -> String {
    let type_opts: String = FIELD_TYPES.iter().map(|(val, label)| {
        let sel = if f.field_type == *val { " selected" } else { "" };
        format!(r#"<option value="{val}"{sel}>{label}</option>"#, val = val, label = label, sel = sel)
    }).collect();

    let (has_options, options_label, options_hint, options_placeholder) = options_meta(&f.field_type);
    let options_display = if has_options { "" } else { "display:none" };
    let (label_of_label, label_placeholder) = field_label_meta(&f.field_type);
    let visual_only_display = if is_visual_only(&f.field_type) { "display:none" } else { "" };

    format!(
        r#"<div class="field-row" data-index="{index}" style="border:1px solid var(--border);border-radius:var(--radius);padding:.85rem 1rem;margin-bottom:.6rem;background:var(--tint)">
  <div style="display:flex;align-items:flex-start;gap:.6rem">
    <span class="drag-handle" title="Drag to reorder" draggable="true" style="margin-top:1.6rem">
      <img src="/admin/static/icons/move.svg" alt="">
    </span>
    <div style="flex:1;display:grid;grid-template-columns:1fr 1fr 1fr auto;gap:.6rem;align-items:end">
      <div class="form-group" style="margin:0">
        <label class="field-label-caption">{label_of_label}</label>
        <input type="text" class="field-label" maxlength="255" value="{label}" placeholder="{label_placeholder}">
      </div>
      <div class="form-group field-name-wrap" style="margin:0;{visual_only_display}">
        <label>Field name <span class="field-hint">(used in submissions)</span></label>
        <input type="text" class="field-name" maxlength="100" value="{name}" placeholder="e.g. name">
      </div>
      <div class="form-group" style="margin:0">
        <label>Type</label>
        <select class="field-type">{type_opts}</select>
      </div>
      <button type="button" class="icon-btn icon-danger field-remove" title="Remove field" style="margin-bottom:.45rem">
        <img src="/admin/static/icons/trash.svg" alt="Remove">
      </button>
    </div>
  </div>
  <div class="field-required-wrap" style="margin-left:2.2rem;margin-top:.5rem;display:flex;align-items:center;gap:1.2rem;{visual_only_display}">
    <label style="display:flex;align-items:center;gap:.4rem;font-size:13px;cursor:pointer">
      <input type="checkbox" class="field-required"{required}>
      Required
    </label>
  </div>
  <div class="field-options-wrap form-group" style="margin:.6rem 0 0 2.2rem;{options_display}">
    <label class="field-options-label">{options_label} <span class="field-hint field-options-hint">({options_hint})</span></label>
    <textarea class="field-options" rows="3" placeholder="{options_placeholder}">{options_text}</textarea>
  </div>
</div>"#,
        index = index,
        label = html_escape(&f.label),
        label_of_label = label_of_label,
        label_placeholder = label_placeholder,
        name = html_escape(&f.name),
        type_opts = type_opts,
        required = if f.required { " checked" } else { "" },
        visual_only_display = visual_only_display,
        options_display = options_display,
        options_label = options_label,
        options_hint = options_hint,
        options_placeholder = options_placeholder,
        options_text = html_escape(&f.options_text),
    )
}

pub fn render_editor(data: &FormEditData, ctx: &PageContext, flash: Option<&str>) -> String {
    let is_edit = data.id.is_some();
    let action = match &data.id {
        Some(id) => format!("/admin/form-designer/{id}"),
        None => "/admin/form-designer".to_string(),
    };
    let title = if is_edit { format!("Editing form - {}", html_escape(&data.name)) } else { "New Form".to_string() };

    let rows_html: String = data.fields.iter().enumerate()
        .map(|(i, f)| field_row_html(f, i))
        .collect::<Vec<_>>()
        .join("\n");

    let provider_options_html: String = data.provider_options.iter().map(|p| {
        let selected = if p.id == data.email_provider_id { " selected" } else { "" };
        format!(r#"<option value="{id}"{selected}>{label}</option>"#, id = html_escape(&p.id), label = html_escape(&p.label))
    }).collect();

    // Delete lives as an icon button next to Save (see below) rather than a
    // nested <form> — the whole editor is already one big <form>, and nested
    // forms are invalid HTML, so it goes through fetch() instead, same
    // pattern as the post editor's delete button.
    let delete_btn = if let Some(id) = &data.id {
        format!(
            r#"<button type="button" class="icon-btn icon-danger" title="Delete Form" aria-label="Delete Form"
        onclick="event.preventDefault();event.stopPropagation();deleteFormConfirm('{id}')">
    <img src="/admin/static/icons/trash.svg" alt="">
  </button>"#,
            id = html_escape(id)
        )
    } else {
        String::new()
    };

    // Only a saved form has anywhere to go back to — a new, not-yet-created
    // form has no row on the Forms tab yet. bar-chart-2 (not bar-chart, which
    // is already the per-form "view metrics" icon on the Forms tab itself)
    // keeps the two visually distinct despite both being analytics-related.
    let analytics_link = if let Some(id) = &data.id {
        format!(
            r#"<a href="/admin/analytics?tab=forms&form={id}" class="icon-btn" title="Analytics" aria-label="Analytics">
    <img src="/admin/static/icons/bar-chart-2.svg" alt="">
  </a>"#,
            id = html_escape(id),
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"<style>
.field-hint {{ font-size: 11px; color: var(--muted); font-weight: 400; }}
.form-tab-panel {{ display: none; }}
.form-tab-panel.active {{ display: block; }}
</style>
<form method="POST" action="{action}" id="form-designer-form">
  <div class="two-col">
    <div>
      <div class="card-boxed">
        <h2 class="card-boxed-header">Fields</h2>
        <div class="card-boxed-body">
          <div id="field-rows">{rows_html}</div>
          <input type="hidden" name="fields_json" id="fields-json">
          <input type="hidden" name="active_tab" id="active-tab" value="general">
          <div class="icon-pill">
            <button type="button" id="add-field-btn" class="icon-btn" title="Add Field" aria-label="Add Field">
              <img src="/admin/static/icons/file-plus.svg" alt="">
            </button>
          </div>
        </div>
      </div>
    </div>
    <div>
      <div class="card-boxed" style="position:sticky;top:1rem">
        <h2 class="card-boxed-header">Form Settings</h2>
        <div class="card-boxed-body">
          <div class="page-tabs" role="tablist" style="margin:0 0 1rem">
            <button type="button" class="page-tab active" role="tab" aria-selected="true" aria-controls="tab-general" data-tab="general">General Settings</button>
            <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-mail" data-tab="mail">Mail Settings</button>
            <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-preview" data-tab="preview">Preview</button>
          </div>
          <div id="tab-general" class="form-tab-panel active" role="tabpanel">
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="form-name">Form name</label>
              <input type="text" id="form-name" name="name" required minlength="5" maxlength="255" value="{name}" placeholder="e.g. Contact Us">
            </div>
          </div>
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="success-message">Success message</label>
              <input type="text" id="success-message" name="success_message" maxlength="200" value="{success_message}">
            </div>
          </div>
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="button-label">Submit button label</label>
              <input type="text" id="button-label" name="button_label" maxlength="40" value="{button_label}">
            </div>
          </div>
          <div class="card-boxed-section">
            <label class="switch-toggle">
              <input type="checkbox" id="include-honeypot" name="include_honeypot" value="true"{honeypot_checked}>
              <span class="switch-slider"></span>
              <span>Include spam honeypot field</span>
            </label>
          </div>
          <div class="card-boxed-section">
            <div class="form-note" style="margin-bottom:0">
              <p><strong>Requirements:</strong></p>
              <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
                <li id="form-req-name"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>Form name (5-255 characters)</li>
                <li id="form-req-fields"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one field, each with a label and field name</li>
              </ul>
            </div>
          </div>
          <div class="icon-pill">
            <button type="button" id="save-form-btn" class="icon-btn" title="{save_label}" aria-label="{save_label}"
                    onclick="event.preventDefault();event.stopPropagation();document.getElementById('form-designer-form').requestSubmit();">
              <img src="/admin/static/icons/save.svg" alt="">
            </button>
            {analytics_link}
            {delete_btn}
          </div>
          </div>
          <div id="tab-mail" class="form-tab-panel" role="tabpanel">
          <div class="card-boxed-section">
            <label class="switch-toggle">
              <input type="checkbox" id="no-mail" name="no_mail" value="true"{no_mail_checked}>
              <span class="switch-slider"></span>
              <span>Don't send any email for this form</span>
            </label>
            <p class="field-hint">Submissions still save to the database as normal — no admin notification or submitter confirmation is sent.</p>
          </div>
          <div id="mail-fields-wrap" style="display:{mail_fields_display}">
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="email-provider">Send via</label>
              <select id="email-provider" name="email_provider_id">
                <option value="">Install-wide default account</option>
                {provider_options_html}
              </select>
              <p class="field-hint">Configure providers on this site's Settings &rarr; <a href="/admin/sites/{site_id}/settings?tab=email" target="_blank" rel="noopener">Email Settings</a> tab.</p>
            </div>
          </div>
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="notify-email">Notify on new submission</label>
              <input type="email" id="notify-email" name="notify_email" maxlength="255" value="{notify_email}" placeholder="you@example.com">
              <p class="field-hint">Leave blank to disable. Sent via the site's configured Mailgun account.</p>
            </div>
          </div>
          <div class="card-boxed-section">
            <label class="switch-toggle">
              <input type="checkbox" id="confirm-submitter" name="confirm_submitter" value="true"{confirm_submitter_checked}>
              <span class="switch-slider"></span>
              <span>Email the submitter a confirmation</span>
            </label>
            <p class="field-hint">Sends to whatever the form's first Email-type field collects. Add one to the fields above if this form doesn't have one yet.</p>
          </div>
          <div class="card-boxed-section" id="confirm-fields" style="display:{confirm_fields_display}">
            <div class="form-group">
              <label for="confirm-subject">Confirmation subject</label>
              <input type="text" id="confirm-subject" name="confirm_subject" maxlength="200" value="{confirm_subject}">
            </div>
            <div class="form-group" style="margin-top:.6rem">
              <label for="confirm-body">Confirmation message</label>
              <textarea id="confirm-body" name="confirm_body" rows="4" style="resize:vertical">{confirm_body}</textarea>
              <p class="field-hint">Use <code>{{{{field_name}}}}</code> to insert a submitted value, e.g. <code>{{{{name}}}}</code>.</p>
            </div>
          </div>
          </div>
          <div class="icon-pill">
            <button type="button" id="save-mail-btn" class="icon-btn" title="{save_label}" aria-label="{save_label}"
                    onclick="event.preventDefault();event.stopPropagation();document.getElementById('form-designer-form').requestSubmit();">
              <img src="/admin/static/icons/save.svg" alt="">
            </button>
          </div>
          </div>
          <div id="tab-preview" class="form-tab-panel" role="tabpanel">
            <p class="form-note" style="margin:0 0 1rem">
              A live mockup — this is what visitors will see. It isn't wired to submit anything from here.
              Note that the actual form will pick up the active theme's colors and fonts, so it may not
              look exactly like this preview once it's embedded on a page.
            </p>
            <div id="form-preview"></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</form>
<script>
(function() {{
  var editorTabs = document.querySelectorAll('.page-tab[data-tab]');
  var editorPanels = document.querySelectorAll('.form-tab-panel');
  var activeTabField = document.getElementById('active-tab');
  function activate(btn) {{
    editorTabs.forEach(function(b) {{
      var on = b === btn;
      b.classList.toggle('active', on);
      b.setAttribute('aria-selected', on ? 'true' : 'false');
    }});
    editorPanels.forEach(function(panel) {{
      panel.classList.toggle('active', panel.id === 'tab-' + btn.dataset.tab);
    }});
    if (activeTabField) activeTabField.value = btn.dataset.tab;
  }}
  editorTabs.forEach(function(btn) {{
    btn.addEventListener('click', function() {{ activate(btn); }});
  }});
  var wantedTab = new URLSearchParams(window.location.search).get('tab');
  if (wantedTab) {{
    var wantedBtn = document.querySelector('.page-tab[data-tab="' + wantedTab + '"]');
    if (wantedBtn) activate(wantedBtn);
  }}
}})();
(function() {{
  var FIELD_TYPES_WITH_OPTIONS = ['select', 'radio', 'toggle'];
  var VISUAL_ONLY_TYPES = ['separator', 'note'];
  var OPTIONS_META = {{
    toggle: {{ label: 'Off / On labels', hint: 'exactly two lines — off state, then on state', placeholder: 'e.g.\noff|Off\non|On' }},
    select: {{ label: 'Options', hint: 'one per line — "value|Label", or just "Label"', placeholder: 'e.g.\nyes|Yes\nno|No' }},
    radio:  {{ label: 'Options', hint: 'one per line — "value|Label", or just "Label"', placeholder: 'e.g.\nyes|Yes\nno|No' }}
  }};
  var LABEL_META = {{
    separator: {{ caption: 'Section title (optional)', placeholder: 'e.g. Shipping details' }},
    note:      {{ caption: 'Note text', placeholder: "e.g. We'll never share your email with anyone." }}
  }};
  var container = document.getElementById('field-rows');
  var addBtn = document.getElementById('add-field-btn');
  var form = document.getElementById('form-designer-form');

  function toSlugName(s) {{
    return s.toLowerCase().replace(/[^a-z0-9\s_-]/g, '').trim().replace(/[\s-]+/g, '_');
  }}

  function makeRow() {{
    var row = document.createElement('div');
    row.className = 'field-row';
    row.style.cssText = 'border:1px solid var(--border);border-radius:var(--radius);padding:.85rem 1rem;margin-bottom:.6rem;background:var(--tint)';
    var typeOpts = {type_opts_js}.map(function(t) {{
      return '<option value="' + t[0] + '">' + t[1] + '</option>';
    }}).join('');
    row.innerHTML =
      '<div style="display:flex;align-items:flex-start;gap:.6rem">' +
        '<span class="drag-handle" title="Drag to reorder" draggable="true" style="margin-top:1.6rem"><img src="/admin/static/icons/move.svg" alt=""></span>' +
        '<div style="flex:1;display:grid;grid-template-columns:1fr 1fr 1fr auto;gap:.6rem;align-items:end">' +
          '<div class="form-group" style="margin:0"><label class="field-label-caption">Field label</label><input type="text" class="field-label" maxlength="255" placeholder="e.g. Your name"></div>' +
          '<div class="form-group field-name-wrap" style="margin:0"><label>Field name <span class="field-hint">(used in submissions)</span></label><input type="text" class="field-name" maxlength="100" placeholder="e.g. name"></div>' +
          '<div class="form-group" style="margin:0"><label>Type</label><select class="field-type">' + typeOpts + '</select></div>' +
          '<button type="button" class="icon-btn icon-danger field-remove" title="Remove field" style="margin-bottom:.45rem"><img src="/admin/static/icons/trash.svg" alt="Remove"></button>' +
        '</div>' +
      '</div>' +
      '<div class="field-required-wrap" style="margin-left:2.2rem;margin-top:.5rem;display:flex;align-items:center;gap:1.2rem">' +
        '<label style="display:flex;align-items:center;gap:.4rem;font-size:13px;cursor:pointer"><input type="checkbox" class="field-required">Required</label>' +
      '</div>' +
      '<div class="field-options-wrap form-group" style="margin:.6rem 0 0 2.2rem;display:none">' +
        '<label class="field-options-label">Options <span class="field-hint field-options-hint"></span></label>' +
        '<textarea class="field-options" rows="3"></textarea>' +
      '</div>';
    return row;
  }}

  function updateOptionsMeta(row) {{
    var type = row.querySelector('.field-type').value;
    var meta = OPTIONS_META[type];
    if (!meta) return;
    row.querySelector('.field-options-label').firstChild.textContent = meta.label + ' ';
    row.querySelector('.field-options-hint').textContent = '(' + meta.hint + ')';
    row.querySelector('.field-options').placeholder = meta.placeholder;
  }}

  function updateFieldMeta(row) {{
    var type = row.querySelector('.field-type').value;
    var meta = LABEL_META[type];
    row.querySelector('.field-label-caption').textContent = meta ? meta.caption : 'Field label';
    row.querySelector('.field-label').placeholder = meta ? meta.placeholder : 'e.g. Your name';
    var visualOnly = VISUAL_ONLY_TYPES.indexOf(type) !== -1;
    row.querySelector('.field-name-wrap').style.display = visualOnly ? 'none' : '';
    row.querySelector('.field-required-wrap').style.display = visualOnly ? 'none' : '';
  }}

  function wireRow(row) {{
    var labelInput = row.querySelector('.field-label');
    var nameInput  = row.querySelector('.field-name');
    var typeSelect = row.querySelector('.field-type');
    var optionsWrap = row.querySelector('.field-options-wrap');
    var nameTouched = !!(nameInput.value && nameInput.value.trim());

    labelInput.addEventListener('input', function() {{
      if (!nameTouched) nameInput.value = toSlugName(labelInput.value);
      updatePreview();
    }});
    nameInput.addEventListener('input', function() {{ nameTouched = nameInput.value.trim().length > 0; updatePreview(); }});
    typeSelect.addEventListener('change', function() {{
      optionsWrap.style.display = FIELD_TYPES_WITH_OPTIONS.indexOf(typeSelect.value) !== -1 ? '' : 'none';
      updateOptionsMeta(row);
      updateFieldMeta(row);
      updatePreview();
    }});
    row.querySelector('.field-required').addEventListener('change', updatePreview);
    row.querySelector('.field-options').addEventListener('input', updatePreview);
    row.querySelector('.field-remove').addEventListener('click', function() {{
      var label = row.querySelector('.field-label').value.trim() || row.querySelector('.field-name').value.trim() || 'this field';
      if (!confirm('Remove "' + label + '"?')) return;
      row.remove();
      updatePreview();
    }});
    if (!row.querySelector('.field-options').placeholder) updateOptionsMeta(row);
    updateFieldMeta(row);
  }}

  container.querySelectorAll('.field-row').forEach(wireRow);

  addBtn.addEventListener('click', function() {{
    var row = makeRow();
    container.appendChild(row);
    wireRow(row);
    row.querySelector('.field-label').focus();
    updatePreview();
  }});

  // Drag-to-reorder, same pattern as the menu editor's item list.
  var dragEl = null;
  container.addEventListener('dragstart', function(e) {{
    if (!e.target.classList.contains('drag-handle') && !e.target.closest('.drag-handle')) return;
    dragEl = e.target.closest('.field-row');
    e.dataTransfer.effectAllowed = 'move';
  }});
  container.addEventListener('dragover', function(e) {{
    e.preventDefault();
    if (!dragEl) return;
    var target = e.target.closest('.field-row');
    if (!target || target === dragEl) return;
    var rect = target.getBoundingClientRect();
    var before = (e.clientY - rect.top) < rect.height / 2;
    container.insertBefore(dragEl, before ? target : target.nextSibling);
  }});
  container.addEventListener('dragend', function() {{ dragEl = null; updatePreview(); }});

  // ── Shared field collection — used by both the live preview and submit ──
  function readFields() {{
    var fields = [];
    container.querySelectorAll('.field-row').forEach(function(row) {{
      var label = row.querySelector('.field-label').value.trim();
      var name  = row.querySelector('.field-name').value.trim();
      var type  = row.querySelector('.field-type').value;
      var isVisualOnly = VISUAL_ONLY_TYPES.indexOf(type) !== -1;
      // Skip fully-empty rows — but a separator/note with no title text is
      // a normal, common case (not "nobody filled this in yet"), so only
      // apply that guard to types that actually submit data.
      if (!isVisualOnly && !label && !name) return;
      var required = row.querySelector('.field-required').checked;
      var options = [];
      if (FIELD_TYPES_WITH_OPTIONS.indexOf(type) !== -1) {{
        row.querySelector('.field-options').value.split('\n').forEach(function(line) {{
          line = line.trim();
          if (!line) return;
          var parts = line.split('|');
          var value = parts.length > 1 ? parts[0].trim() : toSlugName(parts[0]);
          var opLabel = parts.length > 1 ? parts[1].trim() : parts[0].trim();
          options.push([value, opLabel]);
        }});
      }}
      fields.push({{ label: label, name: name || toSlugName(label), type: type, required: required, options: options }});
    }});
    return fields;
  }}

  // ── Live preview ─────────────────────────────────────────────────────
  // Button color isn't a form setting — the real submit button is styled
  // from the active theme's CSS at render time, so the mockup here just
  // uses the admin panel's own --primary as a stand-in.
  var preview = document.getElementById('form-preview');
  var buttonLabelInput = document.getElementById('button-label');
  var formNameInput = document.getElementById('form-name');
  var successMessageInput = document.getElementById('success-message');
  var honeypotInput = document.getElementById('include-honeypot');
  var emailProviderInput = document.getElementById('email-provider');
  var notifyEmailInput = document.getElementById('notify-email');
  var confirmSubmitterInput = document.getElementById('confirm-submitter');
  var confirmFieldsBox = document.getElementById('confirm-fields');
  var confirmSubjectInput = document.getElementById('confirm-subject');
  var confirmBodyInput = document.getElementById('confirm-body');
  var noMailInput = document.getElementById('no-mail');
  var mailFieldsWrap = document.getElementById('mail-fields-wrap');
  var saveBtns = [document.getElementById('save-form-btn'), document.getElementById('save-mail-btn')];

  function setReq(id, ok, touched) {{
    var li = document.getElementById(id);
    if (!li) return;
    var dot = li.querySelector('.pw-dot');
    if (!touched) {{
      li.style.color = ''; if (dot) dot.textContent = '·';
    }} else if (ok) {{
      li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
    }} else {{
      li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
    }}
  }}
  function updateRequirements() {{
    var nameLen = formNameInput.value.trim().length;
    setReq('form-req-name', nameLen >= 5 && nameLen <= 255, nameLen > 0);
    var fields = readFields();
    var fieldsOk = fields.length > 0 && fields.every(function(f) {{
      if (VISUAL_ONLY_TYPES.indexOf(f.type) !== -1) return true;
      return f.label.length >= 1 && f.label.length <= 255 && f.name.length >= 1 && f.name.length <= 100;
    }});
    setReq('form-req-fields', fieldsOk, fields.length > 0);
  }}

  confirmSubmitterInput.addEventListener('change', function() {{
    confirmFieldsBox.style.display = confirmSubmitterInput.checked ? 'block' : 'none';
    checkDirty();
  }});

  noMailInput.addEventListener('change', function() {{
    mailFieldsWrap.style.display = noMailInput.checked ? 'none' : 'block';
    checkDirty();
  }});

  function esc(s) {{
    var div = document.createElement('div');
    div.textContent = s == null ? '' : s;
    return div.innerHTML;
  }}

  function renderPreviewField(f) {{
    if (f.type === 'separator') {{
      var title = f.label ? '<p style="margin:0 0 6px;font-size:11px;font-weight:700;text-transform:uppercase;letter-spacing:.04em;color:var(--muted)">' + esc(f.label) + '</p>' : '';
      return '<div style="margin:18px 0">' + title + '<hr style="border:none;border-top:1px solid var(--border);margin:0"></div>';
    }}
    if (f.type === 'note') {{
      return '<div style="margin-bottom:14px;padding:.7rem .9rem;background:var(--tint);border-left:3px solid var(--primary);border-radius:3px;font-size:13px;color:var(--text)">' + esc(f.label || 'Note text goes here') + '</div>';
    }}
    var req = f.required ? ' <span style="color:var(--danger)">*</span>' : '';
    var html = '<div style="margin-bottom:14px">';
    if (f.type !== 'checkbox') {{
      html += '<label style="display:block;font-weight:600;margin-bottom:4px;font-size:13px">' + esc(f.label) + req + '</label>';
    }}
    switch (f.type) {{
      case 'textarea':
        html += '<textarea disabled rows="3" style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;box-sizing:border-box;background:var(--field-bg);color:var(--field-text)"></textarea>';
        break;
      case 'select':
        html += '<select disabled style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;background:var(--field-bg);color:var(--field-text)">' +
          (f.options || []).map(function(o) {{ return '<option>' + esc(o[1]) + '</option>'; }}).join('') +
          '</select>';
        break;
      case 'radio':
        html += (f.options || []).map(function(o) {{
          return '<label style="display:flex;align-items:center;gap:6px;font-size:13px;margin-bottom:4px"><input type="radio" disabled>' + esc(o[1]) + '</label>';
        }}).join('');
        break;
      case 'checkbox':
        html += '<label style="display:flex;align-items:center;gap:6px;font-size:13px;cursor:default"><input type="checkbox" disabled>' + esc(f.label) + req + '</label>';
        break;
      case 'toggle':
        var offLabel = (f.options && f.options[0]) ? f.options[0][1] : 'Off';
        var onLabel  = (f.options && f.options[1]) ? f.options[1][1] : 'On';
        html += '<label class="switch-toggle" style="cursor:default;gap:8px"><span style="font-size:12px;color:var(--muted)">' + esc(offLabel) + '</span><input type="checkbox" disabled><span class="switch-slider"></span><span style="font-size:12px;color:var(--muted)">' + esc(onLabel) + '</span></label>';
        break;
      default:
        var inputType = f.type === 'phone' ? 'tel' : (['text','email','number','date'].indexOf(f.type) !== -1 ? f.type : 'text');
        html += '<input type="' + inputType + '" disabled style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;box-sizing:border-box;background:var(--field-bg);color:var(--field-text)">';
    }}
    html += '</div>';
    return html;
  }}

  function updatePreview() {{
    var fields = readFields();
    var buttonLabel = (buttonLabelInput.value.trim() || 'Submit');
    var html = fields.map(renderPreviewField).join('');
    html += '<button type="button" disabled style="background:var(--primary);color:#fff;border:none;padding:10px 22px;border-radius:6px;font-weight:600;font-size:13px;cursor:default;opacity:.9">' + esc(buttonLabel) + '</button>';
    preview.innerHTML = html;
    checkDirty();
  }}

  // Save stays disabled until something actually changes from what was
  // loaded — same pattern the theme customizer's per-card Save buttons
  // use. readFields() already captures every field/add/remove/reorder
  // change, so the snapshot only needs to add the form-level settings
  // that don't affect the live preview (and so wouldn't otherwise trigger
  // a re-check).
  function snapshot() {{
    return JSON.stringify({{
      name: formNameInput.value,
      fields: readFields(),
      success_message: successMessageInput.value,
      button_label: buttonLabelInput.value,
      include_honeypot: honeypotInput.checked,
      email_provider_id: emailProviderInput.value,
      notify_email: notifyEmailInput.value,
      confirm_submitter: confirmSubmitterInput.checked,
      confirm_subject: confirmSubjectInput.value,
      confirm_body: confirmBodyInput.value,
      no_mail: noMailInput.checked
    }});
  }}
  var initialSnapshot = null;
  function isComplete() {{
    var nameLen = formNameInput.value.trim().length;
    if (nameLen < 5 || nameLen > 255) return false;
    var fields = readFields();
    if (fields.length === 0) return false;
    return fields.every(function(f) {{
      if (VISUAL_ONLY_TYPES.indexOf(f.type) !== -1) return true;
      return f.label.length >= 1 && f.label.length <= 255 && f.name.length >= 1 && f.name.length <= 100;
    }});
  }}
  function checkDirty() {{
    updateRequirements();
    if (initialSnapshot === null) return;
    var dirty = (snapshot() !== initialSnapshot);
    saveBtns.forEach(function(b) {{ b.disabled = !(dirty && isComplete()); }});
  }}

  buttonLabelInput.addEventListener('input', updatePreview);
  formNameInput.addEventListener('input', checkDirty);
  successMessageInput.addEventListener('input', checkDirty);
  honeypotInput.addEventListener('change', checkDirty);
  emailProviderInput.addEventListener('change', checkDirty);
  notifyEmailInput.addEventListener('input', checkDirty);
  confirmSubjectInput.addEventListener('input', checkDirty);
  confirmBodyInput.addEventListener('input', checkDirty);
  updatePreview();
  initialSnapshot = snapshot();
  saveBtns.forEach(function(b) {{ b.disabled = true; }});

  form.addEventListener('submit', function(e) {{
    var fields = readFields();
    if (fields.length === 0) {{
      e.preventDefault();
      alert('Add at least one field before saving.');
      return;
    }}
    document.getElementById('fields-json').value = JSON.stringify(fields);
  }});

  // No nested <form> possible (the whole editor is already one big
  // <form>), so delete goes through fetch(), same pattern as the post
  // editor's delete button.
  window.deleteFormConfirm = function(id) {{
    if (!confirm('Delete this form? This does not delete any submissions already collected under it.')) return;
    fetch('/admin/form-designer/' + id + '/delete', {{ method: 'POST' }}).then(function(r) {{
      window.location.href = r.url || '/admin/form-designer';
    }});
  }};
}})();
</script>"#,
        action = action,
        name = html_escape(&data.name),
        rows_html = rows_html,
        analytics_link = analytics_link,
        delete_btn = delete_btn,
        success_message = html_escape(&data.success_message),
        button_label = html_escape(&data.button_label),
        provider_options_html = provider_options_html,
        site_id = crate::html_escape(&data.site_id),
        notify_email = html_escape(&data.notify_email),
        honeypot_checked = if data.include_honeypot { " checked" } else { "" },
        confirm_submitter_checked = if data.confirm_submitter { " checked" } else { "" },
        confirm_fields_display = if data.confirm_submitter { "block" } else { "none" },
        no_mail_checked = if data.no_mail { " checked" } else { "" },
        mail_fields_display = if data.no_mail { "none" } else { "block" },
        confirm_subject = html_escape(&data.confirm_subject),
        confirm_body = html_escape(&data.confirm_body),
        save_label = if is_edit { "Save Changes" } else { "Create Form" },
        type_opts_js = format!("[{}]", FIELD_TYPES.iter().map(|(v, l)| format!("['{v}','{l}']")).collect::<Vec<_>>().join(",")),
    );

    admin_page(&title, "/admin/form-designer", flash, &content, ctx)
}
