//! Poll Designer: build single-question vote polls that can later be
//! inserted into posts/pages. This module only renders the list/create/edit
//! admin pages — storage lives in `models::poll_def`/`models::poll_vote`.

use crate::{html_escape, PageContext};

/// One row for the list view.
pub struct PollRow {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub option_count: usize,
    pub total_votes: i64,
    pub updated_at: String,
}

/// One option, as rendered into the editor.
pub struct PollOptionRow {
    pub key: String,
    pub label: String,
}

/// Data needed to render the create/edit poll form. `id` is `None` for create.
pub struct PollEditData {
    pub id: Option<String>,
    pub name: String,
    pub question: String,
    pub options: Vec<PollOptionRow>,
    pub success_message: String,
    pub button_label: String,
    /// "cookie_only" | "cookie_and_ip".
    pub vote_protection: String,
}

impl Default for PollEditData {
    fn default() -> Self {
        PollEditData {
            id: None,
            name: String::new(),
            question: String::new(),
            options: vec![
                PollOptionRow { key: "option_1".to_string(), label: String::new() },
                PollOptionRow { key: "option_2".to_string(), label: String::new() },
            ],
            success_message: "Thanks for voting!".to_string(),
            button_label: "Vote".to_string(),
            vote_protection: "cookie_and_ip".to_string(),
        }
    }
}

