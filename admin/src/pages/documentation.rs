use pulldown_cmark::{html as cm_html, Options, Parser};

use crate::{html_escape, admin_page, PageContext};

/// Render a markdown string to an HTML string.
fn render_markdown(md: &str) -> String {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(md, opts);
    let mut html = String::new();
    cm_html::push_html(&mut html, parser);
    html
}

pub struct DocEntry {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub last_updated: String,
    pub updated_by: Option<String>,
    pub grp: String,
}

fn render_nav_group(label: &str, entries: &[&DocEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let items: String = entries
        .iter()
        .map(|e| format!(
            r##"<li><a href="#doc-{slug}" style="font-size:13px;color:inherit;text-decoration:none;padding:.2rem 0;display:block">{title}</a></li>"##,
            slug = html_escape(&e.slug),
            title = html_escape(&e.title),
        ))
        .collect();
    format!(
        r#"<div style="margin-bottom:1rem">
  <p style="font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:.07em;color:var(--muted);margin:0 0 .4rem">{label}</p>
  <ul style="list-style:none;padding:0;margin:0;display:flex;flex-direction:column">{items}</ul>
</div>"#,
        label = label,
        items = items,
    )
}

fn render_doc_section(e: &DocEntry) -> String {
    let by = e.updated_by.as_deref()
        .map(|b| format!(" &middot; {}", html_escape(b)))
        .unwrap_or_default();
    format!(
        r##"<div class="card doc-section" id="doc-{slug}" style="margin-bottom:2rem;padding:1.25rem">
  <div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:1rem;padding-bottom:.75rem;border-bottom:1px solid var(--border)">
    <h2 style="margin:0;font-size:1.1rem">{title}</h2>
    <span style="font-size:11px;color:var(--muted);white-space:nowrap;margin-left:1rem">Updated {updated}{by}</span>
  </div>
  <div class="doc-content">{content}</div>
</div>"##,
        slug    = html_escape(&e.slug),
        title   = html_escape(&e.title),
        updated = html_escape(&e.last_updated),
        by      = by,
        content = render_markdown(&e.content),
    )
}

pub fn render_list(entries: &[DocEntry], flash: Option<&str>, ctx: &PageContext) -> String {
    if entries.is_empty() {
        let content = r#"<div class="card" style="padding:2rem;text-align:center;color:var(--muted)">
  <p>No documentation yet.</p>
  <p style="font-size:13px;margin-top:.5rem">Run <code>/document-changes --all</code> in Claude Code to generate docs from the current codebase.</p>
</div>"#;
        return admin_page("Documentation", "/admin/documentation", flash, content, ctx);
    }

    let system_entries: Vec<&DocEntry> = entries.iter().filter(|e| e.grp == "system").collect();
    let feature_entries: Vec<&DocEntry> = entries.iter().filter(|e| e.grp == "feature").collect();
    let other_entries: Vec<&DocEntry> = entries.iter()
        .filter(|e| e.grp != "system" && e.grp != "feature")
        .collect();

    let nav = format!(
        r#"<nav class="card" style="position:sticky;top:1.5rem;padding:1rem;min-width:160px">
  {system_nav}
  {feature_nav}
  {other_nav}
</nav>"#,
        system_nav  = render_nav_group("System", &system_entries),
        feature_nav = render_nav_group("Features", &feature_entries),
        other_nav   = render_nav_group("Other", &other_entries),
    );

    let system_sections: String = system_entries.iter().map(|e| render_doc_section(e)).collect();
    let feature_sections: String = feature_entries.iter().map(|e| render_doc_section(e)).collect();
    let other_sections: String = other_entries.iter().map(|e| render_doc_section(e)).collect();

    let group_header = |label: &str| format!(
        r#"<h2 style="font-size:.8rem;font-weight:700;text-transform:uppercase;letter-spacing:.08em;color:var(--muted);margin:0 0 1rem;padding-bottom:.5rem;border-bottom:2px solid var(--border)">{label}</h2>"#,
        label = label
    );

    let mut sections = String::new();
    if !system_sections.is_empty() {
        sections.push_str(&group_header("System"));
        sections.push_str(&system_sections);
    }
    if !feature_sections.is_empty() {
        sections.push_str(&group_header("Features"));
        sections.push_str(&feature_sections);
    }
    if !other_sections.is_empty() {
        sections.push_str(&group_header("Other"));
        sections.push_str(&other_sections);
    }

    let content = format!(
        r#"<div style="display:grid;grid-template-columns:180px 1fr;gap:2rem;align-items:start">
  {nav}
  <div>{sections}</div>
</div>"#,
        nav      = nav,
        sections = sections,
    );

    admin_page("Documentation", "/admin/documentation", flash, &content, ctx)
}
