//! Template engine: wraps Tera, registers filters/functions, loads themes and plugins.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use sqlx::PgPool;
use tera::Tera;
use tracing::info;

use crate::plugins::HookRegistry;
use crate::templates::filters;
use crate::templates::functions::{GetPostsFunction, GetTermsFunction, HookFunction, UrlForFunction};
use crate::errors::Result;

/// Thread-safe Tera template engine wrapper.
#[derive(Clone)]
pub struct TemplateEngine {
    inner: Arc<RwLock<Tera>>,
    themes_dir: PathBuf,
    /// Shared so switch_theme() and reload_theme() always agree on the current theme name.
    active_theme: Arc<RwLock<String>>,
    base_url: String,
    hook_registry: Arc<HookRegistry>,
    db: PgPool,
    /// Plugin templates registered via add_raw_template(), keyed by template name.
    /// Stored so switch_theme() can re-add them to the fresh Tera instance.
    plugin_templates: Arc<RwLock<HashMap<String, String>>>,
}

impl TemplateEngine {
    /// Create and initialize a template engine for the given theme and base URL.
    pub fn new(
        themes_dir: impl Into<PathBuf>,
        active_theme: &str,
        base_url: &str,
        hook_registry: Arc<HookRegistry>,
        db: PgPool,
    ) -> anyhow::Result<Self> {
        let themes_dir = themes_dir.into();
        let theme_path = themes_dir.join(active_theme).join("templates");

        let glob = format!("{}/**/*.html", theme_path.display());
        let mut tera = Tera::new(&glob)
            .map_err(|e| anyhow::anyhow!("Failed to load theme '{}': {}", active_theme, e))?;

        // Auto-escape HTML and XML templates; raw strings from the DB are already sanitised
        // by ammonia before storage, but auto-escaping is a defence-in-depth measure.
        tera.autoescape_on(vec![".html", ".xml"]);

        let engine = TemplateEngine {
            inner: Arc::new(RwLock::new(tera)),
            themes_dir,
            active_theme: Arc::new(RwLock::new(active_theme.to_string())),
            base_url: base_url.to_string(),
            hook_registry,
            db,
            plugin_templates: Arc::new(RwLock::new(HashMap::new())),
        };

        engine.register_filters_and_functions()?;

        info!("template engine initialized with theme '{}'", active_theme);
        Ok(engine)
    }

    fn register_filters_and_functions(&self) -> anyhow::Result<()> {
        let mut tera = self.inner.write().unwrap();

        // ── Filters ──────────────────────────────────────────────────────────
        tera.register_filter("date_format", filters::date_format);
        tera.register_filter("excerpt", filters::excerpt);
        tera.register_filter("strip_html", filters::strip_html);
        tera.register_filter("reading_time", filters::reading_time);
        tera.register_filter("slugify", filters::slugify);
        tera.register_filter("truncate_words", filters::truncate_words);
        tera.register_filter("absolute_url", filters::absolute_url);

        // ── Functions ─────────────────────────────────────────────────────────
        tera.register_function(
            "hook",
            HookFunction {
                registry: self.hook_registry.clone(),
            },
        );

        tera.register_function("url_for", UrlForFunction { base_url: self.base_url.clone() });

        tera.register_function(
            "get_posts",
            GetPostsFunction {
                pool: self.db.clone(),
                base_url: self.base_url.clone(),
            },
        );

        tera.register_function(
            "get_terms",
            GetTermsFunction {
                pool: self.db.clone(),
                base_url: self.base_url.clone(),
            },
        );

        Ok(())
    }

    /// Add plugin templates to the engine (called by plugin loader).
    /// Templates are persisted internally so switch_theme() can re-add them after a theme swap.
    pub fn add_raw_template(&self, name: &str, source: &str) -> anyhow::Result<()> {
        let mut tera = self.inner.write().unwrap();
        tera.add_raw_template(name, source)?;
        drop(tera);
        self.plugin_templates.write().unwrap().insert(name.to_string(), source.to_string());
        Ok(())
    }

