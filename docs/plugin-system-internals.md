# Synaptic Signals — Plugin System Internals

> **Audience:** Engineers (human or AI) debugging, extending, or reviewing the plugin system.
> **Last updated:** 2026-03-01

---

## Table of Contents

1. [Overview](#1-overview)
2. [Directory Structure](#2-directory-structure)
3. [Key Data Structures](#3-key-data-structures)
4. [Startup Sequence](#4-startup-sequence)
5. [Per-Request Hook Resolution](#5-per-request-hook-resolution)
6. [Per-Site Activation](#6-per-site-activation)
7. [Tera Template Registration](#7-tera-template-registration)
8. [Plugin Routes](#8-plugin-routes)
9. [Known Design Constraints](#9-known-design-constraints)
10. [Troubleshooting Runbook](#10-troubleshooting-runbook)
11. [Key File Reference](#11-key-file-reference)

---

## 1. Overview

Plugins are **Tera template partials** loaded at runtime from the `plugins/` directory. There is no compilation step. A plugin declares itself in `plugin.toml` and supplies `.html` or `.xml` template files. The core registers these templates into the Tera engine and wires them to named hook points. When a page request arrives, only the hooks for that site's active plugins fire.

The sandbox is structural: plugins can control presentation only. They cannot execute code, query the database, make network requests, or access the filesystem. The Tera engine is the security boundary.

---

## 2. Directory Structure

```
plugins/
├── global/                    ← agency-managed plugins (super_admin)
│   ├── seo/
│   │   ├── plugin.toml
│   │   └── seo/
│   │       ├── meta.html      ← template name: "seo/meta.html"
│   │       └── sitemap.xml    ← template name: "seo/sitemap.xml"
│   └── hello/
│       ├── plugin.toml
│       └── hello/
│           └── footer.html    ← template name: "hello/footer.html"
└── sites/
    └── <site-uuid>/           ← copies installed by site admin
        └── seo/
            ├── plugin.toml
            └── seo/
                ├── meta.html
                └── sitemap.xml
```

**Template naming rule:** The template name used in `plugin.toml` and registered in Tera is the **path relative to the plugin's root directory**. For a file at `plugins/global/seo/seo/meta.html`, the plugin root is `plugins/global/seo/`, so the template name is `seo/meta.html`.

This one-level subdirectory convention (`seo/meta.html` not just `meta.html`) namespaces templates to avoid collisions between plugins that happen to have files with the same base name.

---

## 3. Key Data Structures

### `PluginManifest` (`core/src/plugins/manifest.rs`)

Deserialized from `plugin.toml`. Contains:
- `plugin: PluginInfo` — name, version, api_version, description, author, plugin_type ("tera" or "wasm")
- `hooks: HashMap<String, String>` — hook_name → template_path (e.g. `"head_end" → "seo/meta.html"`)
- `routes: HashMap<String, RouteRegistration>` — path → `{template, content_type}` (e.g. `"/sitemap.xml"`)
- `meta_fields: HashMap<String, MetaFieldDef>` — custom post meta fields this plugin uses

### `LoadedPlugin` (`core/src/plugins/loader.rs`)

Runtime representation of a scanned plugin:
```rust
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub directory: PathBuf,
    pub source: String,       // "global" or "site"
    pub site_id: Option<Uuid>, // None for global; Some(id) for site copies
}
```
Stored in `AppState.loaded_plugins: Arc<Vec<LoadedPlugin>>`. Used by the admin plugin list handler to build the UI.

### `HookRegistry` / `HookHandler` (`core/src/plugins/hook_registry.rs`)

```rust
pub struct HookHandler {
    pub plugin_name: String,   // "seo"
    pub template_path: String, // "seo/meta.html"
}
```
The `HookRegistry` maps hook names to `Vec<HookHandler>`. Populated at startup by `load_plugins_into_engine`. Queried at render time by `render_hooks_for_theme`.

### `SitePlugin` (`core/src/models/site_plugin.rs`)

DB row tracking per-site plugin state:
```rust
pub struct SitePlugin {
    pub site_id: Uuid,
    pub plugin_name: String,
    pub active: bool,
    pub installed_at: DateTime<Utc>,
}
```
Table: `site_plugins (site_id, plugin_name) PRIMARY KEY`. Functions: `install`, `activate`, `deactivate`, `delete`, `list_for_site`, `is_active`, `active_plugin_names`.

---

## 4. Startup Sequence

Understanding the order is critical for debugging template-not-found errors.

```
main.rs
  │
  ├─ 1. TemplateEngine::new(themes_root, startup_theme, ...)
  │       └─ load_theme_for_site(theme, site_id=None)
  │               - Reads plugin_templates lock → EMPTY (plugins not loaded yet)
  │               - Creates Tera instance from theme files only
  │               - Inserts into engines map keyed by canonical theme path
  │
  ├─ 2. load_plugins_into_engine(plugins_dir, hook_registry, engine)
  │       ├─ Scans plugins/global/* (source="global")
  │       │     For each plugin:
  │       │       - Globs **/*.html AND **/*.xml (two passes — see §7)
  │       │       - Calls engine.add_raw_template(name, source) for each file
  │       │           → writes to plugin_templates map
  │       │           → adds to all current Tera instances
  │       │       - Registers hook handlers in HookRegistry
  │       │       - Registers plugin routes
  │       │
  │       └─ Scans plugins/sites/<uuid>/* (source="site")
  │             For each plugin NOT already in registered_plugin_names:
  │               - Same template/hook registration as global
  │             For plugins already registered (copies of global):
  │               - Added to loaded_plugins for admin UI only
  │               - Templates and hooks NOT re-registered (avoids duplication)
  │
  └─ 3. AppState built, server starts
```

After step 2, `plugin_templates` contains all registered template sources, keyed by template name. The global Tera instance also has them. New Tera instances created later (for site-specific themes) re-read `plugin_templates` during lazy loading.

---

## 5. Per-Request Hook Resolution

When a public page is rendered (e.g. `GET /`):

```
home.rs::render_home(state, site_id, ...)
  │
  ├─ 1. Fetch active plugins for this site:
  │       active_plugins = site_plugin::active_plugin_names(&db, site_id).await
  │       → e.g. ["seo"]
  │
  ├─ 2. Determine theme:
  │       theme = state.active_theme_for_site(Some(site_id))
  │
  ├─ 3. Pre-render hooks:
  │       hook_outputs = templates.render_hooks_for_theme(
  │           &theme, Some(site_id),
  │           &["head_start", "head_end", "footer", ...],
  │           &ctx,
  │           Some(&active_plugins),   ← per-site filter
  │       )
  │       │
  │       └─ render_hooks_for_theme:
  │             - ensure_theme_loaded_for_site(theme, site_id)
  │               (lazy-loads site-specific Tera instance if first request)
  │             - For each hook name:
  │                 handlers = hook_registry.handlers_for(hook_name)
  │                 handlers.retain(|h| active_plugins.contains(&h.plugin_name))
  │                 For each handler: tera.render(handler.template_path, ctx)
  │                 → HTML concatenated into hook_outputs["head_end"] etc.
  │
  ├─ 4. Inject hook outputs into context as __hook_output__<name> variables
  │
  ├─ 5. Render main template:
  │       templates.render_for_theme(&theme, Some(site_id), "index.html", &ctx)
  │       │
  │       └─ The theme template calls: {{ hook(name="head_end") | safe }}
  │          The HookFunction returns sentinel: [[HOOK:__hook_output__head_end]]
  │          After render, resolve_hook_sentinels() replaces sentinels with
  │          the pre-rendered HTML from step 3.
  │
  └─ Return HTML
```

### The Hook Sentinel System

Hooks are **pre-rendered before** the main template render, not during it. The Tera `hook()` function call in a theme template returns a sentinel string `[[HOOK:__hook_output__<name>]]`. After the main template renders, `resolve_hook_sentinels()` replaces these with the pre-rendered HTML. This means:
- Hook templates execute with the **same context** as the main template
- The order hooks fire is determined by order of registration in `HookRegistry`
- Multiple plugins can register to the same hook; their outputs are concatenated

---

## 6. Per-Site Activation

The `site_plugins` table records which plugins a site has installed and activated:

```sql
CREATE TABLE site_plugins (
    site_id      UUID        NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    plugin_name  TEXT        NOT NULL,
    active       BOOLEAN     NOT NULL DEFAULT false,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (site_id, plugin_name)
);
```

**States:**
- **Not installed:** no row in `site_plugins`; plugin is not available to the site
- **Installed, inactive:** row exists with `active = false`; plugin installed but hooks don't fire
- **Installed, active:** row exists with `active = true`; hooks fire on every page request

**Admin flow (Global Plugins view):**
1. Click **Install** → copies `plugins/global/<name>/` → `plugins/sites/<uuid>/<name>/`, inserts row (`active=false`)
2. Click **Activate** → sets `active=true` in DB
3. Hooks now fire on every public page request for that site

**Important:** The template is registered in Tera at server startup (from the global copy), not at install/activate time. Activating a plugin changes the DB state that `active_plugin_names()` reads — it does not register new templates. Templates are always available in the engine; `active_plugin_names` controls whether the handlers are allowed to fire.

---

## 7. Tera Template Registration

### How templates are stored

`TemplateEngine` wraps one **Tera instance per theme directory** (keyed by canonical path). Plugin templates are added to each Tera instance as "raw templates" (not loaded from disk). They are also stored in `plugin_templates: Arc<RwLock<HashMap<String, String>>>` so that any Tera instance created later (lazy theme loads) can receive them.

```rust
// From templates/loader.rs:
pub fn add_raw_template(&self, name: &str, source: &str) -> anyhow::Result<()> {
    // 1. Persist for future theme loads
    self.plugin_templates.write().unwrap().insert(name, source);
    // 2. Add to all currently loaded Tera instances
    for tera in engines.values_mut() {
        tera.add_raw_template(name, source);
    }
}
```

When a site-specific theme is lazily loaded for the first time:
```rust
fn load_theme_for_site(&self, theme_name, site_id) {
    let mut tera = Tera::new(&glob)?; // loads theme .html files
    tera.autoescape_on(vec![".html", ".xml"]);
    // Re-add all plugin templates registered so far
    for (name, source) in self.plugin_templates.read().unwrap().iter() {
        tera.add_raw_template(name, source);
    }
    self.engines.write().unwrap().insert(cache_key, tera);
}
```

### Critical: glob brace expansion is NOT supported

The Rust `glob` crate does **not** support `{ext1,ext2}` brace expansion. This is a shell feature, not a standard glob feature.

**Wrong (silently matches nothing):**
```rust
let pattern = format!("{}/**/*.{{html,xml}}", dir);
glob::glob(&pattern) // ← matches ZERO files
```

**Correct (two separate passes):**
```rust
for ext in &["html", "xml"] {
    let pattern = format!("{}/**/*.{}", dir, ext);
    glob::glob(&pattern) // ← works correctly
}
```

This bug caused all plugin templates to be missing from the Tera engine on startup. If you ever add a new file extension to plugin templates, add it as a third separate glob pass.

### Tera instance cache key

The engines HashMap is keyed by the **canonical absolute path** of the theme directory, not the theme name. This means:
- `themes/global/default/` and `themes/sites/<uuid>/default/` are separate Tera instances
- Two sites using the same global theme share one Tera instance (same canonical path)
- The fallback chain in `render_for_theme`: site-specific key → global key → `active_theme` string (last resort, usually fails since the string is a name not a path)

---

## 8. Plugin Routes

Plugin routes are registered at server startup and wired to `plugin_route::dispatch`. They are **global** — registered from `plugins/global/` only, not from site copies.

```toml
# plugin.toml
[routes]
"/sitemap.xml" = { template = "seo/sitemap.xml", content_type = "application/xml" }
```

The `dispatch` handler:
1. Looks up the path in `AppState.plugin_routes`
2. Builds a context with all published posts/pages for the site
3. Renders the template via `render_for_theme`
4. Returns with the declared content type

**Note:** Plugin routes do not participate in per-site activation filtering. They fire for any site that has a registered route matching the request path. Per-site filtering of routes is not currently implemented.

---

## 9. Known Design Constraints

### Templates are globally shared, activation is DB-only
All plugin templates are loaded into every Tera instance regardless of site. Activation only controls whether the hook handlers are allowed to fire (via `active_plugin_names` filtering). A plugin's template source is visible to all theme instances once registered.

### Hook registry is process-wide
There is one `HookRegistry` for the entire process. All plugins from all sites register their hooks at startup. Per-site filtering happens at render time in `render_hooks_for_theme`, not at registration time.

### Duplicate registration prevention
At startup, `load_plugins_into_engine` tracks which plugin names have been registered in the global scan. Site copies of global plugins (same plugin name) are added to `loaded_plugins` for admin UI purposes but do **not** re-register hooks or templates. This prevents double rendering.

Custom site-uploaded plugins (no global counterpart) do register their own hooks and templates.

### No hot-reload for plugin templates
Restarting the server is required to pick up new plugin files or changes to existing plugin templates. Theme template files use a similar restriction (SIGUSR1 reloads theme files but not plugin templates). A future improvement would be to add a plugin reload signal.

---

## 10. Troubleshooting Runbook

### "Template 'X' not found" in logs

**Symptom:** `WARN hook 'head_end' template 'seo/meta.html' render error: Template 'seo/meta.html' not found`

**Causes and checks:**

1. **Glob brace expansion bug in custom code** — if you added new file scanning using `{ext1,ext2}`, it matches nothing. Use separate `**/*.ext1` and `**/*.ext2` passes.

2. **Template not registered at startup** — check startup logs for `add_raw_template 'seo/meta.html'`. If missing, the plugin directory wasn't scanned. Verify `plugins/global/<name>/plugin.toml` exists.

3. **Template name mismatch** — the name in `plugin.toml` `[hooks]` section must exactly match the relative path from the plugin root. For `plugins/global/seo/seo/meta.html`, the name is `seo/meta.html` (not `meta.html` or `global/seo/seo/meta.html`).

4. **add_raw_template parse error** — if the template has a Tera syntax error, `add_raw_template` fails. Look for `WARN load_theme: could not add plugin template '...'` in logs. Fix the template syntax.

5. **Plugin templates added after theme loaded** — this should not happen with the current startup order but if `TemplateEngine::new` is ever called after `load_plugins_into_engine`, the initial Tera instance won't have the templates. The `add_raw_template` method adds to existing instances, so subsequent loads are fine; only the initial load would be affected.

### Hooks fire for all sites (no per-site filtering)

**Symptom:** Plugin output appears on every site regardless of activation.

**Cause:** Public handlers are passing `None` to `render_hooks_for_theme` instead of `Some(&active_plugins)`.

**Fix:** In each public handler (`home.rs`, `post.rs`, `page.rs`, `archive.rs`, `search.rs`), ensure this pattern:
```rust
let active_plugins = crate::models::site_plugin::active_plugin_names(&state.db, site_id)
    .await
    .unwrap_or_default();
// ...
state.templates.render_hooks_for_theme(&theme, Some(site_id), &hooks, &ctx, Some(&active_plugins));
```

### Hook fires twice (duplicate output)

**Symptom:** Plugin output appears twice in the page HTML.

**Cause:** A plugin was registered twice in `HookRegistry` — once from `plugins/global/` and once from `plugins/sites/<uuid>/`.

**Fix:** Ensure `load_plugins_into_engine` uses the `registered_plugin_names` HashSet to skip hook registration for site copies of global plugins. Check `main.rs` `load_plugins_into_engine`.

### Plugin installed but hooks not firing

**Symptom:** Plugin shows as Active in admin UI, no output on site.

**Checks:**
1. `site_plugins` row: `SELECT * FROM site_plugins WHERE site_id = '<uuid>' AND plugin_name = 'seo';` — confirm `active = true`.
2. `active_plugin_names` query returns correct list — add a temporary `tracing::info!` if needed.
3. Theme's `base.html` calls the relevant hook: `grep "hook(name=" themes/global/default/templates/base.html`
4. The hook name in `plugin.toml` matches what the theme calls: `head_end` vs `head-end` (hyphens vs underscores matter).

### Sitemap returns 404 or 500

**Symptom:** `GET /sitemap.xml` returns 404 or 500.

**Checks:**
1. Plugin route registered: check `state.plugin_routes` — logged at startup as `N route(s) registered`.
2. Template not found: check for `ERROR plugin route '/sitemap.xml' render error: TemplateNotFound("seo/sitemap.xml")` — template registration issue (see above).
3. Axum router: plugin routes are registered at startup from `state.plugin_routes.keys()`. If the server started before the plugin existed, the route was never added to the router. **Restart required.**

### Adding debug logging temporarily

To trace template registration during a specific startup:

```rust
// In templates/loader.rs, add_raw_template():
tracing::info!("add_raw_template '{}': adding to {} engine(s)", name, engines.len());

// In load_theme_for_site(), before the plugin_templates loop:
tracing::info!("load_theme '{}' (site={:?}): {} plugin template(s) to add",
    theme_name, site_id, plugin_templates.len());
```

Remove after diagnosis. The LOG_LEVEL `synaptic=debug,info` ensures these appear in the log.

---

## 11. Key File Reference

| File | Purpose |
|------|---------|
| `core/src/main.rs` | `load_plugins_into_engine()` — startup scanning, template/hook/route registration |
| `core/src/plugins/manifest.rs` | `PluginManifest`, `PluginInfo`, `RouteRegistration` structs; `from_file()` parser |
| `core/src/plugins/loader.rs` | `LoadedPlugin` struct |
| `core/src/plugins/hook_registry.rs` | `HookRegistry`, `HookHandler`; `register()`, `handlers_for()` |
| `core/src/templates/loader.rs` | `TemplateEngine`; `add_raw_template()`, `load_theme_for_site()`, `render_hooks_for_theme()`, `resolve_hook_sentinels()` |
| `core/src/models/site_plugin.rs` | DB functions: `install`, `activate`, `deactivate`, `delete`, `active_plugin_names` |
| `core/src/handlers/admin/plugins.rs` | Admin handlers: `list`, `install`, `upload`, `activate`, `deactivate`, `delete`; `register_plugin_templates()` helper |
| `core/src/handlers/plugin_route.rs` | `dispatch` — renders plugin-registered routes (sitemap, etc.) |
| `core/src/handlers/home.rs` | Public home handler; per-site `active_plugins` fetch pattern |
| `admin/src/pages/plugins.rs` | `PluginCard` struct; admin plugins UI renderer |
| `migrations/0023_site_plugins.sql` | `site_plugins` table schema |
| `plugins/global/seo/` | Reference plugin implementation |
| `plugins/global/hello/` | Minimal test plugin for verifying hook injection |

---

## Appendix: Plugin Manifest Reference

```toml
[plugin]
name        = "seo"          # unique identifier; used as plugin_name in DB and hook filtering
version     = "1.0.0"
api_version = "1"
type        = "tera"         # "tera" or "wasm" (wasm not yet implemented)
description = "..."
author      = "..."

[hooks]
# hook_name = "template_path_relative_to_plugin_root"
head_end = "seo/meta.html"

[routes]
# "/url-path" = { template = "template_name", content_type = "mime/type" }
"/sitemap.xml" = { template = "seo/sitemap.xml", content_type = "application/xml" }

[meta_fields]
# custom_field_key = { label = "...", type = "text|textarea|boolean", description = "..." }
seo_title = { label = "SEO Title", type = "text", description = "Overrides the <title> tag." }
```

**Available hooks (defined in theme `base.html`):**
- `head_start` — inside `<head>`, before any content
- `head_end` — inside `<head>`, last element
- `body_start` — immediately after `<body>`
- `body_end` — immediately before `</body>`
- `before_content` — before main content area
- `after_content` — after main content area
- `footer` — inside the footer element
