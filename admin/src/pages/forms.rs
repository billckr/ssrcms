//! Admin forms pages — form list and submission detail views.

use crate::{html_escape, admin_page, PageContext};

pub struct FormSummaryRow {
    pub form_name: String,
    pub submission_count: i64,
    pub last_submitted_at: String,
    pub unread_count: i64,
    pub blocked: bool,
}

pub struct SubmissionRow {
    pub id: String,
    pub data: serde_json::Value,
    pub ip_address: Option<String>,
    pub read_at: Option<String>,
    pub submitted_at: String,
}

// ── Forms list ────────────────────────────────────────────────────────────────

pub fn render_forms_list(forms: &[FormSummaryRow], flash: Option<&str>, ctx: &PageContext) -> String {
    let rows = if forms.is_empty() {
        r#"<tr><td colspan="5" class="empty-state">No form submissions yet.</td></tr>"#.to_string()
    } else {
        forms.iter().map(|f| {
            let blocked_badge = if f.blocked {
                r#" <span class="badge badge-danger" title="Not accepting submissions">Blocked</span>"#
            } else { "" };
            let block_btn = if f.blocked {
                format!(
                    r#"<form method="POST" action="/admin/forms/{}/toggle-block" style="display:inline">
  <button class="btn btn-sm btn-secondary" type="submit">Unblock</button>
</form>"#,
                    html_escape(&f.form_name)
                )
            } else {
                format!(
                    r#"<form method="POST" action="/admin/forms/{}/toggle-block" style="display:inline"
      onsubmit="return confirm('Block this form? New submissions will be silently discarded.')">
  <button class="btn btn-sm btn-danger" type="submit">Block</button>
</form>"#,
                    html_escape(&f.form_name)
                )
            };
            let row_class = if f.blocked { " class=\"muted\"" } else { "" };
            format!(
                r#"<tr{row_class}>
  <td><a href="/admin/forms/{name}">{name}</a>{blocked_badge}</td>
  <td>{count}</td>
  <td>{last}</td>
  <td>
    <a href="/admin/forms/{name}" class="btn btn-sm btn-secondary">View</a>
    <a href="/admin/forms/{name}/export" class="btn btn-sm btn-secondary">CSV</a>
    {block_btn}
  </td>
</tr>"#,
                row_class = row_class,
                name = html_escape(&f.form_name),
                count = f.submission_count,
                last = html_escape(&f.last_submitted_at),
                blocked_badge = blocked_badge,
                block_btn = block_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div class="table-wrap">
<table class="data-table">
  <thead>
    <tr>
      <th>Form Name</th>
      <th>Submissions</th>
      <th>Last Submitted</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>
    {rows}
  </tbody>
</table>
</div>"#
    );

    admin_page("Forms", "/admin/forms", flash, &content, ctx)
}

// ── Submission detail ─────────────────────────────────────────────────────────

pub fn render_form_detail(
    form_name: &str,
    submissions: &[SubmissionRow],
    columns: &[String],
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    let col_headers = columns.iter().map(|c| {
        format!("<th>{}</th>", html_escape(c))
    }).collect::<Vec<_>>().join("");

    let rows = if submissions.is_empty() {
        let span = columns.len() + 3; // data cols + submitted_at + ip + actions
        format!(r#"<tr><td colspan="{span}" class="empty-state">No submissions yet.</td></tr>"#)
    } else {
        submissions.iter().map(|s| {
            let cells = columns.iter().map(|col| {
                let val = s.data.get(col)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("<td>{}</td>", html_escape(val))
            }).collect::<Vec<_>>().join("");

            format!(
                r#"<tr>
  {cells}
  <td>{submitted}</td>
  <td>{ip}</td>
  <td>
    <form method="POST" action="/admin/forms/{fname}/{id}/delete"
          onsubmit="return confirm('Delete this submission?')">
      <button class="btn btn-sm btn-danger" type="submit">Delete</button>
    </form>
  </td>
</tr>"#,
                cells = cells,
                submitted = html_escape(&s.submitted_at),
                ip = html_escape(s.ip_address.as_deref().unwrap_or("—")),
                fname = html_escape(form_name),
                id = html_escape(&s.id),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div class="page-actions" style="margin-bottom:1rem;display:flex;gap:0.5rem;flex-wrap:wrap;align-items:center;">
  <a href="/admin/forms/{fname}/export" class="btn btn-secondary">Export CSV</a>
  <form method="POST" action="/admin/forms/{fname}/delete-all" style="display:inline"
        onsubmit="return confirm('Delete ALL submissions for this form?')">
    <button class="btn btn-danger" type="submit">Delete All</button>
  </form>
  <a href="/admin/forms" class="btn btn-secondary" style="margin-left:auto">← All Forms</a>
</div>
<div class="table-wrap">
<table class="data-table">
  <thead>
    <tr>
      {col_headers}
      <th>Submitted</th>
      <th>IP</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>
    {rows}
  </tbody>
</table>
</div>"#,
        fname = html_escape(form_name),
        col_headers = col_headers,
        rows = rows,
    );

    let title = format!("Form: {}", form_name);
    admin_page(&title, "/admin/forms", flash, &content, ctx)
}
