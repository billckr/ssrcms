use crate::admin_page;

/// Represents one plugin card shown in the admin plugins UI.
pub struct PluginCard {
    pub name: String,
    pub version: String,
    /// "tera" or "wasm"
    pub plugin_type: String,
    pub author: String,
    pub description: String,
    /// "global" or "site"
    pub source: String,
    /// True when this plugin is active for the current site (My Plugins view).
    pub is_active: bool,
    /// True when the plugin has a row in site_plugins (Global view: already installed).
    pub is_installed: bool,
    /// Hook names this plugin registers.
    pub hooks: Vec<String>,
}

pub fn render_with_flash(
    plugins: &[PluginCard],
    flash: Option<&str>,
    ctx: &crate::PageContext,
    filter: &str,
    _is_global_admin: bool,
) -> String {
    let content = render_content(plugins, filter);
    admin_page("Plugins", "/admin/plugins", flash, &content, ctx)
}

fn render_content(plugins: &[PluginCard], filter: &str) -> String {
    // ── Toolbar ───────────────────────────────────────────────────────────────
    let sel_my     = if filter == "my"     { " selected" } else { "" };
    let sel_global = if filter == "global" { " selected" } else { "" };

    let toolbar = format!(
        r#"<div class="appearance-toolbar">
  <form method="GET" action="/admin/plugins" style="display:contents">
    <select name="filter" class="appearance-filter-select" onchange="this.form.submit()" aria-label="Plugin filter">
      <option value="my"{sel_my}>My Plugins</option>
      <option value="global"{sel_global}>Global Plugins</option>
    </select>
  </form>
</div>"#,
        sel_my = sel_my,
        sel_global = sel_global,
    );

    // ── Cards ─────────────────────────────────────────────────────────────────
    let cards_html = if plugins.is_empty() {
        let msg = if filter == "global" {
            "No plugins found in the global library."
        } else {
            "No plugins installed for this site. Switch to <strong>Global Plugins</strong> to install one, or upload a zip below."
        };
        format!(r#"<div class="empty-state"><p>{}</p></div>"#, msg)
    } else {
        plugins.iter().map(|p| render_card(p, filter)).collect::<Vec<_>>().join("\n")
    };

    // ── Upload section ────────────────────────────────────────────────────────
    let upload_section = r#"<div class="theme-upload-section">
  <h2>Upload Plugin</h2>
  <p class="muted">Upload a <code>.zip</code> file containing a valid plugin. The zip must include a <code>plugin.toml</code> with a <code>[plugin]</code> section.</p>
  <form method="post" action="/admin/plugins/upload" enctype="multipart/form-data" class="upload-form">
    <div class="form-group">
      <label for="plugin_zip">Plugin zip file</label>
      <input type="file" id="plugin_zip" name="file" accept=".zip" required>
    </div>
    <button type="submit" class="btn btn-primary">Upload &amp; Install</button>
  </form>
</div>"#;

    format!(
        r#"{toolbar}<div class="plugin-list">{cards}</div>{upload}"#,
        toolbar = toolbar,
        cards = cards_html,
        upload = upload_section,
    )
}

fn render_card(p: &PluginCard, filter: &str) -> String {
    let type_badge = match p.plugin_type.as_str() {
        "wasm" => r#"<span class="badge badge-blue">WASM</span>"#,
        _      => r#"<span class="badge badge-green">Tera</span>"#,
    };

    let status_badge = if filter == "my" && p.is_active {
        r#" <span class="badge badge-success">Active</span>"#
    } else {
        ""
    };

    let hooks_html = if p.hooks.is_empty() {
        String::new()
    } else {
        let items: String = p.hooks.iter()
            .map(|h| format!(r#"<code class="hook-chip">{}</code>"#, crate::html_escape(h)))
            .collect::<Vec<_>>()
            .join(", ");
        format!(r#"<p class="muted" style="font-size:0.8em;margin-top:0.35rem;">Hooks: {}</p>"#, items)
    };

    let actions = if filter == "global" {
        if p.is_installed {
            r#"<span class="badge badge-secondary" style="margin-left:auto;">&#10003; Installed</span>"#.to_string()
        } else {
            format!(
                r#"<form method="POST" action="/admin/plugins/install" style="margin-left:auto;">
                    <input type="hidden" name="plugin_name" value="{}">
                    <button type="submit" class="btn btn-primary btn-sm">Install</button>
                </form>"#,
                crate::html_escape(&p.name)
            )
        }
    } else {
        // My Plugins: activate/deactivate + delete
        let toggle = if p.is_active {
            format!(
                r#"<form method="POST" action="/admin/plugins/deactivate">
                    <input type="hidden" name="plugin_name" value="{}">
                    <button type="submit" class="btn btn-secondary btn-sm">Deactivate</button>
                </form>"#,
                crate::html_escape(&p.name)
            )
        } else {
            format!(
                r#"<form method="POST" action="/admin/plugins/activate">
                    <input type="hidden" name="plugin_name" value="{}">
                    <button type="submit" class="btn btn-primary btn-sm">Activate</button>
                </form>"#,
                crate::html_escape(&p.name)
            )
        };

        let delete_btn = if p.is_active {
            r#"<button type="button" class="btn btn-danger btn-sm" disabled title="Deactivate first">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
            </button>"#.to_string()
        } else {
            format!(
                r#"<form method="POST" action="/admin/plugins/delete"
                    onsubmit="return confirm('Delete plugin \'{name}\'? This cannot be undone.');">
                    <input type="hidden" name="plugin_name" value="{name_esc}">
                    <button type="submit" class="btn btn-danger btn-sm" title="Delete plugin">
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
                    </button>
                </form>"#,
                name     = crate::html_escape(&p.name),
                name_esc = crate::html_escape(&p.name),
            )
        };

        format!(
            r#"<div style="display:flex;gap:0.5rem;margin-left:auto;align-items:center;">{}{}</div>"#,
            toggle, delete_btn
        )
    };

    format!(
        r#"<div class="plugin-card">
  <div class="plugin-card-header" style="display:flex;align-items:center;gap:0.5rem;flex-wrap:wrap;">
    <span class="plugin-name">{name}</span>
    <span class="badge badge-secondary">{version}</span>
    {type_badge}{status_badge}
    {actions}
  </div>
  <p class="plugin-description">{desc}</p>
  <p class="plugin-author muted" style="font-size:0.85em;margin-top:0.1rem;">by {author}</p>
  {hooks}
</div>"#,
        name         = crate::html_escape(&p.name),
        version      = crate::html_escape(&p.version),
        type_badge   = type_badge,
        status_badge = status_badge,
        actions      = actions,
        desc         = crate::html_escape(&p.description),
        author       = crate::html_escape(&p.author),
        hooks        = hooks_html,
    )
}
