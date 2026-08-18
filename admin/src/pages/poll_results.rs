//! Read-only admin view of a poll's tallied results and raw vote log —
//! sibling to `pages::forms`' submissions view, but tally-oriented rather
//! than dynamic-column-oriented since a poll's shape (one question, a fixed
//! option list) never varies per vote the way a form's fields can.

use crate::{admin_page, html_escape, PageContext};

pub struct ResultOption {
    pub label: String,
    pub votes: i64,
    pub percent: u32,
}

pub struct VoteRow {
    pub id: String,
    pub option_label: String,
    pub ip_address: String,
    pub voted_at: String,
}

fn pagination(base: &str, page: i64, total_pages: i64) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let prev = if page > 1 {
        format!(r#"<a href="{base}?page={}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="{base}?page={}" class="page-btn">Next &raquo;</a>"#, page + 1)
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
            nums.push_str(&format!(r#"<a href="{base}?page={p}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

pub fn render_results(
    poll_id: &str,
    poll_name: &str,
    options: &[ResultOption],
    total_votes: i64,
    votes: &[VoteRow],
    page: i64,
    total_pages: i64,
    ctx: &PageContext,
    flash: Option<&str>,
) -> String {
    let bars = if options.is_empty() {
        r#"<p class="empty-state">No options defined.</p>"#.to_string()
    } else {
        options.iter().map(|o| format!(
            r#"<div class="ss-poll-result-row" style="margin-bottom:12px">
  <div style="display:flex;justify-content:space-between;font-size:.875rem;margin-bottom:4px"><span>{label}</span><span>{pct}% ({votes})</span></div>
  <div style="background:var(--tint);border-radius:4px;height:10px;overflow:hidden"><div style="width:{pct}%;height:100%;background:var(--primary)"></div></div>
</div>"#,
            label = html_escape(&o.label), pct = o.percent, votes = o.votes,
        )).collect::<Vec<_>>().join("\n")
    };

    let vote_rows = if votes.is_empty() {
        r#"<tr><td colspan="3" style="text-align:center;color:var(--muted)">No votes yet.</td></tr>"#.to_string()
    } else {
        votes.iter().map(|v| format!(
            r#"<tr><td>{option}</td><td>{ip}</td><td>{when}</td></tr>"#,
            option = html_escape(&v.option_label),
            ip = if v.ip_address.is_empty() { "—".to_string() } else { html_escape(&v.ip_address) },
            when = html_escape(&v.voted_at),
        )).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div class="card-boxed" style="max-width:640px;margin-bottom:1.25rem">
  <h2 class="card-boxed-header">Results</h2>
  <div class="card-boxed-body">
    <div class="card-boxed-section">
      {bars}
      <p style="font-size:.85rem;color:var(--muted);margin:.5rem 0 0">{total_votes} total votes</p>
    </div>
  </div>
</div>

<div style="display:flex;align-items:center;justify-content:flex-end;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  <div class="icon-pill" style="align-self:flex-end;margin-top:0">
    <a href="/admin/designer/polls/{poll_id}" class="icon-btn" title="Edit Poll" aria-label="Edit Poll"><img src="/admin/static/icons/edit.svg" alt=""></a>
    <a href="/admin/designer/polls/{poll_id}/results/export" download class="icon-btn" title="Export CSV" aria-label="Export CSV"><img src="/admin/static/icons/download.svg" alt=""></a>
    <form method="POST" action="/admin/designer/polls/{poll_id}/results/reset" style="display:inline"
          onsubmit="return confirm('Delete ALL votes for this poll and reset the count to zero?')">
      <button class="icon-btn icon-danger" type="submit" title="Reset Results" aria-label="Reset Results"><img src="/admin/static/icons/trash.svg" alt=""></button>
    </form>
  </div>
</div>

<table class="data-table">
  <thead><tr><th>Option</th><th>IP</th><th>Voted</th></tr></thead>
  <tbody>{vote_rows}</tbody>
</table>
{pagination}"#,
        bars = bars,
        total_votes = total_votes,
        poll_id = html_escape(poll_id),
        vote_rows = vote_rows,
        pagination = pagination(&format!("/admin/designer/polls/{}/results", html_escape(poll_id)), page, total_pages),
    );

    let title = format!("Poll: {poll_name}");
    admin_page(&title, "/admin/designer", flash, &content, ctx)
}
