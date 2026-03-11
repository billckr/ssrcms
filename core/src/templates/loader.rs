//! Template engine: wraps Tera, registers filters/functions, loads themes and plugins.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use sqlx::PgPool;
use tera::Tera;
use tracing::info;

use crate::plugins::HookRegistry;
use crate::templates::filters;
use crate::templates::functions::{GetMenuFunction, GetPostsFunction, GetTermsFunction, HookFunction, UrlForFunction};
use crate::errors::{AppError, Result};

/// Thread-safe Tera template engine wrapper.
///
/// Holds one Tera instance **per loaded theme** so multiple sites with different
/// themes can render concurrently without stomping on each other.
#[derive(Clone)]
pub struct TemplateEngine {
    /// Per-theme Tera instances: theme_name → Tera.
    engines: Arc<RwLock<HashMap<String, Tera>>>,
    /// Root of the themes directory tree (parent of `global/` and `sites/`).
    themes_root: PathBuf,
    /// Fallback theme name for legacy single-argument render() paths.
    active_theme: Arc<RwLock<String>>,
    base_url: String,
    hook_registry: Arc<HookRegistry>,
    db: PgPool,
    /// Plugin templates registered via add_raw_template(), keyed by template name.
    /// Stored so switch_theme() can re-add them when a fresh Tera instance is loaded.
    plugin_templates: Arc<RwLock<HashMap<String, String>>>,
}

impl TemplateEngine {
    /// Create and initialize a template engine for the given theme and base URL.
    ///
    /// `themes_root` is the parent of the `global/` and `sites/` subdirectories.
    pub fn new(
        themes_root: impl Into<PathBuf>,
        active_theme: &str,
        base_url: &str,
        hook_registry: Arc<HookRegistry>,
        db: PgPool,
    ) -> anyhow::Result<Self> {
        let themes_root = themes_root.into();

        let engine = TemplateEngine {
            engines: Arc::new(RwLock::new(HashMap::new())),
            themes_root,
            active_theme: Arc::new(RwLock::new(active_theme.to_string())),
            base_url: base_url.to_string(),
            hook_registry,
            db,
            plugin_templates: Arc::new(RwLock::new(HashMap::new())),
        };

        engine.load_theme(active_theme)?;

        info!("template engine initialized with theme '{}'", active_theme);
        Ok(engine)
    }

    /// Resolve the filesystem directory for a named theme, preferring a
    /// site-specific copy when `site_id` is provided.
    ///
    /// Resolution order with site_id:
    ///   1. `themes_root/sites/<site_id>/<name>/`
    ///   2. `themes_root/global/<name>/`
    ///
    /// Resolution order without site_id (or when site copy does not exist):
    ///   1. `themes_root/global/<name>/`
    ///   2. Any `themes_root/sites/*/<name>/`
    pub fn resolve_theme_dir(&self, name: &str) -> Option<PathBuf> {
        self.resolve_theme_dir_for_site(name, None)
    }