/// Table + pagination-free list — a site's poll count is small enough that,
/// same as Form Designer, no pagination is needed here.
pub fn polls_list_fragment(rows: &[PollRow], search: &str) -> String {
    let needle = search.trim().to_lowercase();
    let filtered: Vec<&PollRow> = if needle.is_empty() {
        rows.iter().collect()
    } else {
        rows.iter().filter(|r| r.name.to_lowercase().contains(&needle) || r.slug.to_lowercase().contains(&needle)).collect()
    };

    let body = if filtered.is_empty() {
        r#"<tr><td colspan="5" style="text-align:center;color:var(--muted)">No polls yet. Create one to get started. <a href="/admin/designer/polls/new">Create</a></td></tr>"#.to_string()
    } else {
        filtered.iter().map(|p| {
            format!(
                r#"<tr>
  <td>{name}</td>
  <td><code>{slug}</code></td>
  <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{count}</span></td>
  <td>{votes}</td>
  <td class="actions">
    <div class="icon-pill-actionbuttons">
      <a href="/admin/designer/polls/{id}/results" class="icon-btn" title="Results" aria-label="Results">
        <img src="/admin/static/icons/bar-chart-2.svg" alt="">
      </a>
      <a href="/admin/designer/polls/{id}" class="icon-btn" title="Edit" aria-label="Edit">
        <img src="/admin/static/icons/edit.svg" alt="">
      </a>
    </div>
  </td>
</tr>"#,
                id = html_escape(&p.id),
                name = html_escape(&p.name),
                slug = html_escape(&p.slug),
                count = p.option_count,
                votes = p.total_votes,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    format!(
        r#"<table class="data-table">
  <thead><tr><th>Name</th><th>Slug</th><th>Options</th><th>Votes</th><th>Actions</th></tr></thead>
  <tbody>{body}</tbody>
</table>"#
    )
}

fn option_row_html(o: &PollOptionRow, index: usize) -> String {
    format!(
        r#"<div class="poll-option-row" data-index="{index}" style="display:flex;align-items:end;gap:.6rem;border:1px solid var(--border);border-radius:var(--radius);padding:.6rem .75rem;margin-bottom:.5rem;background:var(--tint)">
  <div class="form-group" style="margin:0;flex:1">
    <label>Option label</label>
    <input type="text" class="poll-option-label" maxlength="255" value="{label}" placeholder="{placeholder}">
  </div>
  <button type="button" class="icon-btn poll-option-up" title="Move up" aria-label="Move up"><img src="/admin/static/icons/chevron-up.svg" alt=""></button>
  <button type="button" class="icon-btn poll-option-down" title="Move down" aria-label="Move down"><img src="/admin/static/icons/chevron-down.svg" alt=""></button>
  <button type="button" class="icon-btn icon-danger poll-option-remove" title="Remove option" aria-label="Remove option"><img src="/admin/static/icons/trash.svg" alt=""></button>
</div>"#,
        index = index,
        label = html_escape(&o.label),
        placeholder = format!("e.g. Option {}", index + 1),
    )
}

pub fn render_editor(data: &PollEditData, ctx: &PageContext, flash: Option<&str>) -> String {
    let is_edit = data.id.is_some();
    let action = match &data.id {
        Some(id) => format!("/admin/designer/polls/{id}"),
        None => "/admin/designer/polls".to_string(),
    };
    let title = if is_edit { format!("Editing poll - {}", html_escape(&data.name)) } else { "New Poll".to_string() };

    let rows_html: String = data.options.iter().enumerate()
        .map(|(i, o)| option_row_html(o, i))
        .collect::<Vec<_>>()
        .join("\n");

    let delete_btn = if let Some(id) = &data.id {
        format!(
            r#"<button type="button" class="icon-btn icon-danger" title="Delete Poll" aria-label="Delete Poll"
        onclick="event.preventDefault();event.stopPropagation();deletePollConfirm('{id}')">
    <img src="/admin/static/icons/trash.svg" alt="">
  </button>"#,
            id = html_escape(id)
        )
    } else {
        String::new()
    };

    let results_link = if let Some(id) = &data.id {
        format!(
            r#"<a href="/admin/designer/polls/{id}/results" class="icon-btn" title="Results" aria-label="Results">
    <img src="/admin/static/icons/bar-chart-2.svg" alt="">
  </a>"#,
            id = html_escape(id),
        )
    } else {
        String::new()
    };

    let cookie_only_checked = if data.vote_protection == "cookie_only" { " checked" } else { "" };
    let cookie_and_ip_checked = if data.vote_protection != "cookie_only" { " checked" } else { "" };

    let content = format!(
        r#"<form method="POST" action="{action}" id="poll-designer-form">
  <div class="two-col">
    <div>
      <div class="card-boxed">
        <h2 class="card-boxed-header">Question &amp; Options</h2>
        <div class="card-boxed-body">
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="poll-question">Question</label>
              <input type="text" id="poll-question" name="question" required minlength="5" maxlength="255" value="{question}" placeholder="e.g. What's your favorite color?">
            </div>
          </div>
          <div class="card-boxed-section">
            <div id="option-rows">{rows_html}</div>
            <input type="hidden" name="options_json" id="options-json">
            <div class="icon-pill">
              <button type="button" id="add-option-btn" class="icon-btn" title="Add Option" aria-label="Add Option">
                <img src="/admin/static/icons/file-plus.svg" alt="">
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div>
      <div class="card-boxed" style="position:sticky;top:1rem">
        <h2 class="card-boxed-header">Poll Settings</h2>
        <div class="card-boxed-body">
          <div class="card-boxed-section">
            <div class="form-group">
              <label for="poll-name">Poll name</label>
              <input type="text" id="poll-name" name="name" required minlength="5" maxlength="255" value="{name}" placeholder="e.g. Favorite Color Poll">
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
              <label for="button-label">Vote button label</label>
              <input type="text" id="button-label" name="button_label" maxlength="40" value="{button_label}">
            </div>
          </div>
          <div class="card-boxed-section">
            <label>Vote protection</label>
            <div style="display:flex;flex-direction:column;gap:.4rem;margin-top:.4rem">
              <label class="radio-label">
                <input type="radio" name="vote_protection" value="cookie_and_ip"{cookie_and_ip_checked}> Cookie + IP address (more secure, default)
              </label>
              <label class="radio-label">
                <input type="radio" name="vote_protection" value="cookie_only"{cookie_only_checked}> Cookie only
              </label>
            </div>
            <p class="field-hint">Controls what stops the same visitor from voting twice. Neither option requires a login — clearing cookies (and, for Cookie only, switching networks) can still allow a re-vote.</p>
          </div>
          <div class="card-boxed-section">
            <div class="form-note" style="margin-bottom:0">
              <p><strong>Requirements:</strong></p>
              <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
                <li id="poll-req-name"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>Poll name (5-255 characters)</li>
                <li id="poll-req-question"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>Question (5-255 characters)</li>
                <li id="poll-req-options"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least two options with labels</li>
              </ul>
            </div>
          </div>
          <div class="icon-pill">
            <button type="button" id="save-poll-btn" class="icon-btn" title="{save_label}" aria-label="{save_label}"
                    onclick="event.preventDefault();event.stopPropagation();document.getElementById('poll-designer-form').requestSubmit();">
              <img src="/admin/static/icons/save.svg" alt="">
            </button>
            {results_link}
            {delete_btn}
          </div>
        </div>
      </div>
    </div>
  </div>
</form>
<script>
(function() {{
  var container = document.getElementById('option-rows');
  var addBtn = document.getElementById('add-option-btn');
  var form = document.getElementById('poll-designer-form');

  function toSlugKey(s) {{
    return s.toLowerCase().replace(/[^a-z0-9\s_-]/g, '').trim().replace(/[\s-]+/g, '_') || 'option';
  }}

  function makeRow() {{
    var row = document.createElement('div');
    row.className = 'poll-option-row';
    row.style.cssText = 'display:flex;align-items:end;gap:.6rem;border:1px solid var(--border);border-radius:var(--radius);padding:.6rem .75rem;margin-bottom:.5rem;background:var(--tint)';
    row.innerHTML =
      '<div class="form-group" style="margin:0;flex:1"><label>Option label</label><input type="text" class="poll-option-label" maxlength="255" placeholder="e.g. Blue"></div>' +
      '<button type="button" class="icon-btn poll-option-up" title="Move up" aria-label="Move up"><img src="/admin/static/icons/chevron-up.svg" alt=""></button>' +
      '<button type="button" class="icon-btn poll-option-down" title="Move down" aria-label="Move down"><img src="/admin/static/icons/chevron-down.svg" alt=""></button>' +
      '<button type="button" class="icon-btn icon-danger poll-option-remove" title="Remove option" aria-label="Remove option"><img src="/admin/static/icons/trash.svg" alt=""></button>';
    return row;
  }}

  function wireRow(row) {{
    row.querySelector('.poll-option-label').addEventListener('input', checkDirty);
    row.querySelector('.poll-option-up').addEventListener('click', function() {{
      var prev = row.previousElementSibling;
      if (prev) container.insertBefore(row, prev);
      checkDirty();
    }});
    row.querySelector('.poll-option-down').addEventListener('click', function() {{
      var next = row.nextElementSibling;
      if (next) container.insertBefore(next, row);
      checkDirty();
    }});
    row.querySelector('.poll-option-remove').addEventListener('click', function() {{
      if (container.querySelectorAll('.poll-option-row').length <= 2) {{
        alert('A poll needs at least two options.');
        return;
      }}
      row.remove();
      checkDirty();
    }});
  }}

  container.querySelectorAll('.poll-option-row').forEach(wireRow);

  addBtn.addEventListener('click', function() {{
    var row = makeRow();
    container.appendChild(row);
    wireRow(row);
    row.querySelector('.poll-option-label').focus();
    checkDirty();
  }});

  function readOptions() {{
    var options = [];
    container.querySelectorAll('.poll-option-row').forEach(function(row) {{
      var label = row.querySelector('.poll-option-label').value.trim();
      if (!label) return;
      options.push({{ key: toSlugKey(label), label: label }});
    }});
    return options;
  }}

  var nameInput = document.getElementById('poll-name');
  var questionInput = document.getElementById('poll-question');
  var successMessageInput = document.getElementById('success-message');
  var buttonLabelInput = document.getElementById('button-label');
  var saveBtn = document.getElementById('save-poll-btn');

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
    var nameLen = nameInput.value.trim().length;
    var questionLen = questionInput.value.trim().length;
    var optionCount = readOptions().length;
    setReq('poll-req-name', nameLen >= 5 && nameLen <= 255, nameLen > 0);
    setReq('poll-req-question', questionLen >= 5 && questionLen <= 255, questionLen > 0);
    setReq('poll-req-options', optionCount >= 2, optionCount > 0);
  }}

  function snapshot() {{
    var protection = document.querySelector('input[name="vote_protection"]:checked');
    return JSON.stringify({{
      name: nameInput.value,
      question: questionInput.value,
      options: readOptions(),
      success_message: successMessageInput.value,
      button_label: buttonLabelInput.value,
      vote_protection: protection ? protection.value : ''
    }});
  }}
  var initialSnapshot = null;
  function isComplete() {{
    var nameLen = nameInput.value.trim().length;
    var questionLen = questionInput.value.trim().length;
    return nameLen >= 5 && nameLen <= 255 && questionLen >= 5 && questionLen <= 255 && readOptions().length >= 2;
  }}
  function checkDirty() {{
    updateRequirements();
    if (initialSnapshot === null) return;
    var dirty = snapshot() !== initialSnapshot;
    saveBtn.disabled = !(dirty && isComplete());
  }}

  nameInput.addEventListener('input', checkDirty);
  questionInput.addEventListener('input', checkDirty);
  successMessageInput.addEventListener('input', checkDirty);
  buttonLabelInput.addEventListener('input', checkDirty);
  document.querySelectorAll('input[name="vote_protection"]').forEach(function(r) {{
    r.addEventListener('change', checkDirty);
  }});
  initialSnapshot = snapshot();
  saveBtn.disabled = true;

  form.addEventListener('submit', function(e) {{
    var options = readOptions();
    if (options.length < 2) {{
      e.preventDefault();
      alert('Add at least two options before saving.');
      return;
    }}
    document.getElementById('options-json').value = JSON.stringify(options);
  }});

  window.deletePollConfirm = function(id) {{
    if (!confirm('Delete this poll? This also deletes all of its votes.')) return;
    fetch('/admin/designer/polls/' + id + '/delete', {{ method: 'POST' }}).then(function(r) {{
      window.location.href = r.url || '/admin/designer?tab=polls';
    }});
  }};
}})();
</script>"#,
        action = action,
        name = html_escape(&data.name),
        question = html_escape(&data.question),
        rows_html = rows_html,
        results_link = results_link,
        delete_btn = delete_btn,
        success_message = html_escape(&data.success_message),
        button_label = html_escape(&data.button_label),
        cookie_only_checked = cookie_only_checked,
        cookie_and_ip_checked = cookie_and_ip_checked,
        save_label = if is_edit { "Save Changes" } else { "Create Poll" },
    );

    crate::admin_page(&title, "/admin/designer", flash, &content, ctx)
}
