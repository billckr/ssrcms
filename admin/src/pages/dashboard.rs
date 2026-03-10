//! Admin dashboard page.

pub struct DashboardData {
    pub published_posts: i64,
    pub draft_posts: i64,
    pub total_pages: i64,
    pub total_users: i64,
    /// Posts waiting for editor review (all roles see this on their dashboard).
    pub pending_posts: i64,
    /// Author-scoped counts (only meaningful when user_role == "author").
    pub author_draft_posts: i64,
    pub author_pending_posts: i64,
    pub author_published_posts: i64,
    /// Author posts chart: x-axis labels (weeks/months/years)
    pub author_chart_labels: Vec<String>,
    /// Author posts chart: published count for each label slot
    pub author_chart_values: Vec<f32>,
    /// Active range for the posts chart: "week", "month", or "year"
    pub chart_range: String,
    /// Years that have published posts (for dropdown); most recent first
    pub available_years: Vec<i32>,
    /// Currently selected year for the posts chart
    pub selected_year: i32,
    /// Author view chart: x-axis labels
    pub author_views_labels: Vec<String>,
    /// Author view chart: unique view count per label slot
    pub author_views_values: Vec<f32>,
    /// Active range for the views chart: "week", "month", or "year"
    pub views_range: String,
    /// Years that have view data (for dropdown); most recent first
    pub available_views_years: Vec<i32>,
    /// Currently selected year for the views chart
    pub selected_views_year: i32,
    /// All-time total unique views across the author's posts
    pub author_total_views: i64,
}

