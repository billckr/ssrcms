//! Admin Analytics — tabbed overview at /admin/analytics. General (site-wide
//! stats, placeholder for now) and Forms (the form-submissions summary table
//! that used to be the whole of /admin/form-data-analytics — individual
//! form submission pages still live at their own /admin/form-data-analytics/
//! {name} URLs, unchanged for now).

use crate::pages::forms::{forms_tab_content, forms_tab_controls, FormSummaryRow};
use crate::{admin_page, html_escape, PageContext};

pub fn render(
    active_tab: &str,
    forms: &[FormSummaryRow],
    sort: &str,
    dir: &str,
    flash: Option<&str>,
    ctx: &PageContext,
) -> String {
    let is_forms = active_tab == "forms";
    let general_active = if is_forms { "" } else { " active" };
    let forms_active = if is_forms { " active" } else { "" };

    let tabs = format!(
        r#"<div class="page-tabs" style="margin-bottom:0">
  <a href="/admin/analytics?tab=general" class="page-tab{general_active}">General</a>
  <a href="/admin/analytics?tab=forms" class="page-tab{forms_active}">Forms</a>
</div>"#,
        general_active = general_active,
        forms_active = forms_active,
    );

    // Each tab supplies its own controls on this same row (search, New
    // Form, etc.) — same layout convention as /admin/pages: tabs and
    // controls side by side, not controls stacked below the tab bar.
    let controls = if is_forms { forms_tab_controls() } else { String::new() };

    let tab_body = if is_forms {
        forms_tab_content(forms, sort, dir)
    } else {
        // Placeholder — site-wide stats land here in a later pass.
        r#"<div class="empty-state">General analytics are coming soon.</div>"#.to_string()
    };

    let content = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:space-between;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  {tabs}
  {controls}
</div>
{tab_body}"#,
        tabs = tabs,
        controls = controls,
        tab_body = tab_body,
    );
    admin_page("Analytics", "/admin/analytics", flash, &content, ctx)
}

// ── Per-form mail delivery log (moved from the old /admin/form-analytics/{id},
// itself part of Form Designer) — now under /admin/analytics/form/{id},
// matching the new base. ──────────────────────────────────────────────────

/// One row of a form's email send history, already formatted for display.
pub struct MailLogRow {
    pub to_email: String,
    pub subject: String,
    pub success: bool,
    pub mailgun_message_id: Option<String>,
    pub error: Option<String>,
    pub sent_at: String,
}

pub struct FormAnalyticsData {
    pub id: String,
    pub form_name: String,
    pub total_sent: i64,
    pub succeeded: i64,
    pub failed: i64,
    /// Most recent sends, newest first.
    pub recent: Vec<MailLogRow>,
}

/// Compute integer Y-axis bounds so every tick label is a whole number —
/// same approach as the dashboard's author charts.
fn integer_y_axis(values: &[f32]) -> (f32, usize) {
    let max_val = values.iter().cloned().fold(0.0f32, f32::max);
    let max_int = (max_val.ceil() as u32).max(1) as f32;
    let step = if max_int <= 10.0 {
        1.0
    } else if max_int <= 20.0 {
        2.0
    } else if max_int <= 50.0 {
        5.0
    } else if max_int <= 100.0 {
        10.0
    } else if max_int <= 500.0 {
        50.0
    } else {
        100.0
    };
    let axis_max = (max_int / step).ceil() * step;
    let splits = (axis_max / step) as usize;
    (axis_max, splits.max(1))
}

