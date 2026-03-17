//! Composition renderer: turns a saved `PageComposition` into full HTML.
//!
//! The algorithm:
//!   1. Parse the `composition` JSONB into `CompositionJson` (zone → block list).
//!   2. For each zone, render each block template (`blocks/{block_type}.html`)
//!      with a context that includes `block_config` alongside the standard site vars.
//!   3. Collect CSS for every unique block type used, inject as `builder_styles`
//!      into the layout context so the layout's `{% block head_extra %}` can place
//!      it in `<head>` — no theme file is touched.
//!   4. Render the layout shell (`layouts/{layout}.html`) with `zones` and
//!      `builder_styles` injected into the context.

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::errors::{AppError, Result};
use crate::models::page_composition::{BlockEntry, CompositionJson, PageComposition};
use crate::templates::TemplateEngine;

/// Render a full page from a saved `PageComposition`.
///
/// `base_ctx` is a fully-built `tera::Context` (site, request, session, nav already set).
/// The renderer adds `block_config`, `zones`, and `builder_styles` on top of it.
pub fn render_composition(
    composition: &PageComposition,
    engine: &TemplateEngine,
    site_id: Option<Uuid>,
    theme: &str,
    base_ctx: &tera::Context,
) -> Result<String> {
    let comp: CompositionJson = serde_json::from_value(composition.composition.clone())
        .unwrap_or_default();

    let zone_names = zones_for_layout(&composition.layout);

    // Render each zone and track which block types are used.
    let mut zones: HashMap<String, String> = HashMap::new();
    let mut used_block_types: HashSet<String> = HashSet::new();

    for zone_name in &zone_names {
        let blocks = comp.zones.get(*zone_name).cloned().unwrap_or_default();
        for b in &blocks {
            used_block_types.insert(b.block_type.clone());
        }
        let zone_html = render_zone(blocks, engine, site_id, theme, base_ctx)?;
        zones.insert(zone_name.to_string(), zone_html);
    }

    // Build the CSS for every block type used in this composition and wrap it
    // in a single <style> tag.  The layout template injects this into <head>
    // via {% block head_extra %}{{ builder_styles | safe }}{% endblock %}.
    let css_body: String = used_block_types
        .iter()
        .filter_map(|bt| block_css(bt))
        .collect::<Vec<_>>()
        .join("\n");

    let builder_styles = if css_body.is_empty() {
        String::new()
    } else {
        format!("<style>\n{}\n</style>", css_body)
    };

    let mut layout_ctx = base_ctx.clone();
    layout_ctx.insert("zones", &zones);
    layout_ctx.insert("builder_styles", &builder_styles);

    let template = format!("layouts/{}.html", &composition.layout);
    engine.render_for_theme(theme, site_id, &template, &layout_ctx)
}

/// Render all blocks in a single zone, returning concatenated HTML.
fn render_zone(
    blocks: Vec<BlockEntry>,
    engine: &TemplateEngine,
    site_id: Option<Uuid>,
    theme: &str,
    base_ctx: &tera::Context,
) -> Result<String> {
    let mut html = String::new();
    for block in blocks {
        match render_block(&block, engine, site_id, theme, base_ctx) {
            Ok(block_html) => html.push_str(&block_html),
            Err(e) => {
                let mut chain = format!("{e}");
                let mut src: &dyn std::error::Error = &e;
                while let Some(cause) = src.source() {
                    chain.push_str(&format!(" → {cause}"));
                    src = cause;
                }
                tracing::warn!(
                    "composer: failed to render block '{}': {}",
                    block.block_type,
                    chain
                );
                html.push_str(&format!(
                    r#"<div class="block-error" style="border:1px solid #f00;padding:.5rem;color:#f00">Block &ldquo;{}&rdquo; could not be rendered.</div>"#,
                    html_escape(&block.block_type)
                ));
            }
        }
    }
    Ok(html)
}

/// Render a single block using its template from the theme directory.
fn render_block(
    block: &BlockEntry,
    engine: &TemplateEngine,
    site_id: Option<Uuid>,
    theme: &str,
    base_ctx: &tera::Context,
) -> Result<String> {
    if !is_known_block_type(&block.block_type) {
        return Err(AppError::BadRequest(format!(
            "Unknown block type: {}",
            block.block_type
        )));
    }

    let mut block_ctx = base_ctx.clone();
    block_ctx.insert("block_config", &block.config);

    let template = format!("blocks/{}.html", &block.block_type);
    engine.render_for_theme(theme, site_id, &template, &block_ctx)
}

