use axum::{
    extract::{State, Form},
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::app_state::{AppState, set_site_setting};
use crate::middleware::admin_auth::AdminUser;
use admin::pages::appearance::{ThemeInfo, render_with_flash};

pub async fn list(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    render_appearance_list(&state, None).await
}

#[derive(Deserialize)]
pub struct ActivateForm {
    pub theme: String,
}

pub async fn activate(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<ActivateForm>,
) -> impl IntoResponse {
    let themes_dir = &state.config.themes_dir;
    let theme_path = Path::new(themes_dir).join(&form.theme);

    if !theme_path.is_dir() {
        tracing::warn!("theme activation failed: theme '{}' not found", form.theme);
        return render_appearance_list(&state, Some("Theme not found.")).await.into_response();
    }

    if let Err(e) = set_site_setting(&state.db, "active_theme", &form.theme).await {
        tracing::error!("failed to save active_theme to DB: {:?}", e);
        return render_appearance_list(&state, Some("Failed to activate theme. Please try again.")).await.into_response();
    }

    if let Err(e) = state.templates.switch_theme(&form.theme) {
        tracing::error!("failed to switch theme to '{}': {:?}", form.theme, e);
        return render_appearance_list(&state, Some("Theme files could not be loaded. Please try again.")).await.into_response();
    }

    *state.active_theme.write().unwrap() = form.theme.clone();

    Redirect::to("/admin/appearance").into_response()
}

async fn render_appearance_list(state: &AppState, flash: Option<&str>) -> Html<String> {
    let themes_dir = &state.config.themes_dir;

    let active_theme_from_db: String = sqlx::query_scalar("SELECT value FROM site_settings WHERE key = 'active_theme'")
        .fetch_optional(&state.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to read active_theme from DB: {:?}", e);
            None
        })
        .unwrap_or_else(|| "default".to_string());

    let mut themes = Vec::new();

    if let Ok(entries) = fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let theme_name = match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };
                let toml_path = path.join("theme.toml");
                if let Ok(toml_content) = fs::read_to_string(&toml_path) {
                    if let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) {
                        if let Some(theme_section) = parsed.get("theme").and_then(|v| v.as_table()) {
                            let name = theme_section.get("name").and_then(|v| v.as_str()).unwrap_or(&theme_name).to_string();
                            let version = theme_section.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                            let description = theme_section.get("description").and_then(|v| v.as_str()).unwrap_or("No description").to_string();
                            let author = theme_section.get("author").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                            themes.push(ThemeInfo { name: name.clone(), version, description, author, active: name == active_theme_from_db });
                        }
                    }
                }
            }
        }
    }

    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Html(render_with_flash(&themes, flash))
}
