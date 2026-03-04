//! Admin dashboard page.

pub struct DashboardData {
    pub published_posts: i64,
    pub draft_posts: i64,
    pub total_pages: i64,
    pub total_users: i64,
}

pub fn render(data: &DashboardData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = format!(
        r#"<div class="stats-grid">
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
</div>"#,
        published_posts = data.published_posts,
        draft_posts = data.draft_posts,
        total_pages = data.total_pages,
        total_users = data.total_users,
    );

    crate::admin_page("Dashboard", "/admin", flash, &content, ctx)
}
