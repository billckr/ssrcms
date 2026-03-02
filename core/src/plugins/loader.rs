//! Plugin loader: scans the plugins directory, parses manifests, registers hooks,
//! and adds plugin templates to the Tera engine.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tera::Tera;
use tracing::{info, warn};
use uuid::Uuid;

use super::hook_registry::{HookHandler, HookRegistry};
use super::manifest::PluginManifest;

/// Loaded plugin state — includes source provenance for multi-site support.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub directory: PathBuf,
    /// "global" for agency-managed plugins, "site" for per-site copies.
    pub source: String,
    /// Present when `source == "site"` — identifies which site owns this copy.
    pub site_id: Option<Uuid>,
}

/// Plugin loader: discovers and loads plugins from the plugins directory.
pub struct PluginLoader {
    plugins_dir: PathBuf,
    pub loaded: Vec<LoadedPlugin>,
    pub hook_registry: HookRegistry,
}

impl PluginLoader {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        PluginLoader {
            plugins_dir: plugins_dir.into(),
            loaded: Vec::new(),
            hook_registry: HookRegistry::new(),
        }
    }

    /// Scan the plugins directory, load manifests, and register hooks + templates into Tera.
    pub fn load_all(&mut self, tera: &mut Tera) -> anyhow::Result<()> {
        if !self.plugins_dir.exists() {
            info!("plugins directory {:?} not found — no plugins loaded", self.plugins_dir);
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.plugins_dir)?;

        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                warn!("skipping {:?}: no plugin.toml found", plugin_dir);
                continue;
            }

            match self.load_plugin(&plugin_dir, &manifest_path, tera) {
                Ok(plugin) => {
                    info!(
                        "loaded plugin '{}' v{}",
                        plugin.manifest.plugin.name, plugin.manifest.plugin.version
                    );
                    self.loaded.push(plugin);
                }
                Err(e) => {
                    warn!("failed to load plugin at {:?}: {}", plugin_dir, e);
                }
            }
        }

        Ok(())
    }

    fn load_plugin(
        &self,
        plugin_dir: &Path,
        manifest_path: &Path,
        tera: &mut Tera,
    ) -> anyhow::Result<LoadedPlugin> {
        let manifest = PluginManifest::from_file(manifest_path)?;

        // Add plugin templates to Tera.
        // Template names are relative paths within the plugin directory's template tree.
        let templates_glob = plugin_dir.join("**/*.html");
        let glob_str = templates_glob
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 path"))?;

        // We need to map plugin template paths to names that Tera can refer to.
        // Convention: templates in plugins/seo/seo/meta.html are registered as "seo/meta.html".
        for path in glob::glob(glob_str)?.flatten() {
            let rel = path.strip_prefix(plugin_dir)?;
            let template_name = rel.to_str().ok_or_else(|| anyhow::anyhow!("non-UTF-8 path"))?;
            // Convert Windows path separators
            let template_name = template_name.replace('\\', "/");
            let source = std::fs::read_to_string(&path)?;
            tera.add_raw_template(&template_name, &source)?;
        }

        // Register hooks from the manifest.
        for (hook_name, template_path) in &manifest.hooks {
            self.hook_registry.register(
                hook_name,
                HookHandler {
                    plugin_name: manifest.plugin.name.clone(),
                    template_path: template_path.clone(),
                },
            );
        }

        Ok(LoadedPlugin {
            manifest,
            directory: plugin_dir.to_path_buf(),
            source: "global".to_string(),
            site_id: None,
        })
    }

    /// Reload all plugin templates (dev mode hot reload).
    pub fn reload(&mut self, tera: &mut Tera) -> anyhow::Result<()> {
        // Unregister all existing plugin hooks.
        for plugin in &self.loaded {
            self.hook_registry.unregister_plugin(&plugin.manifest.plugin.name);
        }
        self.loaded.clear();
        self.load_all(tera)
    }
}
