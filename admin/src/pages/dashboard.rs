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
    /// Author posts chart: x-axis labels (days/weeks/months)
    pub author_chart_labels: Vec<String>,
    /// Author posts chart: published count for each label slot
    pub author_chart_values: Vec<f32>,
    /// Active range for the posts chart: "week", "month", or "year"
    pub chart_range: String,
    /// Author view chart: x-axis labels
    pub author_views_labels: Vec<String>,
    /// Author view chart: unique view count per label slot
    pub author_views_values: Vec<f32>,
    /// Active range for the views chart: "week", "month", or "year"
    pub views_range: String,
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

pub fn render(data: &DashboardData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = if ctx.user_role.eq_ignore_ascii_case("author") {
        // ── Posts chart ──────────────────────────────────────────────────────
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

        // ── Active tab classes (independent per chart) ────────────────────────
        let vr = data.views_range.as_str();
        let (paw, pam, pay) = match data.chart_range.as_str() {
            "month" => ("btn", "btn btn-primary", "btn"),
            "year"  => ("btn", "btn", "btn btn-primary"),
            _       => ("btn btn-primary", "btn", "btn"),
        };
        let (vaw, vam, vay) = match vr {
            "month" => ("btn", "btn btn-primary", "btn"),
            "year"  => ("btn", "btn", "btn btn-primary"),
            _       => ("btn btn-primary", "btn", "btn"),
        };

        // Posts chart tabs preserve the current views_range; views tabs preserve range.
        let pr  = &data.chart_range;
        format!(
            r#"<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-num">{published}</div>
    <div class="stat-label">Your Published Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{drafts}</div>
    <div class="stat-label">Your Drafts</div>
  </div>
  <div class="stat-card stat-card-pending">
    <div class="stat-num">{pending}</div>
    <div class="stat-label">Awaiting Review</div>
    {pending_link}
  </div>
  <div class="stat-card">
    <div class="stat-num">{total_views}</div>
    <div class="stat-label">Total Post Views</div>
  </div>
</div>
<div class="two-col" style="margin-top:1.5rem">
  <div class="card" style="padding:1.25rem">
    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
      <h3 style="margin:0;font-size:.95rem;font-weight:600">Published Posts</h3>
      <div style="display:flex;gap:.35rem">
        <a href="/admin?range=week&amp;views_range={vr}"  class="{paw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
        <a href="/admin?range=month&amp;views_range={vr}" class="{pam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
        <a href="/admin?range=year&amp;views_range={vr}"  class="{pay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
      </div>
    </div>
    {chart_html}
  </div>
  <div class="card" style="padding:1.25rem">
    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
      <h3 style="margin:0;font-size:.95rem;font-weight:600">Post Views</h3>
      <div style="display:flex;gap:.35rem">
        <a href="/admin?range={pr}&amp;views_range=week"  class="{vaw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
        <a href="/admin?range={pr}&amp;views_range=month" class="{vam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
        <a href="/admin?range={pr}&amp;views_range=year"  class="{vay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
      </div>
    </div>
    {views_chart_html}
  </div>
</div>"#,
            published        = data.author_published_posts,
            drafts           = data.author_draft_posts,
            pending          = data.author_pending_posts,
            total_views      = data.author_total_views,
            pending_link     = if data.author_pending_posts > 0 {
                r#"<a href="/admin/posts?status=pending" class="stat-action">View pending posts &rarr;</a>"#
            } else { "" },
            vr               = vr,
            pr               = pr,
            paw = paw, pam = pam, pay = pay,
            vaw = vaw, vam = vam, vay = vay,
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
