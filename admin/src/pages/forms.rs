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

pub fn render_forms_list(
    forms: &[FormSummaryRow],
    all_names: &[String],
    active_filter: &str,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    // ── filter dropdown ───────────────────────────────────────────────────────
    let filter_dropdown = if all_names.is_empty() {
        String::new()
    } else {
        let options = std::iter::once(
            format!(
                r#"<option value=""{}>(All forms)</option>"#,
                if active_filter.is_empty() { " selected" } else { "" }
            )
        ).chain(all_names.iter().map(|n| {
            let sel = if n == active_filter { " selected" } else { "" };
            format!(r#"<option value="{n}"{sel}>{n}</option>"#, n = html_escape(n), sel = sel)
        })).collect::<Vec<_>>().join("\n");

        format!(
            r#"<form method="GET" action="/admin/forms" style="display:inline;margin-left:0.5rem;">
  <select name="filter" class="forms-filter-select" onchange="this.form.submit()" aria-label="Filter by form name">
    {options}
  </select>
</form>"#
        )
    };

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
        r#"<div style="margin-bottom:1rem;display:flex;align-items:center;gap:0.5rem;flex-wrap:wrap;">
  <a href="/admin/pages/new" class="btn btn-primary">New Form</a>
  {filter_dropdown}
</div>
<div class="table-wrap">
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
</div>
<style>
.forms-filter-select {{
  padding: 0.4rem 0.65rem;
  font-size: 0.875rem;
  font-family: inherit;
  border: 1.5px solid var(--color-border, #d1d5db);
  border-radius: 6px;
  background: #fff;
  color: var(--color-text, #1a1a2e);
  cursor: pointer;
  height: 2.15rem;
}}
.forms-filter-select:focus {{
  outline: none;
  border-color: var(--color-primary, #2b6cb0);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-primary, #2b6cb0) 15%, transparent);
}}
</style>"#,
        filter_dropdown = filter_dropdown,
        rows = rows,
    );

    admin_page("Forms", "/admin/forms", flash, &content, ctx)
}

// ── Submission detail ─────────────────────────────────────────────────────────

/// Link a submission's IP to its ARIN RDAP lookup, opening in a new tab.
/// `stopPropagation` keeps the click from also toggling the enclosing
/// `<details>` — without it, clicking the link both opens ARIN and
/// collapses/expands the row underneath it.
fn ip_link_html(ip: Option<&str>) -> String {
    match ip {
        Some(ip) if !ip.is_empty() => format!(
            r#"<a href="https://search.arin.net/rdap/?query={ip_enc}" target="_blank" rel="noopener noreferrer" onclick="event.stopPropagation()">{ip_esc}</a>"#,
            ip_enc = html_escape(ip),
            ip_esc = html_escape(ip),
        ),
        _ => "—".to_string(),
    }
}

fn submission_pagination(form_name: &str, page: i64, total_pages: i64) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let base_path = format!("/admin/forms/{}", html_escape(form_name));
    let prev = if page > 1 {
        format!(r#"<a href="{base_path}?page={}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="{base_path}?page={}" class="page-btn">Next &raquo;</a>"#, page + 1)
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
            nums.push_str(&format!(r#"<a href="{base_path}?page={p}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

pub fn render_form_detail(
    form_name: &str,
    submissions: &[SubmissionRow],
    columns: &[String],
    page: i64,
    total_pages: i64,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    let rows = if submissions.is_empty() {
        r#"<p class="empty-state">No submissions yet.</p>"#.to_string()
    } else {
        submissions.iter().map(|s| {
            let fields = columns.iter().map(|col| {
                let val = s.data.get(col).and_then(|v| v.as_str()).unwrap_or("");
                format!(
                    r#"<div class="submission-field">
  <dt>{col}</dt>
  <dd>{val}</dd>
</div>"#,
                    col = html_escape(col),
                    val = if val.is_empty() { "—".to_string() } else { html_escape(val) },
                )
            }).collect::<Vec<_>>().join("\n");

            format!(
                r#"<details class="card-boxed submission-row">
  <summary class="card-boxed-header">
    <span class="submission-summary-date">{submitted}</span>
    <span class="submission-summary-ip">{ip}</span>
  </summary>
  <div class="card-boxed-body">
    <dl class="submission-fields">
      {fields}
    </dl>
    <form method="POST" action="/admin/forms/{fname}/{id}/delete"
          onsubmit="return confirm('Delete this submission?')" style="margin-top:.75rem">
      <button class="btn btn-sm btn-danger" type="submit">Delete</button>
    </form>
  </div>
</details>"#,
                submitted = html_escape(&s.submitted_at),
                fields = fields,
                ip = ip_link_html(s.ip_address.as_deref()),
                fname = html_escape(form_name),
                id = html_escape(&s.id),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let has_submissions = !submissions.is_empty();
    let search_box = if has_submissions {
        r#"<div class="icon-search-box">
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
  <input type="search" id="submission-search" placeholder="Search responses…" autocomplete="off">
</div>"#
    } else {
        ""
    };

    let content = format!(
        r#"<style>
.submission-list .card-boxed {{ margin-bottom: .6rem; }}
.submission-row summary.card-boxed-header {{ transition: background-color .1s ease; }}
.submission-row summary.card-boxed-header:hover {{ background: #eef1f5; }}
.submission-summary-date {{
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
  color: var(--text);
  font-weight: 600;
  font-size: .85rem;
  padding-right: .9rem;
  margin-right: .9rem;
  border-right: 1px solid var(--border);
}}
.submission-summary-ip {{ flex: 1; color: var(--muted); font-weight: 400; font-size: .85rem; }}
.submission-summary-ip a {{ color: var(--muted); text-decoration: none; }}
.submission-summary-ip a:hover {{ color: var(--primary); text-decoration: underline; }}
.submission-fields {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: .75rem 1.5rem; }}
.submission-field dt {{ font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .03em; color: var(--muted); margin-bottom: .15rem; }}
.submission-field dd {{ font-size: 13px; color: var(--text); word-break: break-word; }}
#submission-no-matches {{ display: none; color: var(--muted); font-size: .9rem; padding: 1rem 0; }}
</style>
<div class="page-actions" style="margin-bottom:1rem;display:flex;gap:0.5rem;flex-wrap:wrap;align-items:center;">
  <a href="/admin/forms/{fname}/export" class="btn btn-secondary">Export CSV</a>
  <form method="POST" action="/admin/forms/{fname}/delete-all" style="display:inline"
        onsubmit="return confirm('Delete ALL submissions for this form?')">
    <button class="btn btn-danger" type="submit">Delete All</button>
  </form>
  <a href="/admin/forms" class="btn btn-secondary" style="margin-left:auto">← All Forms</a>
</div>
{search_box}
<div class="submission-list">
{rows}
</div>
<p id="submission-no-matches">No responses match your search.</p>
{pagination}
<script>
(function() {{
  var input = document.getElementById('submission-search');
  if (!input) return;
  var rows = document.querySelectorAll('.submission-row');
  var noMatches = document.getElementById('submission-no-matches');
  input.addEventListener('input', function() {{
    var q = input.value.trim().toLowerCase();
    var visible = 0;
    rows.forEach(function(row) {{
      var match = !q || row.textContent.toLowerCase().indexOf(q) !== -1;
      row.style.display = match ? '' : 'none';
      if (match) visible++;
    }});
    noMatches.style.display = (q && visible === 0) ? '' : 'none';
  }});
}})();
</script>"#,
        fname = html_escape(form_name),
        search_box = search_box,
        rows = rows,
        pagination = submission_pagination(form_name, page, total_pages),
    );

    let title = format!("Form: {}", form_name);
    admin_page(&title, "/admin/forms", flash, &content, ctx)
}
