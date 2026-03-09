use crate::{html_escape, admin_page, PageContext};

pub struct DocEntry {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub last_updated: String,
    pub updated_by: Option<String>,
}

pub fn render_list(entries: &[DocEntry], flash: Option<&str>, ctx: &PageContext) -> String {
    if entries.is_empty() {
        let content = r#"<div class="card" style="padding:2rem;text-align:center;color:var(--muted)">
  <p>No documentation yet.</p>
  <p style="font-size:13px;margin-top:.5rem">Run <code>/document-changes</code> in Claude Code to generate docs from the current codebase.</p>
</div>"#;
        return admin_page("Documentation", "/admin/documentation", flash, content, ctx);
    }

    let nav_items: String = entries
        .iter()
        .map(|e| {
            format!(
                r##"<li><a href="#doc-{slug}">{title}</a></li>"##,
                slug = html_escape(&e.slug),
                title = html_escape(&e.title),
            )
        })
        .collect();

    let doc_sections: String = entries
        .iter()
        .map(|e| {
            let by = e
                .updated_by
                .as_deref()
                .map(|b| format!(" &middot; {}", html_escape(b)))
                .unwrap_or_default();
            // Render markdown content as preformatted — a full markdown renderer
            // can be added later; for now wrap in <pre> inside a card.
            format!(
                r##"<div class="card doc-section" id="doc-{slug}" style="margin-bottom:2rem">
  <div class="doc-section-header" style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:1rem;padding-bottom:.75rem;border-bottom:1px solid var(--border)">
    <h2 style="margin:0;font-size:1.15rem">{title}</h2>
    <span style="font-size:11px;color:var(--muted)">Updated {updated}{by}</span>
  </div>
  <pre class="doc-content" style="white-space:pre-wrap;font-family:inherit;font-size:13.5px;line-height:1.7;margin:0;overflow-x:auto">{content}</pre>
</div>"##,
                slug = html_escape(&e.slug),
                title = html_escape(&e.title),
                updated = html_escape(&e.last_updated),
                by = by,
                content = html_escape(&e.content),
            )
        })
        .collect();

    let content = format!(
        r#"<div style="display:grid;grid-template-columns:200px 1fr;gap:2rem;align-items:start">
  <nav class="card" style="position:sticky;top:1.5rem;padding:1rem">
    <p style="font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted);margin:0 0 .75rem">Sections</p>
    <ul style="list-style:none;padding:0;margin:0;display:flex;flex-direction:column;gap:.35rem">
      {nav_items}
    </ul>
  </nav>
  <div>{doc_sections}</div>
</div>"#
    );

    admin_page("Documentation", "/admin/documentation", flash, &content, ctx)
}
