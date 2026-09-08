//! Admin documentation viewer — displays docs written by the document-changes skill.
//!
//! Docs live as `.md` files under `AppConfig.documentation_dir` (one per
//! doc, filename is the slug) rather than in the database, so a fresh
//! install always has this reference material without a seed step, and the
//! content is versioned by git instead of being the only copy anywhere.

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::documentation::{render_list, DocEntry};

pub async fn list(State(state): State<AppState>, admin: AdminUser) -> impl IntoResponse {
    // Only super admins can view docs.
    if !admin.caps.is_global_admin {
        return (
            StatusCode::FORBIDDEN,
            Html("<h1>403 Forbidden</h1>".to_string()),
        )
            .into_response();
    }

    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let dir = std::path::Path::new(&state.config.documentation_dir);
    match std::fs::read_dir(dir) {
        Ok(read_dir) => {
            let mut entries: Vec<DocEntry> = Vec::new();
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let raw = match std::fs::read_to_string(&path) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("documentation: failed to read {:?}: {e}", path);
                        continue;
                    }
                };
                match parse_doc_frontmatter(&raw) {
                    Some((title, group, updated_by, last_updated, content)) => {
                        entries.push(DocEntry {
                            slug: slug.to_string(),
                            title,
                            content,
                            last_updated,
                            updated_by,
                            grp: group.unwrap_or_else(|| "feature".to_string()),
                        });
                    }
                    None => {
                        tracing::warn!("documentation: {:?} has no valid frontmatter", path);
                    }
                }
            }
            entries.sort_by(|a, b| {
                let rank = |g: &str| match g {
                    "system" => 0,
                    "feature" => 1,
                    _ => 2,
                };
                rank(&a.grp).cmp(&rank(&b.grp)).then(a.title.cmp(&b.title))
            });
            Html(render_list(&entries, None, &ctx)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to read documentation directory: {e}");
            let msg = "Failed to load documentation. The documentation/ directory may be missing.";
            Html(render_list(&[], Some(msg), &ctx)).into_response()
        }
    }
}

/// Splits a doc file's `---`-delimited frontmatter from its body and pulls
/// out `(title, group, updated_by, last_updated, body)`. Frontmatter is
/// deliberately not real YAML — just `key: value` lines — since the only
/// values ever stored are plain single-line strings with nothing that needs
/// escaping.
fn parse_doc_frontmatter(
    raw: &str,
) -> Option<(String, Option<String>, Option<String>, String, String)> {
    let rest = raw.strip_prefix("---\n")?;
    let (frontmatter, body) = rest.split_once("\n---\n")?;

    let mut title = None;
    let mut group = None;
    let mut updated_by = None;
    let mut last_updated = String::new();
    for line in frontmatter.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().to_string();
            match key.trim() {
                "title" => title = Some(value),
                "group" => group = Some(value),
                "updated_by" if !value.is_empty() => updated_by = Some(value),
                "last_updated" => last_updated = value,
                _ => {}
            }
        }
    }

    Some((title?, group, updated_by, last_updated, body.to_string()))
}