    /// Render a template with the given context, then resolve [[HOOK:...]] sentinels.
    pub fn render(&self, template_name: &str, context: &tera::Context) -> Result<String> {
        let tera = self.inner.read().unwrap();
        let rendered = tera.render(template_name, context)?;
        let resolved = Self::resolve_hook_sentinels(rendered, context);
        Ok(resolved)
    }

    /// Render a template by raw source string (used for plugin-registered routes).
    pub fn render_str(&self, source: &str, context: &tera::Context) -> Result<String> {
        let mut tera = self.inner.read().unwrap().clone();
        tera.add_raw_template("__inline__", source)?;
        let rendered = tera.render("__inline__", context)?;
        let resolved = Self::resolve_hook_sentinels(rendered, context);
        Ok(resolved)
    }

    /// Pre-render all hook outputs for the named hook points, given a context.
    /// Returns a map of hook_name → rendered HTML to be injected into the context.
    pub fn render_hooks(
        &self,
        hook_names: &[&str],
        context: &tera::Context,
    ) -> HashMap<String, String> {
        let tera = self.inner.read().unwrap();
        let mut outputs = HashMap::new();

        for hook_name in hook_names {
            let handlers = self.hook_registry.handlers_for(hook_name);
            let mut html = String::new();

            for handler in &handlers {
                match tera.render(&handler.template_path, context) {
                    Ok(output) => html.push_str(&output),
                    Err(e) => {
                        tracing::warn!(
                            "hook '{}' template '{}' render error: {}",
                            hook_name,
                            handler.template_path,
                            e
                        );
                    }
                }
            }

            outputs.insert(hook_name.to_string(), html);
        }

        outputs
    }

    /// Replace `[[HOOK:__hook_output__<name>]]` sentinels in rendered HTML
    /// with the pre-rendered hook HTML stored in the context.
    fn resolve_hook_sentinels(rendered: String, context: &tera::Context) -> String {
        let sentinel_re =
            regex_lite::Regex::new(r"\[\[HOOK:__hook_output__([^\]]+)\]\]").unwrap();

        let mut result = rendered.clone();
        for cap in sentinel_re.captures_iter(&rendered) {
            let full_match = &cap[0];
            let hook_name = &cap[1]; // e.g. "head_end"
            let ctx_key = format!("__hook_output__{}", hook_name);

            let replacement = context
                .get(&ctx_key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            result = result.replace(full_match, &replacement);
        }

        result
    }

    /// Hot-reload theme templates (dev mode).
    pub fn reload_theme(&self) -> anyhow::Result<()> {
        let active = self.active_theme.read().unwrap().clone();
        let theme_path = self.themes_dir.join(&active).join("templates");
        let glob = format!("{}/**/*.html", theme_path.display());

        let mut tera = self.inner.write().unwrap();
        *tera = Tera::new(&glob)?;
        drop(tera);

        self.register_filters_and_functions()?;
        info!("theme '{}' reloaded", active);
        Ok(())
    }

    /// Dynamically switch to a different theme and reload templates.
    /// Re-adds all plugin templates so hooks continue to work after the switch.
    pub fn switch_theme(&self, new_theme: &str) -> anyhow::Result<()> {
        let theme_path = self.themes_dir.join(new_theme).join("templates");
        let glob = format!("{}/**/*.html", theme_path.display());

        let mut tera = self.inner.write().unwrap();
        *tera = Tera::new(&glob)
            .map_err(|e| anyhow::anyhow!("Failed to load theme '{}': {}", new_theme, e))?;

        // Re-add plugin templates — they are not part of the theme glob.
        let plugin_templates = self.plugin_templates.read().unwrap();
        for (name, source) in plugin_templates.iter() {
            if let Err(e) = tera.add_raw_template(name, source) {
                tracing::warn!("switch_theme: could not re-add plugin template '{}': {}", name, e);
            }
        }
        drop(plugin_templates);
        drop(tera);

        *self.active_theme.write().unwrap() = new_theme.to_string();
        self.register_filters_and_functions()?;
        info!("switched to theme '{}'", new_theme);
        Ok(())
    }
}
