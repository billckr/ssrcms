use axum::{
    extract::{State, Form},
    response::{Html, Redirect},
};
use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::app_state::{AppState, set_site_setting};
use crate::middleware::admin_auth::AdminUser;
use admin::pages::appearance::{ThemeInfo, render};

pub async fn list(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    let themes_dir = &state.config.themes_dir;
    
    // Read the current active theme from the database (not from cache)
    let active_theme_from_db: String = sqlx::query_scalar("SELECT value FROM site_settings WHERE key = 'active_theme'")
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "default".to_string());

    let mut themes = Vec::new();

    // List all theme directories
    if let Ok(entries) = fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let theme_name = match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };

                // Read theme.toml
                let toml_path = path.join("theme.toml");
                if let Ok(toml_content) = fs::read_to_string(&toml_path) {
                    if let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) {
                        if let Some(theme_section) = parsed.get("theme").and_then(|v| v.as_table()) {
                            let name = theme_section.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&theme_name)
                                .to_string();
                            let version = theme_section.get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let description = theme_section.get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("No description")
                                .to_string();
                            let author = theme_section.get("author")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string();

                            themes.push(ThemeInfo {
                                name: name.clone(),
                                version,
                                description,
                                author,
                                active: name == active_theme_from_db,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by name, with active theme first
    themes.sort_by(|a, b| {
        match (a.active, b.active) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    Html(render(&themes))
}

#[derive(Deserialize)]
pub struct ActivateForm {
    pub theme: String,
}

pub async fn activate(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<ActivateForm>,
) -> Result<Redirect, String> {
    let themes_dir = &state.config.themes_dir;
    let theme_path = Path::new(themes_dir).join(&form.theme);

    // Validate theme exists
    if !theme_path.is_dir() {
        return Err("Theme not found".to_string());
    }

    // Update database
    if let Err(e) = set_site_setting(&state.db, "active_theme", &form.theme).await {
        return Err(format!("Failed to update theme: {}", e));
    }

    // Switch the template engine to the new theme immediately
    if let Err(e) = state.templates.switch_theme(&form.theme) {
        return Err(format!("Failed to load theme: {}", e));
    }

    // Update the shared active_theme so the static file handler serves the new theme's assets
    *state.active_theme.write().unwrap() = form.theme.clone();

    Ok(Redirect::to("/admin/appearance"))
}
