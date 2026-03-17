//! Builder composition renderer.
//!
//! Reads a saved Puck JSON composition and renders it to HTML by calling
//! the Tera block template for each block in `content`.
//!
//! Builder block templates live in `themes/builder/blocks/` and are loaded
//! into the theme engine under the key "__builder__" on first use.

use serde::Deserialize;

use crate::templates::loader::TemplateEngine;

#[derive(Debug, Deserialize)]
struct PuckBlock {
    #[serde(rename = "type")]
    block_type: String,
    props: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct PuckData {
    #[serde(default)]
    content: Vec<PuckBlock>,
}

pub struct ComposerError(pub String);

impl std::fmt::Display for ComposerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Render a saved Puck composition to a full HTML page.
pub fn render_composition(
    composition_json: &serde_json::Value,
    templates: &TemplateEngine,
    site_ctx: &tera::Context,
) -> Result<String, ComposerError> {
    let data: PuckData = serde_json::from_value(composition_json.clone())
        .map_err(|e| ComposerError(format!("invalid composition JSON: {e}")))?;

    if data.content.is_empty() {
        return Ok(empty_page_html());
    }

    // Collect per-block-type CSS
    let mut styles = String::new();
    let mut seen = std::collections::HashSet::new();
    for block in &data.content {
        if seen.insert(&block.block_type) {
            if let Some(css) = block_css(&block.block_type) {
                styles.push_str(css);
            }
        }
    }

    // Render each block via its Tera template
    let mut body_html = String::new();
    for block in &data.content {
        let template_name = format!("blocks/{}.html", block.block_type);
        let mut ctx = site_ctx.clone();
        ctx.insert("block_config", &block.props);

        match templates.render_builder_block(&template_name, &ctx) {
            Ok(html) => body_html.push_str(&html),
            Err(e) => {
                tracing::warn!("composer: block '{}' failed: {}", block.block_type, e);
                body_html.push_str(&format!(
                    r#"<!-- block '{}' could not be rendered -->"#,
                    block.block_type
                ));
            }
        }
    }

    let style_tag = if styles.is_empty() {
        String::new()
    } else {
        format!("<style>{}</style>", styles)
    };

    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  {style_tag}
</head>
<body style="margin:0;padding:0;font-family:system-ui,sans-serif">
{body_html}
</body>
</html>"#,
        style_tag = style_tag,
        body_html = body_html,
    ))
}

fn empty_page_html() -> String {
    r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body></body></html>"#.to_string()
}

fn block_css(block_type: &str) -> Option<&'static str> {
    match block_type {
        "Hero" => Some(r#"
@media (max-width: 768px) {
  .builder-hero { padding: 40px 20px !important; }
  .builder-hero h1 { font-size: 2rem !important; }
}
"#),
        _ => None,
    }
}
