//! Plugin system: manifest loading, hook registry, filter registry, plugin loader.

pub mod hook_registry;
pub mod loader;
pub mod manifest;

pub use hook_registry::HookRegistry;
pub use loader::PluginLoader;