/// Compute integer Y-axis bounds for a set of count values.
/// Returns `(axis_max, split_number)` so that every tick label is a whole
/// number.  The step size is chosen to keep the tick count ≤ 10.
fn integer_y_axis(values: &[f32]) -> (f32, usize) {
    let max_val = values.iter().cloned().fold(0.0f32, f32::max);
    let max_int = (max_val.ceil() as u32).max(1) as f32;
    // Pick a step that divides max_int evenly and keeps splits ≤ 10.
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

/// Post-process a charts-rs SVG to be responsive.
/// Replaces the fixed `width` attribute with `width="100%"` and adds a
/// `viewBox` so the chart scales to fill its container at any screen size.
fn responsive_svg(svg: String, w: u32, h: u32) -> String {
    let vb = format!(r#"viewBox="0 0 {w} {h}""#);
    // Replace `width="W"` → `width="100%" viewBox="0 0 W H"`
    let svg = svg.replacen(&format!(r#"width="{w}""#), &format!(r#"width="100%" {vb}"#), 1);
    // Remove the explicit height so CSS controls it via `height: auto`
    svg.replacen(&format!(r#" height="{h}""#), "", 1)
}

/// Build a year <select> form that navigates to /admin with all current params preserved.
/// `hide_on_year_tab`: pass true so the dropdown is omitted when the active tab is "year"
/// (since Year view spans all time and the per-year filter is irrelevant).
fn year_select(
    select_name: &str,
    selected: i32,
    available: &[i32],
    range: &str,
    views_range: &str,
    year: i32,
    views_year: i32,
    hide_on_year_tab: bool,
    active_tab: &str,
) -> String {
    if hide_on_year_tab && active_tab == "year" {
        return String::new();
    }
    let options: String = if available.is_empty() {
        format!("<option value=\"{selected}\" selected>{selected}</option>")
    } else {
        available.iter().map(|&y| {
            if y == selected {
                format!("<option value=\"{y}\" selected>{y}</option>")
            } else {
                format!("<option value=\"{y}\">{y}</option>")
            }
        }).collect()
    };
    // Only emit hidden inputs for params that the <select> itself does NOT control,
    // to avoid duplicate query string fields on submit.
    let year_hidden = if select_name != "year" {
        format!(r#"<input type="hidden" name="year" value="{year}">"#)
    } else {
        String::new()
    };
    let views_year_hidden = if select_name != "views_year" {
        format!(r#"<input type="hidden" name="views_year" value="{views_year}">"#)
    } else {
        String::new()
    };
    format!(
        r#"<form method="GET" action="/admin" style="display:inline-flex;align-items:center">
  <input type="hidden" name="range" value="{range}">
  <input type="hidden" name="views_range" value="{views_range}">
  {year_hidden}{views_year_hidden}<select name="{select_name}" onchange="this.form.submit()" style="font-size:12px;padding:.2rem .5rem;border:1px solid var(--border);border-radius:4px;background:var(--card-bg);color:inherit;cursor:pointer">{options}</select>
</form>"#,
        select_name = select_name,
        range = range,
        views_range = views_range,
        year_hidden = year_hidden,
        views_year_hidden = views_year_hidden,
        options = options,
    )
}

pub fn render(data: &DashboardData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = if ctx.user_role.eq_ignore_ascii_case("author") {
        let y  = data.selected_year;
        let vy = data.selected_views_year;
        let pr = &data.chart_range;
        let vr = &data.views_range;

        // ── Posts chart ───────────────────────────────────────────────────────
        let chart_html = {
            let all_zero = data.author_chart_values.iter().all(|&v| v == 0.0);
            if data.author_chart_labels.is_empty() || all_zero {
                r#"<div style="text-align:center;padding:2rem;color:var(--muted);font-size:13px">No published posts in this period.</div>"#
                    .to_string()
            } else {
                use charts_rs::{BarChart, Color, Series};
                let (y_max, y_splits) = integer_y_axis(&data.author_chart_values);
                let mut chart = BarChart::new(
                    vec![Series::new("Published".to_string(), data.author_chart_values.clone())],
                    data.author_chart_labels.clone(),
                );
                chart.background_color = Color::transparent();
                chart.width = 600.0;
                chart.height = 260.0;
                chart.legend_show = Some(false);
                chart.font_family = "system-ui, -apple-system, sans-serif".to_string();
                chart.y_axis_configs[0].axis_min = Some(0.0);
                chart.y_axis_configs[0].axis_max = Some(y_max);
                chart.y_axis_configs[0].axis_split_number = y_splits;
                responsive_svg(chart.svg().unwrap_or_default(), 600, 260)
            }
        };

        // ── Views chart ───────────────────────────────────────────────────────
        let views_chart_html = {
            let all_zero = data.author_views_values.iter().all(|&v| v == 0.0);
            if data.author_views_labels.is_empty() || all_zero {
                r#"<div style="text-align:center;padding:2rem;color:var(--muted);font-size:13px">No views recorded in this period.</div>"#
                    .to_string()
            } else {
                use charts_rs::{BarChart, Color, Series};
                let (y_max, y_splits) = integer_y_axis(&data.author_views_values);
                let mut chart = BarChart::new(
                    vec![Series::new("Views".to_string(), data.author_views_values.clone())],
                    data.author_views_labels.clone(),
                );
                chart.background_color = Color::transparent();
                chart.width = 600.0;
                chart.height = 260.0;
                chart.legend_show = Some(false);
                chart.font_family = "system-ui, -apple-system, sans-serif".to_string();
                chart.y_axis_configs[0].axis_min = Some(0.0);
                chart.y_axis_configs[0].axis_max = Some(y_max);
                chart.y_axis_configs[0].axis_split_number = y_splits;
                responsive_svg(chart.svg().unwrap_or_default(), 600, 260)
            }
        };

        // ── Tab active classes ────────────────────────────────────────────────
        let (paw, pam, pay) = match pr.as_str() {
            "month" => ("btn", "btn btn-primary", "btn"),
            "year"  => ("btn", "btn", "btn btn-primary"),
            _       => ("btn btn-primary", "btn", "btn"),
        };
        let (vaw, vam, vay) = match vr.as_str() {
            "month" => ("btn", "btn btn-primary", "btn"),
            "year"  => ("btn", "btn", "btn btn-primary"),
            _       => ("btn btn-primary", "btn", "btn"),
        };

        // ── Year selects (hidden on "year" tab since it spans all time) ───────
        let posts_year_sel = year_select(
            "year", y, &data.available_years,
            pr, vr, y, vy,
            true, pr,
        );
        let views_year_sel = year_select(
            "views_year", vy, &data.available_views_years,
            pr, vr, y, vy,
            true, vr,
        );

        // Pending stat gets amber treatment when there are posts awaiting review.
        let pending_num_style = if data.author_pending_posts > 0 {
            "color:#d97706"
        } else {
            ""
        };
        let pending_link_html = if data.author_pending_posts > 0 {
            r#" &nbsp;<a href="/admin/posts?status=pending" style="font-size:.75rem;color:#d97706;text-decoration:none;white-space:nowrap">Review &rarr;</a>"#
        } else { "" };

        format!(
            r#"
<style>
.author-stat-bar {{ display:grid;grid-template-columns:repeat(4,1fr);gap:.75rem;margin-bottom:1.25rem }}
.author-stat-bar .sb-item {{ background:var(--card-bg);border:1px solid var(--border);border-radius:8px;padding:.6rem .75rem;text-align:center }}
.author-stat-bar .sb-num {{ font-size:1.5rem;font-weight:700;line-height:1.2 }}
.author-stat-bar .sb-label {{ font-size:.78rem;color:var(--muted);margin-top:.1rem }}
@media(max-width:640px){{ .author-stat-bar {{ grid-template-columns:repeat(2,1fr) }} }}
</style>
<div class="author-stat-bar">
  <div class="sb-item">
    <div class="sb-num" style="color:var(--accent)">{published}</div>
    <div class="sb-label">Published</div>
  </div>
  <div class="sb-item">
    <div class="sb-num">{drafts}</div>
    <div class="sb-label">Drafts</div>
  </div>
  <div class="sb-item">
    <div class="sb-num" style="{pending_num_style}">{pending}</div>
    <div class="sb-label">Awaiting Review{pending_link_html}</div>
  </div>
  <div class="sb-item">
    <div class="sb-num">{total_views}</div>
    <div class="sb-label">Total Views</div>
  </div>
</div>
<div class="two-col">
  <div>
    <div class="card" style="padding:1.25rem;margin-bottom:1rem">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
        <h3 style="margin:0;font-size:.95rem;font-weight:600">Published Posts</h3>
        <div style="display:flex;align-items:center;gap:.5rem">
          {posts_year_sel}
          <div style="display:flex;gap:.35rem">
            <a href="/admin?range=week&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}"  class="{paw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
            <a href="/admin?range=month&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}" class="{pam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
            <a href="/admin?range=year&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}"  class="{pay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
          </div>
        </div>
      </div>
      {chart_html}
    </div>
    <div class="card" style="padding:1.25rem">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
        <h3 style="margin:0;font-size:.95rem;font-weight:600">Post Views</h3>
        <div style="display:flex;align-items:center;gap:.5rem">
          {views_year_sel}
          <div style="display:flex;gap:.35rem">
            <a href="/admin?range={pr}&amp;views_range=week&amp;year={y}&amp;views_year={vy}"  class="{vaw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
            <a href="/admin?range={pr}&amp;views_range=month&amp;year={y}&amp;views_year={vy}" class="{vam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
            <a href="/admin?range={pr}&amp;views_range=year&amp;year={y}&amp;views_year={vy}"  class="{vay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
          </div>
        </div>
      </div>
      {views_chart_html}
    </div>
  </div>
  <div>
    <!-- right column: reserved for future widgets -->
  </div>
</div>"#,
            published         = data.author_published_posts,
            drafts            = data.author_draft_posts,
            pending           = data.author_pending_posts,
            total_views       = data.author_total_views,
            pending_num_style = pending_num_style,
            pending_link_html = pending_link_html,
            y  = y,  vy = vy,
            pr = pr, vr = vr,
            paw = paw, pam = pam, pay = pay,
            vaw = vaw, vam = vam, vay = vay,
            posts_year_sel   = posts_year_sel,
            views_year_sel   = views_year_sel,
            chart_html       = chart_html,
            views_chart_html = views_chart_html,
        )
    } else if ctx.user_role.eq_ignore_ascii_case("editor") {
        format!(
            r#"<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-num">{published}</div>
    <div class="stat-label">Published Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{drafts}</div>
    <div class="stat-label">Drafts</div>
  </div>
  <div class="stat-card stat-card-pending">
    <div class="stat-num">{pending}</div>
    <div class="stat-label">Pending Review</div>
    {pending_link}
  </div>
</div>"#,
            published = data.published_posts,
            drafts    = data.draft_posts,
            pending   = data.pending_posts,
            pending_link = if data.pending_posts > 0 {
                r#"<a href="/admin/posts?status=pending" class="stat-action">Review submissions &rarr;</a>"#
            } else {
                r#"<p class="stat-hint">No posts pending review.</p>"#
            },
        )
    } else {
        format!(
            r#"<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-num">{published_posts}</div>
    <div class="stat-label">Published Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{draft_posts}</div>
    <div class="stat-label">Draft Posts</div>
  </div>
  <div class="stat-card stat-card-pending">
    <div class="stat-num">{pending}</div>
    <div class="stat-label">Pending Review</div>
    {pending_link}
  </div>
  <div class="stat-card">
    <div class="stat-num">{total_pages}</div>
    <div class="stat-label">Pages</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{total_users}</div>
    <div class="stat-label">Users</div>
  </div>
</div>"#,
            published_posts = data.published_posts,
            draft_posts = data.draft_posts,
            pending = data.pending_posts,
            pending_link = if data.pending_posts > 0 {
                r#"<a href="/admin/posts?status=pending" class="stat-action">Review submissions &rarr;</a>"#
            } else {
                r#"<p class="stat-hint">No posts pending review.</p>"#
            },
            total_pages = data.total_pages,
            total_users = data.total_users,
        )
    };

    crate::admin_page("Dashboard", "/admin", flash, &content, ctx)
}
