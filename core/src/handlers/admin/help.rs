//! End-user help viewer — the production-facing counterpart to
//! `documentation` (which is admin-only reference material stripped before
//! a production build). Docs live as `.md` files under `AppConfig.help_dir`
//! (one per doc, filename is the slug) and are reachable by every
//! authenticated admin user, not just global admins, since the audience is
//! app users rather than operators.

use axum::{
    extract::State,
    response::{Html, IntoResponse},
};

use crate::app_state::AppState;
use crate::middleware::admin_auth::AdminUser;
use admin::pages::help::{render_list, HelpEntry};

pub async fn list(State(state): State<AppState>, admin: AdminUser) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);
    let ctx = super::page_ctx_full(&state, &admin, &cs).await;

    let dir = std::path::Path::new(&state.config.help_dir);
    match std::fs::read_dir(dir) {
        Ok(read_dir) => {
            let mut entries: Vec<HelpEntry> = Vec::new();
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
                        tracing::warn!("help: failed to read {:?}: {e}", path);
                        continue;
                    }
                };
                match parse_help_frontmatter(&raw) {
                    Some((title, group, content)) => {
                        entries.push(HelpEntry {
                            slug: slug.to_string(),
                            title,
                            content,
                            grp: group.unwrap_or_else(|| "feature".to_string()),
                        });
                    }
                    None => {
                        tracing::warn!("help: {:?} has no valid frontmatter", path);
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
            tracing::error!("Failed to read help directory: {e}");
            let msg = "Failed to load help. The help/ directory may be missing.";
            Html(render_list(&[], Some(msg), &ctx)).into_response()
        }
    }
}

/// Splits a help file's `---`-delimited frontmatter from its body and pulls
/// out `(title, group, body)`. Frontmatter is deliberately not real YAML —
/// just `key: value` lines — matching the `documentation` convention.
fn parse_help_frontmatter(raw: &str) -> Option<(String, Option<String>, String)> {
    let rest = raw.strip_prefix("---\n")?;
    let (frontmatter, body) = rest.split_once("\n---\n")?;

    let mut title = None;
    let mut group = None;
    for line in frontmatter.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().to_string();
            match key.trim() {
                "title" => title = Some(value),
                "group" => group = Some(value),
                _ => {}
            }
        }
    }

    Some((title?, group, body.to_string()))
}