/// Zone names used by each layout shell.
pub fn zones_for_layout(layout: &str) -> Vec<&'static str> {
    match layout {
        "single-column" => vec!["header", "main", "footer"],
        "left-sidebar"  => vec!["header", "sidebar", "main", "footer"],
        "right-sidebar" => vec!["header", "main", "sidebar", "footer"],
        _               => vec!["header", "main", "footer"],
    }
}

/// Validate that a block_type maps to a known template (prevents path traversal).
pub fn is_known_block_type(block_type: &str) -> bool {
    matches!(
        block_type,
        "text-block" | "posts-grid" | "nav-menu" | "contact-form"
    )
}

/// CSS for each built-in block type.
///
/// This CSS is owned by the builder, not by any theme.  It is injected into
/// the page `<head>` only when the relevant block is present in the composition,
/// so themes are never polluted with builder-specific styles.
fn block_css(block_type: &str) -> Option<&'static str> {
    match block_type {
        "posts-grid" => Some(r#"
.posts-grid { display: grid; gap: 1.5rem; }
.posts-grid--cols-1 { grid-template-columns: 1fr; }
.posts-grid--cols-2 { grid-template-columns: repeat(2, 1fr); }
.posts-grid--cols-3 { grid-template-columns: repeat(3, 1fr); }
@media (max-width: 768px) {
  .posts-grid--cols-2, .posts-grid--cols-3 { grid-template-columns: 1fr; }
}
.posts-grid__item {
  border: 1px solid var(--border, #e5e7eb); border-radius: 8px; overflow: hidden;
  background: var(--surface, #fff); display: flex; flex-direction: column;
}
.posts-grid__thumb { width: 100%; aspect-ratio: 16/9; object-fit: cover; display: block; }
.posts-grid__body { padding: 1rem; flex: 1; display: flex; flex-direction: column; gap: .5rem; }
.posts-grid__title { font-size: 1.1rem; font-weight: 600; margin: 0; line-height: 1.3; }
.posts-grid__title a { color: var(--text, #111827); text-decoration: none; }
.posts-grid__title a:hover { text-decoration: underline; }
.posts-grid__excerpt { font-size: .9rem; color: var(--muted, #6b7280); margin: 0; flex: 1; }
.posts-grid__more { font-size: .85rem; font-weight: 500; color: var(--accent, #4f46e5); text-decoration: none; }
.posts-grid__more:hover { text-decoration: underline; }
.posts-grid__empty { grid-column: 1/-1; color: var(--muted, #6b7280); font-style: italic; text-align: center; padding: 2rem; }"#),

        "text-block" => Some(r#"
.block-text { line-height: 1.7; }
.block-text h1, .block-text h2, .block-text h3 { margin-top: 1.25em; margin-bottom: .5em; }
.block-text p { margin-bottom: 1em; }
.block-text a { color: var(--accent, #4f46e5); }
.block-text ul, .block-text ol { padding-left: 1.5em; margin-bottom: 1em; }"#),

        "nav-menu" => Some(r#"
.nav-menu { list-style: none; padding: 0; margin: 0; }
.nav-menu--horizontal { display: flex; flex-wrap: wrap; gap: .25rem; }
.nav-menu--vertical { display: flex; flex-direction: column; gap: .1rem; }
.nav-menu__item a { display: block; padding: .4rem .75rem; border-radius: 4px; color: var(--text, #111827); text-decoration: none; font-size: .9rem; }
.nav-menu__item a:hover { background: var(--surface-alt, #f3f4f6); }
.nav-menu__item--active > a { font-weight: 600; color: var(--accent, #4f46e5); }
.nav-menu__sub { list-style: none; padding: .25rem 0 .25rem 1rem; margin: 0; display: flex; flex-direction: column; gap: .1rem; }"#),

        "contact-form" => Some(r#"
.block-contact-form { max-width: 560px; }
.block-contact-form h2 { margin-bottom: 1rem; }
.contact-form__group { margin-bottom: 1rem; }
.contact-form__group label { display: block; font-size: .85rem; font-weight: 500; margin-bottom: .3rem; }
.contact-form__group input, .contact-form__group textarea {
  width: 100%; padding: .5rem .75rem; border: 1px solid var(--border, #d1d5db); border-radius: 6px;
  font-size: .95rem; font-family: inherit; background: var(--surface, #fff); color: var(--text, #111827); box-sizing: border-box;
}
.contact-form__group textarea { resize: vertical; min-height: 120px; }
.contact-form__submit { background: var(--accent, #4f46e5); color: #fff; border: none; padding: .55rem 1.25rem; border-radius: 6px; font-size: .95rem; cursor: pointer; }
.contact-form__submit:hover { opacity: .9; }"#),

        _ => None,
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
