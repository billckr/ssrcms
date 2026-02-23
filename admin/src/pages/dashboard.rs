//! Admin dashboard page.

pub struct DashboardData {
    pub published_posts: i64,
    pub draft_posts: i64,
    pub total_pages: i64,
    pub total_users: i64,
    pub recent_posts: Vec<RecentPost>,
    pub current_site_name: String,
}

pub struct RecentPost {
    pub id: String,
    pub title: String,
    pub status: String,
    pub slug: String,
}

pub fn render(data: &DashboardData, flash: Option<&str>) -> String {
    let recent_rows = data.recent_posts.iter().map(|p| {
        format!(
            r#"<tr>
              <td><a href="/admin/posts/{id}/edit">{title}</a></td>
              <td><span class="badge badge-{status}">{status}</span></td>
              <td class="actions">
                <a href="/admin/posts/{id}/edit" class="icon-btn" title="Edit">
                  <img src="/admin/static/icons/edit.svg" alt="Edit">
                </a>
              </td>
            </tr>"#,
            id = crate::html_escape(&p.id),
            title = crate::html_escape(&p.title),
            status = crate::html_escape(&p.status),
        )
    }).collect::<Vec<_>>().join("\n");

    let site_banner = if !data.current_site_name.is_empty() {
        format!(
            r#"<div class="site-banner">Current site: <strong>{}</strong> &mdash; <a href="/admin/sites">Switch site</a></div>"#,
            crate::html_escape(&data.current_site_name)
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{site_banner}<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-num">{published_posts}</div>
    <div class="stat-label">Published Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{draft_posts}</div>
    <div class="stat-label">Draft Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{total_pages}</div>
    <div class="stat-label">Pages</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{total_users}</div>
    <div class="stat-label">Users</div>
  </div>
</div>
<h2 style="margin-bottom:.75rem">Recent Posts</h2>
<p style="margin-bottom:1rem"><a href="/admin/posts/new" class="btn btn-primary">New Post</a></p>
<table class="data-table">
  <thead><tr><th>Title</th><th>Status</th><th>Actions</th></tr></thead>
  <tbody>{recent_rows}</tbody>
</table>"#,
        site_banner = site_banner,
        published_posts = data.published_posts,
        draft_posts = data.draft_posts,
        total_pages = data.total_pages,
        total_users = data.total_users,
        recent_rows = recent_rows,
    );

    crate::admin_page("Dashboard", "/admin", flash, &content)
}
