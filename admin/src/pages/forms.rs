//! Admin forms pages — form list and submission detail views.

use crate::{html_escape, admin_page, PageContext};

pub struct FormSummaryRow {
    pub form_name: String,
    pub submission_count: i64,
    pub last_submitted_at: String,
    pub unread_count: i64,
    pub blocked: bool,
    /// False when no Form Designer definition has this slug anymore — the
    /// submissions are still here (they're independent, linked only by
    /// name), but there's nothing to edit/re-embed. Surfaced as a badge
    /// rather than hidden, since the data is still real and actionable.
    pub definition_exists: bool,
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
    sort: &str,
    dir: &str,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    let mut sorted: Vec<&FormSummaryRow> = forms.iter().collect();
    match sort {
        "submissions" => sorted.sort_by_key(|f| f.submission_count),
        "last"        => sorted.sort_by(|a, b| a.last_submitted_at.cmp(&b.last_submitted_at)),
        "name"        => sorted.sort_by_key(|f| f.form_name.to_lowercase()),
        _ => {}
    }
    let asc = dir != "desc";
    if !sort.is_empty() && !asc {
        sorted.reverse();
    }

    // Sortable column header: link toggles asc/desc for that column.
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == key;
        let next_dir = if is_active && asc { "desc" } else { "asc" };
        let arrow = if is_active { if asc { " \u{25B2}" } else { " \u{25BC}" } } else { "" };
        format!(
            r#"<th><a href="/admin/form-data-analytics?sort={key}&dir={next_dir}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    let rows = if sorted.is_empty() {
        r#"<tr><td colspan="5" class="empty-state">No form submissions yet.</td></tr>"#.to_string()
    } else {
        sorted.iter().map(|f| {
            let blocked_badge = if f.blocked {
                r#" <span class="badge badge-danger" title="Not accepting submissions">Blocked</span>"#
            } else { "" };
            let deleted_badge = if f.definition_exists {
                ""
            } else {
                r#" <span class="badge" title="No form in Form Designer matches this name anymore — these are the submissions it collected before it was deleted.">Form deleted</span>"#
            };
            let block_btn = if f.blocked {
                format!(
                    r#"<form method="POST" action="/admin/form-data-analytics/{}/toggle-block" style="display:inline">
  <button class="icon-btn" type="submit" title="Unblock" aria-label="Unblock"><img src="/admin/static/icons/unlock.svg" alt=""></button>
</form>"#,
                    html_escape(&f.form_name)
                )
            } else {
                format!(
                    r#"<form method="POST" action="/admin/form-data-analytics/{}/toggle-block" style="display:inline"
      onsubmit="return confirm('Block this form? New submissions will be silently discarded.')">
  <button class="icon-btn icon-danger" type="submit" title="Block" aria-label="Block"><img src="/admin/static/icons/lock.svg" alt=""></button>
</form>"#,
                    html_escape(&f.form_name)
                )
            };
            let row_class = if f.blocked { " class=\"muted\"" } else { "" };
            format!(
                r#"<tr{row_class}>
  <td><a href="/admin/form-data-analytics/{name}">{name}</a>{blocked_badge}{deleted_badge}</td>
  <td>{count}</td>
  <td>{last}</td>
  <td>
    <a href="/admin/form-data-analytics/{name}/export" class="icon-btn" title="Export CSV" aria-label="Export CSV"><img src="/admin/static/icons/download.svg" alt=""></a>
    {block_btn}
  </td>
</tr>"#,
                row_class = row_class,
                name = html_escape(&f.form_name),
                count = f.submission_count,
                last = html_escape(&f.last_submitted_at),
                blocked_badge = blocked_badge,
                deleted_badge = deleted_badge,
                block_btn = block_btn,
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <div style="display:flex;align-items:center;gap:.75rem">
    <div class="icon-search-box" style="margin-bottom:0">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input id="forms-search" type="search" placeholder="Search forms&hellip;" autocomplete="off">
    </div>
  </div>
  <div style="display:flex;align-items:center;gap:.5rem">
    <div class="icon-pill">
      <a href="/admin/form-designer/new" class="icon-btn" title="New Form" aria-label="New Form"><img src="/admin/static/icons/file-plus.svg" alt=""></a>
    </div>
  </div>
</div>
<div class="table-wrap">
<table class="data-table">
  <thead>
    <tr>
      {name_th}
      {submissions_th}
      {last_th}
      <th>Actions</th>
    </tr>
  </thead>
  <tbody id="forms-table-body">
    {rows}
  </tbody>
</table>
</div>
<script>
(function() {{
  var input = document.getElementById('forms-search');
  var rows = document.querySelectorAll('#forms-table-body tr');
  if (!input) return;
  input.addEventListener('input', function() {{
    var q = input.value.trim().toLowerCase();
    rows.forEach(function(row) {{
      row.style.display = (!q || row.textContent.toLowerCase().indexOf(q) !== -1) ? '' : 'none';
    }});
  }});
}})();
</script>"#,
        rows = rows,
        name_th = sort_th("Form Name", "name"),
        submissions_th = sort_th("Submissions", "submissions"),
        last_th = sort_th("Last Submitted", "last"),
    );

    admin_page("Forms", "/admin/form-data-analytics", flash, &content, ctx)
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
    let base_path = format!("/admin/form-data-analytics/{}", html_escape(form_name));
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
    <form method="POST" action="/admin/form-data-analytics/{fname}/{id}/delete"
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
<div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <div style="display:flex;align-items:center;gap:.75rem">
    {search_box}
  </div>
  <div style="display:flex;align-items:center;gap:.5rem">
    <a href="/admin/form-data-analytics/{fname}/export" class="icon-btn" title="Export CSV" aria-label="Export CSV"><img src="/admin/static/icons/download.svg" alt=""></a>
    <form method="POST" action="/admin/form-data-analytics/{fname}/delete-all" style="display:inline"
          onsubmit="return confirm('Delete ALL submissions for this form?')">
      <button class="icon-btn icon-danger" type="submit" title="Delete All" aria-label="Delete All"><img src="/admin/static/icons/trash.svg" alt=""></button>
    </form>
    <a href="/admin/form-data-analytics" class="btn btn-secondary">← All Forms</a>
  </div>
</div>
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
    admin_page(&title, "/admin/form-data-analytics", flash, &content, ctx)
}