    /// Resolve the filesystem directory for a named theme for a specific site.
    pub fn resolve_theme_dir_for_site(&self, name: &str, site_id: Option<Uuid>) -> Option<PathBuf> {
        // Prefer the site-specific copy when a site_id is given.
        if let Some(sid) = site_id {
            let site_candidate = self.themes_root.join("sites").join(sid.to_string()).join(name);
            if site_candidate.is_dir() {
                return Some(site_candidate);
            }
        }
        // Fall back to global.
        let global_candidate = self.themes_root.join("global").join(name);
        if global_candidate.is_dir() {
            return Some(global_candidate);
        }
        // Check private (super_admin-only themes).
        let private_candidate = self.themes_root.join("private").join(name);
        if private_candidate.is_dir() {
            return Some(private_candidate);
        }
        // Last resort: any site copy (single-site installs without a site_id).
        let sites_dir = self.themes_root.join("sites");
        if let Ok(entries) = std::fs::read_dir(&sites_dir) {
            for entry in entries.flatten() {
                let candidate = entry.path().join(name);
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    /// Canonical cache key for a (theme_name, site_id) pair: the resolved
    /// absolute path of the theme directory. This means two sites with a
    /// theme of the same name but different on-disk copies get separate Tera
    /// instances.
    fn theme_cache_key(&self, name: &str, site_id: Option<Uuid>) -> Option<String> {
        self.resolve_theme_dir_for_site(name, site_id)
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string())
    }

    /// Load (or reload) a theme into the engine map.
    ///
    /// The map key is the canonical path of the theme directory so that two
    /// sites with the same theme name but different on-disk copies are kept
    /// separate.
    fn load_theme_for_site(&self, theme_name: &str, site_id: Option<Uuid>) -> anyhow::Result<()> {
        let theme_dir = self.resolve_theme_dir_for_site(theme_name, site_id)
            .ok_or_else(|| anyhow::anyhow!("Theme '{}' not found", theme_name))?;
        let cache_key = theme_dir.canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| theme_dir.to_string_lossy().to_string());

        let theme_path = theme_dir.join("templates");
        let glob = format!("{}/**/*.html", theme_path.display());

        let mut tera = Tera::new(&glob)
            .map_err(|e| anyhow::anyhow!("Failed to load theme '{}': {}", theme_name, e))?;

        // Also load XML templates (e.g. sitemap.xml) from the theme directory.
        Self::load_xml_from_dir(&mut tera, &theme_path, &theme_path);

        tera.autoescape_on(vec![".html", ".xml"]);

        // Re-add plugin templates.
        let plugin_templates = self.plugin_templates.read().unwrap();
        for (tname, source) in plugin_templates.iter() {
            if let Err(e) = tera.add_raw_template(tname, source) {
                tracing::warn!("load_theme: could not add plugin template '{}': {}", tname, e);
            }
        }
        drop(plugin_templates);

        self.register_on_tera(&mut tera);

        self.engines.write().unwrap().insert(cache_key.clone(), tera);
        info!("loaded theme '{}' from {}", theme_name, cache_key);
        Ok(())
    }

    /// Load (or reload) a theme without site context (legacy / global path).
    fn load_theme(&self, theme_name: &str) -> anyhow::Result<()> {
        self.load_theme_for_site(theme_name, None)
    }

    /// Ensure a theme is loaded for a given site, loading lazily if needed.
    fn ensure_theme_loaded_for_site(&self, theme_name: &str, site_id: Option<Uuid>) {
        if let Some(key) = self.theme_cache_key(theme_name, site_id) {
            if !self.engines.read().unwrap().contains_key(&key) {
                if let Err(e) = self.load_theme_for_site(theme_name, site_id) {
                    tracing::warn!("ensure_theme_loaded: could not load '{}': {}", theme_name, e);
                }
            }
        }
    }

    /// Ensure a theme is loaded into the engine map, loading lazily if needed.
    fn ensure_theme_loaded(&self, theme_name: &str) {
        self.ensure_theme_loaded_for_site(theme_name, None);
    }

    /// Remove a theme's Tera instance from the cache so it is reloaded on
    /// the next render. Call this after editing theme files on disk.
    pub fn invalidate_theme(&self, theme_name: &str, site_id: Option<Uuid>) {
        if let Some(key) = self.theme_cache_key(theme_name, site_id) {
            self.engines.write().unwrap().remove(&key);
            tracing::debug!("invalidated theme cache for '{}' (site={:?})", theme_name, site_id);
        }
    }

    fn register_on_tera(&self, tera: &mut Tera) {
        tera.register_filter("date_format", filters::date_format);
        tera.register_filter("excerpt", filters::excerpt);
        tera.register_filter("strip_html", filters::strip_html);
        tera.register_filter("reading_time", filters::reading_time);
        tera.register_filter("slugify", filters::slugify);
        tera.register_filter("truncate_words", filters::truncate_words);
        tera.register_filter("absolute_url", filters::absolute_url);

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
        tera.register_function(
            "get_menu",
            GetMenuFunction {
                pool: self.db.clone(),
            },
        );
    }

    /// Add plugin templates to every loaded theme engine, and persist them so
    /// future loads also receive the templates.
    pub fn add_raw_template(&self, name: &str, source: &str) -> anyhow::Result<()> {
        self.plugin_templates.write().unwrap().insert(name.to_string(), source.to_string());
        let mut engines = self.engines.write().unwrap();
        for tera in engines.values_mut() {
            if let Err(e) = tera.add_raw_template(name, source) {
                tracing::warn!("add_raw_template: could not add '{}': {}", name, e);
            }
        }
        Ok(())
    }

    /// Render a template using the specified theme and site context.
    ///
    /// Passing `site_id` ensures the site-specific copy of the theme is used
    /// when one exists (e.g. `themes/sites/<id>/default/`) in preference to
    /// the shared global copy (`themes/global/default/`).
    pub fn render_for_theme(&self, theme: &str, site_id: Option<Uuid>, template_name: &str, context: &tera::Context) -> Result<String> {
        self.ensure_theme_loaded_for_site(theme, site_id);
        let key = self.theme_cache_key(theme, site_id);
        let fallback_key = self.theme_cache_key(theme, None);
        let active = self.active_theme.read().unwrap().clone();
        let engines = self.engines.read().unwrap();
        let tera = key.as_deref().and_then(|k| engines.get(k))
            .or_else(|| fallback_key.as_deref().and_then(|k| engines.get(k)))
            .or_else(|| engines.get(&active))
            .ok_or_else(|| AppError::Internal("No theme engine available".to_string()))?;
        let rendered = tera.render(template_name, context)?;
        drop(engines);
        Ok(Self::resolve_hook_sentinels(rendered, context))
    }

    /// Render a template using the current `active_theme` (legacy / single-site path).
    pub fn render(&self, template_name: &str, context: &tera::Context) -> Result<String> {
        let theme = self.active_theme.read().unwrap().clone();
        self.render_for_theme(&theme, None, template_name, context)
    }

    /// Render a template by raw source string (used for plugin-registered routes).
    #[allow(dead_code)]
    pub fn render_str(&self, source: &str, context: &tera::Context) -> Result<String> {
        let active = self.active_theme.read().unwrap().clone();
        self.ensure_theme_loaded(&active);
        let engines = self.engines.read().unwrap();
        let mut tera = engines.get(&active)
            .ok_or_else(|| AppError::Internal("No theme engine available".to_string()))?
            .clone();
        drop(engines);
        tera.add_raw_template("__inline__", source)?;
        let rendered = tera.render("__inline__", context)?;
        Ok(Self::resolve_hook_sentinels(rendered, context))
    }

    /// Pre-render hook outputs using a specific theme's engine.
    ///
    /// `active_plugins`: when `Some`, only handlers whose `plugin_name` is in
    /// the list will fire — used for per-site plugin activation. Pass `None`
    /// to fire all registered handlers (backward-compatible default).
    pub fn render_hooks_for_theme(
        &self,
        theme: &str,
        site_id: Option<Uuid>,
        hook_names: &[&str],
        context: &tera::Context,
        active_plugins: Option<&[String]>,
    ) -> HashMap<String, String> {
        self.ensure_theme_loaded_for_site(theme, site_id);
        let key = self.theme_cache_key(theme, site_id);
        let fallback_key = self.theme_cache_key(theme, None);
        let active = self.active_theme.read().unwrap().clone();
        let engines = self.engines.read().unwrap();
        let tera = match key.as_deref().and_then(|k| engines.get(k))
            .or_else(|| fallback_key.as_deref().and_then(|k| engines.get(k)))
            .or_else(|| engines.get(&active))
        {
            Some(t) => t,
            None => return HashMap::new(),
        };

        let mut outputs = HashMap::new();
        for hook_name in hook_names {
            let mut handlers = self.hook_registry.handlers_for(hook_name);
            // Filter by active plugins when a list is provided.
            if let Some(active) = active_plugins {
                handlers.retain(|h| active.contains(&h.plugin_name));
            }
            let mut html = String::new();
            for handler in &handlers {
                match tera.render(&handler.template_path, context) {
                    Ok(output) => html.push_str(&output),
                    Err(e) => tracing::warn!(
                        "hook '{}' template '{}' render error: {}",
                        hook_name, handler.template_path, e
                    ),
                }
            }
            outputs.insert(hook_name.to_string(), html);
        }
        outputs
    }

    /// Pre-render hook outputs using the current `active_theme` (legacy path).
    pub fn render_hooks(
        &self,
        hook_names: &[&str],
        context: &tera::Context,
    ) -> HashMap<String, String> {
        let theme = self.active_theme.read().unwrap().clone();
        self.render_hooks_for_theme(&theme, None, hook_names, context, None)
    }

    /// Replace `[[HOOK:__hook_output__<name>]]` sentinels in rendered HTML
    /// with the pre-rendered hook HTML stored in the context.
    fn resolve_hook_sentinels(rendered: String, context: &tera::Context) -> String {
        let sentinel_re =
            regex_lite::Regex::new(r"\[\[HOOK:__hook_output__([^\]]+)\]\]").unwrap();

        let mut result = rendered.clone();
        for cap in sentinel_re.captures_iter(&rendered) {
            let full_match = &cap[0];
            let hook_name = &cap[1];
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

    /// Recursively load XML files from a directory into a Tera instance.
    /// Template names are relative to `base` (e.g. "sitemap.xml").
    fn load_xml_from_dir(tera: &mut Tera, dir: &std::path::Path, base: &std::path::Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::load_xml_from_dir(tera, &path, base);
            } else if path.extension().and_then(|e| e.to_str()) == Some("xml") {
                if let Ok(relative) = path.strip_prefix(base) {
                    let tname = relative.to_string_lossy().replace('\\', "/");
                    if let Ok(src) = std::fs::read_to_string(&path) {
                        if let Err(e) = tera.add_raw_template(&tname, &src) {
                            tracing::warn!(
                                "load_theme: could not add XML template '{}': {}",
                                tname,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Hot-reload the currently active theme's templates (dev mode).
    #[allow(dead_code)]
    pub fn reload_theme(&self) -> anyhow::Result<()> {
        let active = self.active_theme.read().unwrap().clone();
        self.load_theme(&active)?;
        info!("theme '{}' reloaded", active);
        Ok(())
    }

    /// Load a theme into the engine map and set it as the fallback active_theme.
    ///
    /// Does NOT remove other loaded themes — all sites keep their engines intact.
    pub fn switch_theme(&self, new_theme: &str) -> anyhow::Result<()> {
        self.load_theme(new_theme)?;
        *self.active_theme.write().unwrap() = new_theme.to_string();
        info!("switched active theme to '{}'", new_theme);
        Ok(())
    }
}


