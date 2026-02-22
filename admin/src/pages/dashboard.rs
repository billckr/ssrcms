//! Admin dashboard page.

pub struct DashboardData {
    pub published_posts: i64,
    pub draft_posts: i64,
    pub total_pages: i64,
    pub total_users: i64,
    pub recent_posts: Vec<RecentPost>,
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
              <td>
                <a href="/admin/posts/{id}/edit">Edit</a>
              </td>
            </tr>"#,
            id = crate::html_escape(&p.id),
            title = crate::html_escape(&p.title),
            status = crate::html_escape(&p.status),
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-num">{}</div>
    <div class="stat-label">Published Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{}</div>
    <div class="stat-label">Draft Posts</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{}</div>
    <div class="stat-label">Pages</div>
  </div>
  <div class="stat-card">
    <div class="stat-num">{}</div>
    <div class="stat-label">Users</div>
  </div>
</div>
<h2>Recent Posts</h2>
<table class="data-table">
  <thead><tr><th>Title</th><th>Status</th><th>Actions</th></tr></thead>
  <tbody>{}</tbody>
</table>
<p><a href="/admin/posts/new" class="btn btn-primary">New Post</a></p>"#,
        data.published_posts, data.draft_posts, data.total_pages, data.total_users,
        recent_rows,
    );

    crate::admin_page("Dashboard", "/admin", flash, &content)
}