/// Post-process a charts-rs SVG to be responsive — same helper as the
/// dashboard's charts.
fn responsive_svg(svg: String, w: u32, h: u32) -> String {
    let vb = format!(r#"viewBox="0 0 {w} {h}""#);
    let svg = svg.replacen(&format!(r#"width="{w}""#), &format!(r#"width="100%" {vb}"#), 1);
    svg.replacen(&format!(r#" height="{h}""#), "", 1)
}

/// The Recent Sends `<table>` (sortable headers + rows) — shared by the full
/// page render and the live-search partial endpoint, so both stay in sync.
/// `search_qs` is the `&search=...` suffix (already URL-encoded, empty when
/// there's no active search) appended to each sort link so sorting doesn't
/// clear an in-progress search.
pub fn render_analytics_table(data: &FormAnalyticsData, sort: &str, dir: &str, search_qs: &str) -> String {
    let asc = dir != "desc";
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == key;
        let next_dir = if is_active && asc { "desc" } else { "asc" };
        let arrow = if is_active { if asc { " \u{25B2}" } else { " \u{25BC}" } } else { "" };
        format!(
            r#"<th><a href="?sort={key}&dir={next_dir}{search_qs}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    let rows_html = if data.recent.is_empty() {
        r#"<tr><td colspan="5" style="text-align:center;color:var(--muted)">No matching sends.</td></tr>"#.to_string()
    } else {
        data.recent.iter().map(|r| {
            let status = if r.success {
                r#"<span class="badge" style="background:#dcfce7;color:#166534">Delivered</span>"#.to_string()
            } else {
                r#"<span class="badge" style="background:#fee2e2;color:#991b1b">Failed</span>"#.to_string()
            };
            let detail = if r.success {
                r.mailgun_message_id.as_deref().unwrap_or("")
            } else {
                r.error.as_deref().unwrap_or("")
            };
            format!(
                r#"<tr>
  <td>{to}</td>
  <td>{subject}</td>
  <td>{status}</td>
  <td style="color:var(--muted);font-size:.8rem;max-width:320px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="{detail_title}">{detail}</td>
  <td style="white-space:nowrap;color:var(--muted)">{sent_at}</td>
</tr>"#,
                to = html_escape(&r.to_email),
                subject = html_escape(&r.subject),
                status = status,
                detail = html_escape(detail),
                detail_title = html_escape(detail),
                sent_at = html_escape(&r.sent_at),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    format!(
        r#"<table class="data-table">
  <thead><tr>{to_th}{subject_th}{status_th}<th>Message ID / Error</th>{sent_th}</tr></thead>
  <tbody>{rows_html}</tbody>
</table>"#,
        to_th = sort_th("To", "to"),
        subject_th = sort_th("Subject", "subject"),
        status_th = sort_th("Status", "status"),
        sent_th = sort_th("Sent", "sent"),
        rows_html = rows_html,
    )
}

/// A form's email send history — counts, a Delivered/Failed bar chart, and
/// a recent-sends table. Reads straight from `mail_log`, scoped to this form.
/// `sort`/`dir` reflect the current sort (rows are pre-filtered/sorted by the
/// caller) and are only used here to render the column headers' state/links.
pub fn render_analytics(data: &FormAnalyticsData, active_tab: &str, sort: &str, dir: &str, search: &str, ctx: &PageContext) -> String {
    let is_results = active_tab == "results";
    let stats_active = if is_results { "" } else { " active" };
    let results_active = if is_results { " active" } else { "" };
    let id = html_escape(&data.id);

    let tabs = format!(
        r#"<div class="page-tabs" style="margin-bottom:0">
  <a href="/admin/analytics/form/{id}?tab=stats" class="page-tab{stats_active}">Stats</a>
  <a href="/admin/analytics/form/{id}?tab=results" class="page-tab{results_active}">Delivery Results</a>
</div>"#,
        id = id, stats_active = stats_active, results_active = results_active,
    );

    // Search only makes sense against the Delivery Results table — same
    // layout convention as elsewhere: tab bar and controls on one row (see
    // pages::analytics::render for the tabs list, and forms_tab_controls).
    let search_toggle = crate::pill_search_toggle("analytics-search", "Search sends&hellip;", search);
    let controls = if is_results {
        format!(r#"<div class="icon-pill" style="align-self:flex-end;margin-top:0">{search_toggle}</div>"#, search_toggle = search_toggle)
    } else {
        String::new()
    };

    let tab_body = if is_results {
        let search_qs = if search.is_empty() { String::new() } else { format!("&search={}", html_escape(search)) };
        let table_html = render_analytics_table(data, sort, dir, &search_qs);
        let fetch_prefix = format!("/admin/analytics/form/{}?tab=results&partial=1&sort={}&dir={}", data.id, sort, dir);
        let live_search = crate::live_search_script("analytics-search", "analytics-table", &fetch_prefix);
        format!(
            r#"<div id="analytics-table">{table_html}</div>
{live_search}
{pill_search_init}"#,
            table_html = table_html,
            live_search = live_search,
            pill_search_init = crate::pill_search_init_script(),
        )
    } else {
        let chart_html = if data.total_sent == 0 {
            r#"<p class="field-hint" style="padding:1.5rem 0;text-align:center">No emails sent for this form yet.</p>"#.to_string()
        } else {
            use charts_rs::{BarChart, Color, Series};
            let values = vec![data.succeeded as f32, data.failed as f32];
            let (y_max, y_splits) = integer_y_axis(&values);
            let mut chart = BarChart::new(
                vec![Series::new("Emails".to_string(), values)],
                vec!["Delivered".to_string(), "Failed".to_string()],
            );
            chart.background_color = Color::transparent();
            chart.width = 600.0;
            chart.height = 220.0;
            chart.legend_show = Some(false);
            chart.font_family = "system-ui, -apple-system, sans-serif".to_string();
            chart.series_colors = vec![Color::from("#16a34a"), Color::from("#dc2626")];
            chart.y_axis_configs[0].axis_min = Some(0.0);
            chart.y_axis_configs[0].axis_max = Some(y_max);
            chart.y_axis_configs[0].axis_split_number = y_splits;
            responsive_svg(chart.svg().unwrap_or_default(), 600, 220)
        };
        format!(
            r#"<div class="card-boxed">
  <div class="card-boxed-body">
    <div style="display:flex;gap:2rem;flex-wrap:wrap;margin-bottom:1.5rem">
      <div><div style="font-size:1.6rem;font-weight:700">{total}</div><div class="field-hint">Total sent</div></div>
      <div><div style="font-size:1.6rem;font-weight:700;color:#16a34a">{succeeded}</div><div class="field-hint">Delivered</div></div>
      <div><div style="font-size:1.6rem;font-weight:700;color:#dc2626">{failed}</div><div class="field-hint">Failed</div></div>
    </div>
    <div style="max-width:420px">{chart_html}</div>
  </div>
</div>"#,
            total = data.total_sent,
            succeeded = data.succeeded,
            failed = data.failed,
            chart_html = chart_html,
        )
    };

    let content = format!(
        r#"<div style="display:flex;align-items:flex-end;justify-content:space-between;gap:.75rem;margin-bottom:1.25rem;flex-wrap:wrap">
  {tabs}
  {controls}
</div>
{tab_body}"#,
        tabs = tabs,
        controls = controls,
        tab_body = tab_body,
    );

    admin_page(&format!("Analytics - {}", html_escape(&data.form_name)), "/admin/analytics", None, &content, ctx)
}
