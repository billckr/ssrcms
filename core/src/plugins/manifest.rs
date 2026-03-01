use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed representation of a `plugin.toml` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginInfo,

    /// Hook registrations: hook_name -> template path (relative to plugin dir)
    #[serde(default)]
    pub hooks: HashMap<String, String>,

    /// Custom meta fields declared by this plugin.
    #[serde(default)]
    pub meta_fields: HashMap<String, MetaFieldDef>,

    /// Plugin-registered HTTP routes.
    #[serde(default)]
    pub routes: HashMap<String, RouteRegistration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub description: String,
    pub author: String,
    /// "tera" (default) or "wasm" (future). Used for display badges and validation.
    #[serde(default = "default_plugin_type")]
    pub plugin_type: String,
}

fn default_plugin_type() -> String {
    "tera".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFieldDef {
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRegistration {
    pub template: String,
    pub content_type: String,
}

impl PluginManifest {
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&text)?;
        Ok(manifest)
    }
}
