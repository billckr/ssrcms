//! Admin dashboard page.

use std::collections::HashMap;

pub struct DashboardData {
    pub total_sites: i64,
    pub total_users: i64,
    pub total_subscribers: i64,
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
    /// Saved widget column/order preference, e.g. {"left": ["one"], "middle": ["two"], "right": ["three"]}.
    /// `None` uses the default layout.
    pub widget_layout: Option<serde_json::Value>,
    /// Up to 5 most recently updated drafts (scoped to the current user/site), for the
    /// "Recent Drafts" widget.
    pub recent_drafts: Vec<RecentPostSummary>,
    /// Up to 5 most recently published posts (scoped to the current user/site), for the
    /// "Recently Published" widget.
    pub recent_published: Vec<RecentPostSummary>,
    /// Up to 5 most recently submitted posts pending review (scoped to the current
    /// user/site), for the "Pending Review" widget.
    pub recent_pending: Vec<RecentPostSummary>,
    /// Next 5 upcoming scheduled posts, soonest first (scoped to the current
    /// user/site), for the "Scheduled" widget.
    pub upcoming_scheduled: Vec<RecentPostSummary>,
    /// Whether the dashboard Welcome panel should render — false once the
    /// current user has dismissed it (`users.welcome_panel_dismissed_at`).
    pub show_welcome_panel: bool,
    /// Total posts across the install (super-admin only; 0 otherwise). Drives
    /// the Welcome panel's dynamic "getting started" card.
    pub total_posts_ever: i64,
}

pub struct RecentPostSummary {
    pub id: String,
    pub title: String,
    pub site_hostname: String,
    /// Formatted scheduled publish time (e.g. "2026-08-01 09:00 UTC"), only
    /// populated for the Scheduled widget.
    pub scheduled_at: Option<String>,
    pub author_name: String,
}

/// Truncate a title to `max_chars` characters (by Unicode scalar, not byte), appending
/// "…" when it was cut short, so long titles don't blow out the widget card width.
fn truncate_title(title: &str, max_chars: usize) -> String {
    if title.chars().count() <= max_chars {
        title.to_string()
    } else {
        let truncated: String = title.chars().take(max_chars).collect();
        format!("{truncated}\u{2026}")
    }
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
  {year_hidden}{views_year_hidden}<select name="{select_name}" onchange="this.form.submit()" style="font-size:12px;padding:.2rem .5rem;border:1px solid var(--border);border-radius:4px;background:var(--field-bg);color:var(--field-text);cursor:pointer">{options}</select>
</form>"#,
        select_name = select_name,
        range = range,
        views_range = views_range,
        year_hidden = year_hidden,
        views_year_hidden = views_year_hidden,
        options = options,
    )
}

/// Max characters shown for a post title inside a dashboard widget row, so long
/// titles don't blow out the card width. Longer titles are truncated with "…".
const WIDGET_TITLE_MAX_CHARS: usize = 25;

/// What the second column of a `recent_posts_widget` table shows: the post's
/// author (Drafts / Published / Pending Review — a status badge here would just
/// repeat the widget's own filter) or the scheduled publish time (Scheduled).
enum SecondColumn {
    Author,
    ScheduledTime,
}

