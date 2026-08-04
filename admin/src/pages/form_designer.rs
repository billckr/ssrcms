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

/// Data needed to render the create/edit form. `id` is `None` for create.
pub struct FormEditData {
    pub id: Option<String>,
    pub name: String,
    pub fields: Vec<FieldRow>,
    pub success_message: String,
    pub button_label: String,
    pub include_honeypot: bool,
}

impl Default for FormEditData {
    fn default() -> Self {
        FormEditData {
            id: None,
            name: String::new(),
            fields: vec![FieldRow {
                label: "Your name".to_string(),
                name: "name".to_string(),
                field_type: "text".to_string(),
                required: true,
                options_text: String::new(),
            }],
            success_message: "Thank you for your submission!".to_string(),
            button_label: "Submit".to_string(),
            include_honeypot: true,
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
];

pub fn render_list(rows: &[FormRow], ctx: &PageContext, flash: Option<&str>) -> String {
    let body = if rows.is_empty() {
        r#"<tr><td colspan="4" style="text-align:center;color:var(--muted)">No forms yet. Create one to get started.</td></tr>"#.to_string()
    } else {
        rows.iter().map(|f| {
            format!(
                r#"<tr>
  <td><a href="/admin/form-designer/{id}">{name}</a></td>
  <td><code>{slug}</code></td>
  <td>{count}</td>
  <td class="actions">
    <a href="/admin/form-designer/{id}" class="icon-btn" title="Edit">
      <img src="/admin/static/icons/edit.svg" alt="Edit">
    </a>
    <form method="POST" action="/admin/form-designer/{id}/delete" style="display:inline"
          onsubmit="return confirm('Delete the form \'{name_js}\'? This does not delete any submissions already collected under it.')">
      <button class="icon-btn icon-danger" title="Delete" type="submit">
        <img src="/admin/static/icons/delete.svg" alt="Delete">
      </button>
    </form>
  </td>
</tr>"#,
                id = html_escape(&f.id),
                name = html_escape(&f.name),
                name_js = f.name.replace('\'', "\\'"),
                slug = html_escape(&f.slug),
                count = f.field_count,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<p style="margin-bottom:1rem"><a href="/admin/form-designer/new" class="btn btn-primary">New Form</a></p>
<table class="data-table">
  <thead><tr><th>Name</th><th>Slug</th><th>Fields</th><th>Actions</th></tr></thead>
  <tbody>{body}</tbody>
</table>"#,
        body = body,
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

    format!(
        r#"<div class="field-row" data-index="{index}" style="border:1px solid var(--border);border-radius:var(--radius);padding:.85rem 1rem;margin-bottom:.6rem;background:var(--surface)">
  <div style="display:flex;align-items:flex-start;gap:.6rem">
    <span class="drag-handle" title="Drag to reorder" draggable="true" style="margin-top:1.6rem">
      <img src="/admin/static/icons/move.svg" alt="">
    </span>
    <div style="flex:1;display:grid;grid-template-columns:1fr 1fr 1fr auto;gap:.6rem;align-items:end">
      <div class="form-group" style="margin:0">
        <label>Field label</label>
        <input type="text" class="field-label" value="{label}" placeholder="e.g. Your name">
      </div>
      <div class="form-group" style="margin:0">
        <label>Field name <span class="field-hint">(used in submissions)</span></label>
        <input type="text" class="field-name" value="{name}" placeholder="e.g. name">
      </div>
      <div class="form-group" style="margin:0">
        <label>Type</label>
        <select class="field-type">{type_opts}</select>
      </div>
      <button type="button" class="icon-btn icon-danger field-remove" title="Remove field" style="margin-bottom:.45rem">
        <img src="/admin/static/icons/delete.svg" alt="Remove">
      </button>
    </div>
  </div>
  <div style="margin-left:2.2rem;margin-top:.5rem;display:flex;align-items:center;gap:1.2rem">
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
        name = html_escape(&f.name),
        type_opts = type_opts,
        required = if f.required { " checked" } else { "" },
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
    let title = if is_edit { "Edit Form" } else { "New Form" };

    let rows_html: String = data.fields.iter().enumerate()
        .map(|(i, f)| field_row_html(f, i))
        .collect::<Vec<_>>()
        .join("\n");

    let delete_section = if let Some(id) = &data.id {
        format!(
            r#"<div class="card-boxed" style="margin-top:1rem">
  <div class="card-boxed-body">
    <form method="POST" action="/admin/form-designer/{id}/delete"
          onsubmit="return confirm('Delete this form? This does not delete any submissions already collected under it.')">
      <button type="submit" class="btn" style="color:var(--danger);border-color:var(--danger)">Delete Form</button>
    </form>
  </div>
</div>"#,
            id = html_escape(id)
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"<style>.btn-sm {{ font-size: 12px; padding: .2rem .6rem; }} .field-hint {{ font-size: 11px; color: var(--muted); font-weight: 400; }}</style>
<form method="POST" action="{action}" id="form-designer-form">
  <div class="two-col">
    <div>
      <div class="card-boxed">
        <h2 class="card-boxed-header">Fields</h2>
        <div class="card-boxed-body">
          <div class="form-group">
            <label for="form-name">Form name</label>
            <input type="text" id="form-name" name="name" required maxlength="120" value="{name}" placeholder="e.g. Contact Us">
          </div>
          <div id="field-rows">{rows_html}</div>
          <button type="button" id="add-field-btn" class="btn btn-sm" style="margin-top:.4rem">+ Add Field</button>
          <input type="hidden" name="fields_json" id="fields-json">
        </div>
      </div>
      <div class="card-boxed" style="margin-top:1rem">
        <h2 class="card-boxed-header">Settings</h2>
        <div class="card-boxed-body">
          <div class="form-group">
            <label for="success-message">Success message</label>
            <input type="text" id="success-message" name="success_message" maxlength="200" value="{success_message}">
          </div>
          <div class="form-group">
            <label for="button-label">Submit button label</label>
            <input type="text" id="button-label" name="button_label" maxlength="40" value="{button_label}">
          </div>
          <label class="switch-toggle" style="margin-top:.4rem">
            <input type="checkbox" name="include_honeypot" value="true"{honeypot_checked}>
            <span class="switch-slider"></span>
            <span>Include spam honeypot field</span>
          </label>
        </div>
      </div>
      {delete_section}
      <div style="margin-top:1rem">
        <button type="submit" class="btn btn-primary">{save_label}</button>
        <a href="/admin/form-designer" class="btn">Cancel</a>
      </div>
    </div>
    <div>
      <div class="card-boxed" style="position:sticky;top:1rem">
        <h2 class="card-boxed-header">Preview</h2>
        <div class="card-boxed-body">
          <p class="field-hint" style="margin-bottom:.85rem">A live mockup — this is what visitors will see. It isn't wired to submit anything from here.</p>
          <div id="form-preview"></div>
        </div>
      </div>
    </div>
  </div>
</form>
<script>
(function() {{
  var FIELD_TYPES_WITH_OPTIONS = ['select', 'radio', 'toggle'];
  var OPTIONS_META = {{
    toggle: {{ label: 'Off / On labels', hint: 'exactly two lines — off state, then on state', placeholder: 'e.g.\noff|Off\non|On' }},
    select: {{ label: 'Options', hint: 'one per line — "value|Label", or just "Label"', placeholder: 'e.g.\nyes|Yes\nno|No' }},
    radio:  {{ label: 'Options', hint: 'one per line — "value|Label", or just "Label"', placeholder: 'e.g.\nyes|Yes\nno|No' }}
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
    row.style.cssText = 'border:1px solid var(--border);border-radius:var(--radius);padding:.85rem 1rem;margin-bottom:.6rem;background:var(--surface)';
    var typeOpts = {type_opts_js}.map(function(t) {{
      return '<option value="' + t[0] + '">' + t[1] + '</option>';
    }}).join('');
    row.innerHTML =
      '<div style="display:flex;align-items:flex-start;gap:.6rem">' +
        '<span class="drag-handle" title="Drag to reorder" draggable="true" style="margin-top:1.6rem"><img src="/admin/static/icons/move.svg" alt=""></span>' +
        '<div style="flex:1;display:grid;grid-template-columns:1fr 1fr 1fr auto;gap:.6rem;align-items:end">' +
          '<div class="form-group" style="margin:0"><label>Field label</label><input type="text" class="field-label" placeholder="e.g. Your name"></div>' +
          '<div class="form-group" style="margin:0"><label>Field name <span class="field-hint">(used in submissions)</span></label><input type="text" class="field-name" placeholder="e.g. name"></div>' +
          '<div class="form-group" style="margin:0"><label>Type</label><select class="field-type">' + typeOpts + '</select></div>' +
          '<button type="button" class="icon-btn icon-danger field-remove" title="Remove field" style="margin-bottom:.45rem"><img src="/admin/static/icons/delete.svg" alt="Remove"></button>' +
        '</div>' +
      '</div>' +
      '<div style="margin-left:2.2rem;margin-top:.5rem;display:flex;align-items:center;gap:1.2rem">' +
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
      if (!label && !name) return; // skip fully-empty rows
      var type  = row.querySelector('.field-type').value;
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

  function esc(s) {{
    var div = document.createElement('div');
    div.textContent = s == null ? '' : s;
    return div.innerHTML;
  }}

  function renderPreviewField(f) {{
    var req = f.required ? ' <span style="color:var(--danger)">*</span>' : '';
    var html = '<div style="margin-bottom:14px">';
    if (f.type !== 'checkbox') {{
      html += '<label style="display:block;font-weight:600;margin-bottom:4px;font-size:13px">' + esc(f.label) + req + '</label>';
    }}
    switch (f.type) {{
      case 'textarea':
        html += '<textarea disabled rows="3" style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;box-sizing:border-box;background:#fff"></textarea>';
        break;
      case 'select':
        html += '<select disabled style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;background:#fff">' +
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
        html += '<input type="' + inputType + '" disabled style="width:100%;padding:8px 10px;font-size:13px;border:1px solid var(--border);border-radius:6px;box-sizing:border-box;background:#fff">';
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
  }}

  buttonLabelInput.addEventListener('input', updatePreview);
  updatePreview();

  form.addEventListener('submit', function(e) {{
    var fields = readFields();
    if (fields.length === 0) {{
      e.preventDefault();
      alert('Add at least one field before saving.');
      return;
    }}
    document.getElementById('fields-json').value = JSON.stringify(fields);
  }});
}})();
</script>"#,
        action = action,
        name = html_escape(&data.name),
        rows_html = rows_html,
        delete_section = delete_section,
        success_message = html_escape(&data.success_message),
        button_label = html_escape(&data.button_label),
        honeypot_checked = if data.include_honeypot { " checked" } else { "" },
        save_label = if is_edit { "Save Changes" } else { "Create Form" },
        type_opts_js = format!("[{}]", FIELD_TYPES.iter().map(|(v, l)| format!("['{v}','{l}']")).collect::<Vec<_>>().join(",")),
    );

    admin_page(title, "/admin/form-designer", flash, &content, ctx)
}
