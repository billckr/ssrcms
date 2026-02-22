//! Hook registry: maps hook names to ordered lists of template partials.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A registered hook handler — a template partial path.
#[derive(Debug, Clone)]
pub struct HookHandler {
    /// Plugin name (for ordering and debugging).
    pub plugin_name: String,
    /// Template path relative to the Tera instance (e.g. "seo/meta.html").
    pub template_path: String,
}

/// Thread-safe registry of hook → [handlers].
/// Handlers within a hook fire in registration order (alphabetical by plugin name).
#[derive(Debug, Clone, Default)]
pub struct HookRegistry {
    inner: Arc<RwLock<HashMap<String, Vec<HookHandler>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        HookRegistry::default()
    }

    /// Register a handler for a named hook point.
    pub fn register(&self, hook_name: &str, handler: HookHandler) {
        let mut map = self.inner.write().unwrap();
        map.entry(hook_name.to_string())
            .or_default()
            .push(handler);
    }

    /// Get all handlers registered for a hook, sorted by plugin_name.
    pub fn handlers_for(&self, hook_name: &str) -> Vec<HookHandler> {
        let map = self.inner.read().unwrap();
        let mut handlers = map.get(hook_name).cloned().unwrap_or_default();
        handlers.sort_by(|a, b| a.plugin_name.cmp(&b.plugin_name));
        handlers
    }

    /// Remove all handlers registered by a given plugin.
    #[allow(dead_code)]
    pub fn unregister_plugin(&self, plugin_name: &str) {
        let mut map = self.inner.write().unwrap();
        for handlers in map.values_mut() {
            handlers.retain(|h| h.plugin_name != plugin_name);
        }
    }

    /// List all hook names that have at least one handler.
    #[allow(dead_code)]
    pub fn active_hooks(&self) -> Vec<String> {
        let map = self.inner.read().unwrap();
        map.keys().cloned().collect()
    }
}

/// Well-known hook names. The list is open — plugins may define their own.
#[allow(dead_code)]
pub mod hooks {
    pub const HEAD_START: &str = "head_start";
    pub const HEAD_END: &str = "head_end";
    pub const BODY_START: &str = "body_start";
    pub const BODY_END: &str = "body_end";
    pub const BEFORE_CONTENT: &str = "before_content";
    pub const AFTER_CONTENT: &str = "after_content";
    pub const FOOTER: &str = "footer";
}
