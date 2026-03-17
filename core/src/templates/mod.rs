//! Tera template engine integration: loader, filters, functions, context builder.

pub mod composer;
pub mod context;
pub mod filters;
pub mod functions;
pub mod loader;

pub use loader::TemplateEngine;
