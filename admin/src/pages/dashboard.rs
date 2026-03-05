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
}

pub fn render(data: &DashboardData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = if ctx.user_role.eq_ignore_ascii_case("author") {
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
</div>"#,
            published = data.author_published_posts,
            drafts    = data.author_draft_posts,
            pending   = data.author_pending_posts,
            pending_link = if data.author_pending_posts > 0 {
                r#"<a href="/admin/posts?status=pending" class="stat-action">View pending posts &rarr;</a>"#
            } else { "" },
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