/// Renders a "recent posts" widget body (title, second column, domain) for the
/// Drafts, Published, Pending Review, or Scheduled widgets, mirroring the columns
/// shown on `/admin/posts?status=...`. The title links to the post editor.
fn recent_posts_widget(
    posts: &[RecentPostSummary],
    heading: &str,
    empty_message: &str,
    second_column: SecondColumn,
    boxed: bool,
) -> String {
    let second_col_header = match second_column {
        SecondColumn::Author => "Author",
        SecondColumn::ScheduledTime => "Scheduled For",
    };

    // "Boxed" widgets show their heading in the card's own grey header bar
    // (see `widgets_section`), so the body here omits it.
    let heading_html = if boxed {
        String::new()
    } else {
        format!(r#"<h3 style="margin:0 0 .75rem;font-size:.95rem;font-weight:600">{heading}</h3>"#)
    };

    if posts.is_empty() {
        return format!(
            r#"{heading_html}<p style="margin:0;color:var(--muted);font-size:.85rem">{empty_message}</p>"#
        );
    }

    let rows: String = {
        posts.iter().map(|p| {
            let domain_cell = if p.site_hostname.is_empty() {
                r#"<span style="color:var(--muted);font-size:0.8rem">&mdash;</span>"#.to_string()
            } else {
                format!(
                    r#"<span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500;white-space:nowrap">{}</span>"#,
                    crate::html_escape(&p.site_hostname),
                )
            };
            let second_cell = match second_column {
                SecondColumn::Author => crate::html_escape(&p.author_name),
                SecondColumn::ScheduledTime => match &p.scheduled_at {
                    Some(when) => format!(
                        r#"<span style="font-size:.8rem;white-space:nowrap">{}</span>"#,
                        crate::html_escape(when),
                    ),
                    None => r#"<span style="color:var(--muted);font-size:0.8rem">&mdash;</span>"#.to_string(),
                },
            };
            format!(
                r#"<tr style="border-top:1px solid var(--border)">
      <td style="padding:.45rem .4rem .45rem 0"><a href="/admin/posts/{id}/edit" title="{full_title}">{title}</a></td>
      <td style="padding:.45rem .4rem">{second_cell}</td>
      <td style="padding:.45rem 0">{domain_cell}</td>
    </tr>"#,
                id = crate::html_escape(&p.id),
                full_title = crate::html_escape(&p.title),
                title = crate::html_escape(&truncate_title(&p.title, WIDGET_TITLE_MAX_CHARS)),
                second_cell = second_cell,
                domain_cell = domain_cell,
            )
        }).collect()
    };

    format!(
        r#"{heading_html}<table style="width:100%;border-collapse:collapse;font-size:.85rem">
  <thead>
    <tr style="text-align:left;color:var(--muted);font-size:.72rem;text-transform:uppercase">
      <th style="padding:.3rem .4rem .3rem 0;font-weight:500">Title</th>
      <th style="padding:.3rem .4rem;font-weight:500">{second_col_header}</th>
      <th style="padding:.3rem 0;font-weight:500">Domain</th>
    </tr>
  </thead>
  <tbody>
    {rows}
  </tbody>
</table>"#,
        rows = rows,
    )
}

/// Renders the "Quick Tools" widget: a short list of shortcuts to create new
/// content, scoped to what the current role is allowed to create.
fn quick_tools_widget(ctx: &crate::PageContext) -> String {
    let mut items = vec![
        r#"<a href="/admin/posts/new">New Post</a>"#.to_string(),
    ];
    if ctx.can_manage_pages {
        items.push(r#"<a href="/admin/pages/new">New Page</a>"#.to_string());
    }
    if ctx.can_manage_sites {
        items.push(r#"<a href="/admin/sites/new">New Site</a>"#.to_string());
    }
    if ctx.can_manage_users {
        items.push(r#"<a href="/admin/users/new">New User</a>"#.to_string());
    }

    let links: String = items.join("\n  ");

    format!(
        r#"<div style="display:flex;flex-direction:column;gap:.5rem">
  {links}
</div>"#,
    )
}

/// Dismissible hero banner shown above the widget grid until the user
/// closes it (persisted per-user via `users.welcome_panel_dismissed_at`,
/// see `dismiss_welcome_panel` in `core::handlers::admin::dashboard`).
/// Placeholder copy/links per the current plan — headline and the three
/// feature cards will get real copy later; the "what makes us different"
/// link target doesn't exist yet either.
fn welcome_panel_html(total_posts_ever: i64) -> String {
    let third_card = if total_posts_ever == 0 {
        r#"<div class="welcome-panel-card">
      <div class="welcome-panel-icon"><img src="/admin/static/icons/edit.svg" alt=""></div>
      <div>
        <h3>Get your content in</h3>
        <p>Write your first post, or bring your existing content over with the built-in WordPress importer.</p>
        <a href="/admin/posts/new">Write a Post</a> &nbsp;&middot;&nbsp;
        <a href="http://pong.com/admin/sites/bf9025dc-5196-4442-bb04-a1edf13fbc2e/settings?tab=import">Import from WordPress</a>
      </div>
    </div>"#.to_string()
    } else {
        r#"<div class="welcome-panel-card">
      <div class="welcome-panel-icon"><img src="/admin/static/icons/layers.svg" alt=""></div>
      <div>
        <h3>Run every client site from one install</h3>
        <p>Manage content, media, and users across every site without juggling separate installs or databases.</p>
        <a href="/admin/sites">Go to Sites</a>
      </div>
    </div>"#.to_string()
    };

    let head = r##"<div class="welcome-panel" id="welcome-panel">
  <button type="button" class="welcome-panel-dismiss" title="Dismiss" aria-label="Dismiss" onclick="dismissWelcomePanel()">
    <img src="/admin/static/icons/x.svg" alt="">
  </button>
  <div class="welcome-panel-hero">
    <h1>Welcome to SynapCMS!</h1>
    <a href="/admin/whats-different" target="_blank" rel="noopener">What makes SynapCMS different.</a>
  </div>
  <div class="welcome-panel-cards">
    <div class="welcome-panel-card">
      <div class="welcome-panel-icon"><img src="/admin/static/icons/zap.svg" alt=""></div>
      <div>
        <h3>Fast and more secure by default</h3>
        <p>A single compiled Rust binary behind the scenes — no PHP runtime, no plugin soup, no endless security patches to keep up with.</p>
        <a href="/admin/whats-different" target="_blank" rel="noopener">Read more</a>
      </div>
    </div>
    <div class="welcome-panel-card">
      <div class="welcome-panel-icon"><img src="/admin/static/icons/layout.svg" alt=""></div>
      <div>
        <h3>Build pages visually</h3>
        <p>Drag-and-drop sections with the built-in page builder — no separate plugin to install or keep updated.</p>
        <a href="/admin/builder">Open the Builder</a>
      </div>
    </div>
"##;

    let tail = r##"
  </div>
</div>
<style>
  .welcome-panel { position: relative; margin-bottom: 1rem; border-radius: var(--radius); overflow: hidden; box-shadow: var(--shadow); border: 1px solid var(--border); }
  .welcome-panel-dismiss { position: absolute; top: .75rem; right: .75rem; width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; background: none; border: 1px solid transparent; border-radius: var(--radius); cursor: pointer; z-index: 1; }
  .welcome-panel-dismiss:hover { background: var(--surface); }
  .welcome-panel-dismiss img { width: 16px; height: 16px; }
  .welcome-panel-hero { padding: 2rem 2rem 1.75rem; background: var(--tint); border-bottom: 1px solid var(--border); }
  .welcome-panel-hero h1 { margin: 0 0 .6rem; font-size: 1.9rem; font-weight: 700; color: var(--text); }
  .welcome-panel-hero a { color: var(--primary); text-decoration: underline; font-size: .95rem; }
  .welcome-panel-cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1.5rem; padding: 1.5rem 2rem; background: var(--surface); }
  .welcome-panel-card { display: flex; gap: .85rem; align-items: flex-start; }
  .welcome-panel-icon { flex-shrink: 0; width: 34px; height: 34px; display: flex; align-items: center; justify-content: center; background: var(--primary); border-radius: var(--radius); }
  .welcome-panel-icon img { width: 17px; height: 17px; filter: invert(1); }
  .welcome-panel-card h3 { margin: 0 0 .35rem; font-size: .95rem; font-weight: 600; color: var(--text); }
  .welcome-panel-card p { margin: 0 0 .4rem; font-size: .82rem; color: var(--muted); line-height: 1.4; }
  .welcome-panel-card a { font-size: .82rem; }
  @media (max-width: 900px) { .welcome-panel-cards { grid-template-columns: 1fr; } }
</style>
<script>
function dismissWelcomePanel() {
  var panel = document.getElementById('welcome-panel');
  if (panel) { panel.remove(); }
  fetch('/admin/dashboard/dismiss-welcome', { method: 'POST', credentials: 'same-origin' });
}
</script>"##;

    format!("{head}{third_card}{tail}")
}

pub fn render(data: &DashboardData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let is_author = ctx.user_role.eq_ignore_ascii_case("author");
    let mut widget_bodies: HashMap<&'static str, String> = HashMap::new();

    if is_author {
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

        widget_bodies.insert("posts_chart", format!(
            r#"<div style="display:flex;align-items:center;justify-content:flex-end;flex-wrap:wrap;row-gap:.4rem;margin-bottom:1rem">
  <div style="display:flex;align-items:center;gap:.5rem">
    {posts_year_sel}
    <div style="display:flex;gap:.35rem">
      <a href="/admin?range=week&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}"  class="{paw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
      <a href="/admin?range=month&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}" class="{pam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
      <a href="/admin?range=year&amp;views_range={vr}&amp;year={y}&amp;views_year={vy}"  class="{pay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
    </div>
  </div>
</div>
{chart_html}"#,
            posts_year_sel = posts_year_sel,
            vr = vr, y = y, vy = vy,
            paw = paw, pam = pam, pay = pay,
            chart_html = chart_html,
        ));

        widget_bodies.insert("post_views", format!(
            r#"<div style="display:flex;align-items:center;justify-content:flex-end;flex-wrap:wrap;row-gap:.4rem;margin-bottom:1rem">
  <div style="display:flex;align-items:center;gap:.5rem">
    {views_year_sel}
    <div style="display:flex;gap:.35rem">
      <a href="/admin?range={pr}&amp;views_range=week&amp;year={y}&amp;views_year={vy}"  class="{vaw}" style="font-size:12px;padding:.25rem .65rem">Week</a>
      <a href="/admin?range={pr}&amp;views_range=month&amp;year={y}&amp;views_year={vy}" class="{vam}" style="font-size:12px;padding:.25rem .65rem">Month</a>
      <a href="/admin?range={pr}&amp;views_range=year&amp;year={y}&amp;views_year={vy}"  class="{vay}" style="font-size:12px;padding:.25rem .65rem">Year</a>
    </div>
  </div>
</div>
{views_chart_html}"#,
            views_year_sel = views_year_sel,
            pr = pr, y = y, vy = vy,
            vaw = vaw, vam = vam, vay = vay,
            views_chart_html = views_chart_html,
        ));

        widget_bodies.insert("stats", format!(
            r#"<div class="stat-panel stat-panel-4 widget-stats" style="box-shadow:none;border:none;margin:-.4rem 0 -.7rem">
  <a href="/admin/posts?status=published" class="stat-cell stat-cell-link{published_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Posts</span></div>
    <div class="stat-num">{published}</div>
  </a>
  <a href="/admin/posts?status=draft" class="stat-cell stat-cell-link{drafts_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Drafts</span></div>
    <div class="stat-num">{drafts}</div>
  </a>
  {pending_open}
    <div class="stat-cell-top">
      <span class="stat-label">Pending</span>
    </div>
    <div class="stat-num">{pending}</div>
  {pending_close}
  <div class="stat-cell{views_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Views</span></div>
    <div class="stat-num">{total_views}</div>
  </div>
</div>"#,
            published         = data.author_published_posts,
            drafts            = data.author_draft_posts,
            pending           = data.author_pending_posts,
            total_views       = data.author_total_views,
            published_empty   = if data.author_published_posts == 0 { " is-empty" } else { "" },
            drafts_empty      = if data.author_draft_posts == 0 { " is-empty" } else { "" },
            views_empty       = if data.author_total_views == 0 { " is-empty" } else { "" },
            pending_open = if data.author_pending_posts > 0 {
                r#"<a href="/admin/posts?status=pending" class="stat-cell is-pending stat-cell-link" style="padding:.5rem 1.3rem">"#
            } else {
                r#"<div class="stat-cell is-empty" style="padding:.5rem 1.3rem">"#
            },
            pending_close = if data.author_pending_posts > 0 { "</a>" } else { "</div>" },
        ));
    }

    widget_bodies.insert("one", recent_posts_widget(
        &data.recent_drafts, "Drafts", "No drafts.",
        SecondColumn::Author, true,
    ));
    widget_bodies.insert("two", recent_posts_widget(
        &data.recent_published, "Published", "No published posts.",
        SecondColumn::Author, true,
    ));
    widget_bodies.insert("three", recent_posts_widget(
        &data.recent_pending, "Pending Review", "No posts pending review.",
        SecondColumn::Author, true,
    ));
    widget_bodies.insert("four", recent_posts_widget(
        &data.upcoming_scheduled, "Scheduled", "No posts scheduled.",
        SecondColumn::ScheduledTime, true,
    ));

    // Sites/Users/Subscribers widget — same data and links as the top stat panel's
    // last three cells, only shown to roles that can manage sites (super_admin,
    // site_admin); editors and authors have no use for site-wide counts.
    if ctx.can_manage_sites {
        widget_bodies.insert("five", format!(
            r#"<div class="stat-panel stat-panel-3 widget-stats" style="box-shadow:none;border:none;margin:-.4rem 0 -.7rem">
  <a href="/admin/sites" class="stat-cell stat-cell-link{sites_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Sites</span></div>
    <div class="stat-num">{total_sites}</div>
  </a>
  <a href="/admin/users" class="stat-cell stat-cell-link{users_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Users</span></div>
    <div class="stat-num">{total_users}</div>
  </a>
  <a href="/admin/users?tab=subscribers" class="stat-cell stat-cell-link{subscribers_empty}" style="padding:.5rem 1.3rem">
    <div class="stat-cell-top"><span class="stat-label">Subscribers</span></div>
    <div class="stat-num">{total_subscribers}</div>
  </a>
</div>"#,
            total_sites        = data.total_sites,
            total_users        = data.total_users,
            total_subscribers  = data.total_subscribers,
            sites_empty        = if data.total_sites == 0 { " is-empty" } else { "" },
            users_empty        = if data.total_users == 0 { " is-empty" } else { "" },
            subscribers_empty  = if data.total_subscribers == 0 { " is-empty" } else { "" },
        ));
    }

    widget_bodies.insert("six", quick_tools_widget(ctx));

    let default_layout = if is_author {
        serde_json::json!({
            "left": ["stats", "six", "posts_chart", "post_views"], "middle": ["two", "one"], "right": ["three", "four"]
        })
    } else if ctx.can_manage_sites {
        serde_json::json!({
            "left": ["five", "six"], "middle": ["two", "three"], "right": ["one", "four"]
        })
    } else {
        serde_json::json!({
            "left": ["six"], "middle": ["two", "three"], "right": ["one", "four"]
        })
    };

    let welcome_panel = if data.show_welcome_panel { welcome_panel_html(data.total_posts_ever) } else { String::new() };
    let content = format!("{welcome_panel}{}", widgets_section(&data.widget_layout, &default_layout, &widget_bodies));

    crate::admin_page("Dashboard", "/admin", flash, &content, ctx)
}

/// Renders the draggable widget board: Published Posts / Post Views (authors) plus
/// the Drafts / Published / Pending Review / Scheduled widgets, arranged per
/// the user's saved layout (or `default_layout` if none saved yet).
fn widgets_section(
    layout: &Option<serde_json::Value>,
    default_layout: &serde_json::Value,
    bodies: &HashMap<&'static str, String>,
) -> String {
    // Start from the saved layout, or the default if the user has none yet.
    let mut layout = layout.as_ref().cloned().unwrap_or_else(|| default_layout.clone());

    // Any real widget (e.g. a newly-added one) that isn't referenced anywhere in
    // the saved layout gets prepended to the left column, so it doesn't just
    // vanish for users who saved a layout before it existed.
    let already_placed: std::collections::HashSet<String> = ["left", "middle", "right"]
        .iter()
        .flat_map(|col| layout.get(col).and_then(|v| v.as_array()).into_iter().flatten())
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    // Sorted for a deterministic order — `bodies` is a HashMap, whose iteration
    // order is randomized per-instance, which would otherwise reshuffle these
    // unplaced widgets on every page load.
    let mut unplaced_ids: Vec<&&str> = bodies.keys().filter(|id| !already_placed.contains(**id)).collect();
    unplaced_ids.sort();
    for id in unplaced_ids {
        if let Some(left) = layout.get_mut("left").and_then(|v| v.as_array_mut()) {
            left.insert(0, serde_json::Value::String(id.to_string()));
        }
    }

    // Widgets in this list get "boxed" card chrome: a grey header bar (drag handle
    // + title) spanning the full card width, matching the Menus list table's
    // grey-header/white-body look, instead of the plain white title + padded body
    // every other widget uses.
    let boxed_titles: HashMap<&'static str, &'static str> = [
        ("one", "Drafts"),
        ("two", "Published"),
        ("three", "Pending Review"),
        ("four", "Scheduled"),
        ("five", "Overview"),
        ("six", "Quick Tools"),
        ("stats", "Overview"),
        ("posts_chart", "Published Posts"),
        ("post_views", "Post Views"),
    ].into_iter().collect();

    let col_html = |col: &str| -> String {
        layout.get(col)
            .and_then(|v| v.as_array())
            .map(|ids| {
                ids.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|id| {
                        let body = bodies.get(id).cloned()?;
                        Some(if let Some(title) = boxed_titles.get(id) {
                            format!(
                                r#"<div class="widget-card widget-card-boxed" draggable="true" data-widget="{id}">
      <div class="widget-card-header">
        <span class="widget-drag-handle">&#x2630;</span>
        <h3>{title}</h3>
      </div>
      <div class="widget-body">{body}</div>
    </div>"#,
                                id = id, title = title, body = body,
                            )
                        } else {
                            format!(
                                r#"<div class="widget-card" draggable="true" data-widget="{id}">
      <div class="widget-drag-handle">&#x2630;</div>
      <div class="widget-body">{body}</div>
    </div>"#,
                                id = id, body = body,
                            )
                        })
                    })
                    .collect::<Vec<_>>()
                    .join("\n    ")
            })
            .unwrap_or_default()
    };

    format!(
        r#"<div class="widget-board" id="widget-board">
  <div class="widget-col" data-col="left">
    {left}
  </div>
  <div class="widget-col" data-col="middle">
    {middle}
  </div>
  <div class="widget-col" data-col="right">
    {right}
  </div>
</div>
<style>
  .widget-board {{ display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; align-items: start; }}
  .widget-col {{ display: flex; flex-direction: column; gap: 1rem; min-height: 4rem; min-width: 0; }}
  .widget-body svg {{ max-width: 100%; height: auto; display: block; }}
  .widget-col.col-drag-over {{ outline: 2px dashed var(--primary); outline-offset: 4px; border-radius: var(--radius); }}
  .widget-card {{
    display: flex; align-items: flex-start; gap: .6rem;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius);
    box-shadow: var(--shadow); padding: .72rem 1rem 1rem; user-select: none;
    min-width: 0;
  }}
  .widget-card h3 {{ margin: 0; font-size: .95rem; font-weight: 600; line-height: 1.2; }}
  .widget-body {{ flex: 1; min-width: 0; }}
  .widget-drag-handle {{ display: block; flex-shrink: 0; padding: 0; margin-top: .04rem; color: var(--muted); font-size: .9rem; line-height: 1; cursor: grab; }}
  /* Boxed widgets: grey header bar (drag handle + title) flush to the card edges,
     matching the Menus list table's grey-header/white-body look. Height here
     is a standalone copy of .card-boxed-header's own height fix (same
     .48rem padding + line-height:1.2 on the title) rather than sharing the
     selector, so this widget-specific header can keep evolving on its own —
     see the .card-boxed-header comment in admin.css for the full reasoning
     behind matching .data-table th's rendered height. */
  .widget-card-boxed {{ flex-direction: column; align-items: stretch; padding: 0; overflow: hidden; }}
  .widget-card-header {{
    display: flex; align-items: center; gap: .5rem;
    background: var(--tint); padding: .48rem .8rem; border-bottom: 1px solid var(--border);
  }}
  .widget-card-boxed .widget-body {{ padding: .72rem 1rem 1rem; }}
  .widget-card.dragging .widget-drag-handle {{ cursor: grabbing; }}
  .widget-card.dragging {{ opacity: .4; }}
  .widget-card.drag-over {{ border-top: 2px solid var(--primary); }}
  .widget-stats.stat-panel-3 {{ grid-template-columns: repeat(3, 1fr); }}
  .widget-stats.stat-panel-4 {{ grid-template-columns: repeat(4, 1fr); }}
  .widget-stats .stat-cell {{ padding: .7rem .4rem; }}
  .widget-stats .stat-label {{ font-size: .74rem; white-space: nowrap; }}
  .widget-stats .stat-num {{ font-size: 1.6rem; }}
  @media (max-width: 1200px) {{
    .widget-board {{ grid-template-columns: 1fr; }}
  }}
</style>
<script>
(function() {{
  const board = document.getElementById('widget-board');
  if (!board) return;
  let dragged = null;

  function persistLayout() {{
    const layout = {{}};
    board.querySelectorAll('.widget-col').forEach((col) => {{
      layout[col.dataset.col] = Array.from(col.querySelectorAll('.widget-card'))
        .map((card) => card.dataset.widget);
    }});
    fetch('/admin/dashboard/widget-layout', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify(layout),
    }}).catch((err) => console.error('widget layout save failed', err));
  }}

  board.addEventListener('dragstart', (e) => {{
    const card = e.target.closest('.widget-card');
    if (!card) return;
    dragged = card;
    card.classList.add('dragging');
    e.dataTransfer.effectAllowed = 'move';
  }});

  board.addEventListener('dragend', () => {{
    if (dragged) dragged.classList.remove('dragging');
    board.querySelectorAll('.widget-card').forEach(c => c.classList.remove('drag-over'));
    board.querySelectorAll('.widget-col').forEach(c => c.classList.remove('col-drag-over'));
    dragged = null;
  }});

  board.addEventListener('dragover', (e) => {{
    e.preventDefault();
    if (!dragged) return;
    board.querySelectorAll('.widget-card').forEach(c => c.classList.remove('drag-over'));
    board.querySelectorAll('.widget-col').forEach(c => c.classList.remove('col-drag-over'));

    const card = e.target.closest('.widget-card');
    if (card && card !== dragged) {{
      card.classList.add('drag-over');
      return;
    }}
    const colEl = e.target.closest('.widget-col');
    if (colEl) colEl.classList.add('col-drag-over');
  }});

  board.addEventListener('drop', (e) => {{
    e.preventDefault();
    if (!dragged) return;

    const card = e.target.closest('.widget-card');
    if (card && card !== dragged) {{
      const rect = card.getBoundingClientRect();
      const before = (e.clientY - rect.top) < rect.height / 2;
      card.parentElement.insertBefore(dragged, before ? card : card.nextSibling);
    }} else {{
      const colEl = e.target.closest('.widget-col');
      if (colEl) colEl.appendChild(dragged);
    }}
    board.querySelectorAll('.widget-card').forEach(c => c.classList.remove('drag-over'));
    board.querySelectorAll('.widget-col').forEach(c => c.classList.remove('col-drag-over'));
    persistLayout();
  }});
}})();
</script>"#,
        left = col_html("left"),
        middle = col_html("middle"),
        right = col_html("right"),
    )
}
