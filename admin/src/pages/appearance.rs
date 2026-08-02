use crate::admin_page;

pub struct ThemeInfo {
    /// Folder name — used as the key for all operations (URLs, forms, DB, screenshots).
    /// Always matches the on-disk directory name exactly.
    pub name: String,
    /// Human-readable display name from theme.toml `name` field.
    /// May differ in capitalisation or spacing from the folder name.
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub active: bool,
    pub has_screenshot: bool,
    /// Origin of this theme: `"global"`, `"private"` (super_admin only), or `"site"`.
    pub source: String,
    /// Whether the current user is permitted to delete this theme.
    /// Computed server-side; never shown for active themes.
    pub can_delete: bool,
    /// Number of sites currently using this theme (global themes only).
    pub in_use_by: usize,
    /// True when a site copy of this global theme already exists in the site's theme folder.
    /// Only meaningful in the global filter view; always false for site themes.
    pub has_site_copy: bool,
    /// True when this theme originated from themes/private/ (even if now in a site folder).
    /// Used to keep the Private badge visible on site copies of private themes.
    pub is_private_origin: bool,
    /// True when a global copy of this private theme already exists in themes/global/.
    /// Only meaningful in the private filter view; used to show a confirmation on Make Global.
    pub has_global_copy: bool,
}

pub fn render_with_flash(themes: &[ThemeInfo], flash: Option<&str>, ctx: &crate::PageContext, filter: &str) -> String {
    let cards: String = if themes.is_empty() {
        r#"<div class="empty-state">
            <p>No themes found.</p>
        </div>"#.to_string()
    } else {
        themes.iter().map(|t| render_card(t, ctx, filter)).collect()
    };

    let sel_my      = if filter != "global" && filter != "private" { " selected" } else { "" };
    let sel_global  = if filter == "global"  { " selected" } else { "" };
    let sel_private = if filter == "private" { " selected" } else { "" };

    let toolbar = if ctx.can_manage_appearance {
        // Super admins get a three-option dropdown (My Themes, Global, Private).
        // When impersonating, private themes are hidden — they belong to the super admin's
        // own space and would be confusing in another site's context.
        // Site admins get the two-option dropdown (My Themes, Global Themes).
        let filter_options = if ctx.is_global_admin && !ctx.is_impersonating {
            format!(
                r#"<option value="my"{sel_my}>My Themes</option>
      <option value="global"{sel_global}>Global Themes</option>
      <option value="private"{sel_private}>Private Themes</option>"#,
                sel_my = sel_my,
                sel_global = sel_global,
                sel_private = sel_private,
            )
        } else {
            format!(
                r#"<option value="my"{sel_my}>My Themes</option>
      <option value="global"{sel_global}>Global Themes</option>"#,
                sel_my = sel_my,
                sel_global = sel_global,
            )
        };
        format!(
            r#"<div class="appearance-toolbar">
  <form method="GET" action="/admin/appearance" style="display:contents">
    <select name="filter" class="appearance-filter-select" onchange="this.form.submit()" aria-label="Theme filter">
      {filter_options}
    </select>
  </form>
  <a href="/admin/appearance/create" class="btn btn-primary">+ Create Theme</a>
</div>"#,
            filter_options = filter_options,
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{toolbar}<div class="theme-list">{cards}</div>
<div class="card-boxed" style="margin-top:2.5rem;max-width:520px;">
  <h2 class="card-boxed-header">Upload Theme</h2>
  <div class="card-boxed-body">
  <p class="muted" style="font-size:1.0625rem;margin-bottom:1.25rem;">Upload a <code>.zip</code> file containing a valid theme. The zip must include <code>theme.toml</code> and all required templates.</p>
  <form method="post" action="/admin/appearance/upload" enctype="multipart/form-data" class="upload-form">
    <div class="form-group">
      <input type="file" id="theme_zip" name="file" accept=".zip" required>
    </div>
    <button type="submit" class="btn btn-primary" id="theme-upload-btn" disabled>Upload &amp; Install</button>
  </form>
  </div>
</div>
<script>
(function() {{
  var input = document.getElementById('theme_zip');
  var btn   = document.getElementById('theme-upload-btn');
  input.addEventListener('change', function() {{
    btn.disabled = !input.files.length;
  }});
}})();
</script>"#
    );

    admin_page("Appearance", "/admin/appearance", flash, &content, ctx)
}

pub fn render_create_theme_form(flash: Option<&str>, ctx: &crate::PageContext) -> String {
    // Visibility radio is only shown to super_admin.
    let visibility_section = if ctx.is_global_admin {
        r#"<div class="form-group">
    <label>Visibility</label>
    <div class="radio-group">
      <label class="radio-label">
        <input type="radio" name="visibility" value="private" checked>
        <span>
          <strong>Private</strong> — only you can see, edit, and assign this theme.
          It will not appear in any site admin's theme library.
        </span>
      </label>
      <label class="radio-label">
        <input type="radio" name="visibility" value="public">
        <span>
          <strong>Public</strong> — listed in the global theme library.
          Any site admin can get a copy.
        </span>
      </label>
    </div>
  </div>"#
    } else {
        ""
    };

    let content = format!(
        r#"<div class="card-boxed">
  <h2 class="card-boxed-header">Create Theme</h2>
  <div class="card-boxed-body">
  <form method="POST" action="/admin/appearance/create" class="form-section" style="max-width:520px;">
  <div class="form-group">
    <label for="name">Theme name <span class="required">*</span></label>
    <input type="text" id="name" name="name" required maxlength="64"
           placeholder="my-theme" pattern="[^/\\\.][^/\\]*"
           title="No slashes, backslashes, or leading dots. Max 64 characters.">
    <p class="muted">Used as the folder name. Letters, numbers, hyphens, and underscores only.</p>
  </div>
  <div class="form-group">
    <label for="description">Description — 30 chars max</label>
    <input type="text" id="description" name="description" maxlength="30" placeholder="A minimal starter theme">
  </div>
  <div class="form-group">
    <label for="author">Author</label>
    <input type="text" id="author" name="author" maxlength="100" placeholder="Your name">
  </div>
  {visibility}
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Create Theme</button>
    <a href="/admin/appearance" class="btn btn-secondary">Cancel</a>
  </div>
  </form>
  </div>
</div>"#,
        visibility = visibility_section,
    );

    admin_page("Create Theme", "/admin/appearance", flash, &content, ctx)
}

pub fn render(themes: &[ThemeInfo], ctx: &crate::PageContext) -> String {
    render_with_flash(themes, None, ctx, "my")
}

// ── Theme file editor ─────────────────────────────────────────────────────────

pub struct EditorFile {
    pub rel_path: String,
    pub is_selected: bool,
    pub has_backup: bool,
    /// Formatted last-modified time, only populated when `has_backup` is true.
    pub edited_at: Option<String>,
}

// ── Theme customizer ──────────────────────────────────────────────────────────
// Opt-in, per-theme "quick customization" landing page shown before any file is
// selected in the editor. A theme only gets this UI when its theme.toml has
// [customizer] enabled = true — every other theme (including hand-uploaded
// ones) keeps today's plain "Select a file above" behavior untouched.

/// The subset of theme.toml `[theme]` fields shown in the customizer's
/// right-side "Theme Details" panel.
pub struct ThemeManifestInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

pub struct CustomizerData {
    pub manifest: ThemeManifestInfo,
    /// Declared in theme.toml's `[customizer.colors.*]` — (key, label, group,
    /// current hex value read live from the theme's own CSS `:root` block;
    /// None if that variable isn't actually defined there).
    pub colors: Vec<(String, String, String, Option<String>)>,
    /// Bool toggles declared in theme.toml's `[customizer.options.*]`
    /// (`type = "bool"`) — (option_key, label, group, current resolved
    /// value, placement ["main" or "sidebar"]). Empty for themes that
    /// declare none.
    pub options: Vec<(String, String, String, bool, String)>,
    /// Reorderable option groups declared with `type = "order"` —
    /// (option_key, label, group, items in the current resolved order as
    /// (item_key, item_label), placement). Empty for themes that declare none.
    pub order_options: Vec<(String, String, String, Vec<(String, String)>, String)>,
    /// Single-select option groups declared with `type = "choice"` —
    /// (option_key, label, group, declared (choice_key, choice_label) pairs,
    /// current resolved choice_key, placement). Empty for themes that
    /// declare none.
    pub choices: Vec<(String, String, String, Vec<(String, String)>, String, String)>,
    /// Free-form text fields declared with `type = "text"` — (option_key,
    /// label, group, current resolved string, placement). Empty for themes
    /// that declare none.
    pub texts: Vec<(String, String, String, String, String)>,
    /// Image-picker fields declared with `type = "image"` — (option_key,
    /// label, group, raw stored value [empty means no override — this is
    /// what actually gets submitted/saved], preview URL to display right now
    /// [raw value if set, else the theme's default_preview, else empty],
    /// the theme's own default_preview URL [independent of override state,
    /// so "Use Theme Default" can restore it client-side], placement).
    /// Empty for themes that declare none.
    pub images: Vec<(String, String, String, String, String, String, String)>,
    /// Whether a `.bak` backup exists for this theme's `static/css/style.css` —
    /// gates showing the "Restore original" button next to Save Colors.
    pub has_color_backup: bool,
    /// Option keys (bool/order/choice) that currently have a stored per-site
    /// override — gates showing "Restore original" on a layout-options card
    /// so it only appears once something in that card has actually changed.
    pub overridden_option_keys: std::collections::HashSet<String>,
}

/// Slugify a manifest-declared group name into something safe for DOM ids
/// (e.g. "Layout Options" -> "layout-options").
fn slugify_group(group: &str) -> String {
    group.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect()
}

/// The customizer landing page is a pure reader of whatever theme.toml
/// declares: it groups colors/options/order-groups/choices by their manifest
/// `group` field and renders one `.card-boxed` per distinct group, in the
/// order groups first appear. No card names, color roles, or option keys are
/// hardcoded here — a theme author adds a new panel just by declaring a new
/// `group` value in theme.toml.
fn render_customizer_landing(theme_name: &str, source: &str, data: &CustomizerData) -> String {
    let theme_esc = crate::html_escape(theme_name);
    let source_esc = crate::html_escape(source);

    // Restore original renders as a small Feather-icon button (rotate-ccw, the
    // same vendored icon set used app-wide e.g. for logout) in the card
    // header, top-right — rather than a stacked text button under Save
    // Changes, so it reads as a secondary/undo action, not a peer of Save.
    let restore_colors_btn = if data.has_color_backup {
        format!(
            r#"<form method="POST" action="/admin/appearance/editor/{theme}/restore" class="customizer-restore-form"
     onsubmit="return confirm('Restore the original backup? Your current color edits will be overwritten.')">
  <input type="hidden" name="file" value="static/css/style.css">
  <input type="hidden" name="source" value="{source}">
  <input type="hidden" name="stay" value="1">
  <button type="submit" class="customizer-restore-icon-btn" title="Restore original" aria-label="Restore original"><img src="/admin/static/icons/rotate-ccw.svg" alt=""></button>
</form>"#,
            theme = theme_esc,
            source = source_esc,
        )
    } else {
        String::new()
    };

    // Restore original for a card's layout options (bool/order/choice) —
    // deletes the site's stored overrides for the given keys so they fall
    // back to each option's manifest-declared default. Unlike colors there's
    // no backup file to gate on; instead only render when at least one of
    // this card's keys actually has a stored override (i.e. has been changed
    // from its default), same "only after changes" behavior as colors.
    let render_restore_options_btn = |keys: &[String]| -> String {
        let has_override = keys.iter().any(|k| data.overridden_option_keys.contains(k));
        if keys.is_empty() || !has_override { return String::new(); }
        format!(
            r#"<form method="POST" action="/admin/appearance/editor/{theme}/customizer-reset" class="customizer-restore-form"
     onsubmit="return confirm('Restore original settings? Your current changes in this section will be overwritten.')">
  <input type="hidden" name="keys" value="{keys}">
  <input type="hidden" name="source" value="{source}">
  <button type="submit" class="customizer-restore-icon-btn" title="Restore original" aria-label="Restore original"><img src="/admin/static/icons/rotate-ccw.svg" alt=""></button>
</form>"#,
            theme = theme_esc,
            source = source_esc,
            keys = crate::html_escape(&keys.join(",")),
        )
    };

    // Each of the four render_* closures below returns *inner* markup only —
    // no <form>, no submit button, no script. One group can mix any
    // combination of these, so the group-assembly step below wraps
    // whichever are non-empty in a single shared <form>/button/change-
    // detector instead of each kind managing its own.

    let render_colors_section = |entries: &[&(String, String, String, Option<String>)]| -> String {
        if entries.is_empty() { return String::new(); }
        let rows: String = entries.iter().filter_map(|(key, label, _, value)| {
            let hex = value.as_deref()?;
            Some(format!(
                r#"<div class="customizer-color-card">
  <input type="color" name="{key}" value="{hex}" class="customizer-color-swatch" title="{label}">
  <span class="customizer-color-label">{label}</span>
</div>"#,
                key = key,
                hex = crate::html_escape(hex),
                label = crate::html_escape(label),
            ))
        }).collect();
        format!(r#"<div class="customizer-color-grid">
{rows}
</div>"#, rows = rows)
    };

    let render_bool_options_section = |entries: &[&(String, String, String, bool, String)]| -> String {
        if entries.is_empty() { return String::new(); }
        let rows: String = entries.iter().map(|(key, label, _, value, _)| {
            let checked = if *value { " checked" } else { "" };
            format!(
                r#"<label class="customizer-option-row">
  <input type="checkbox" name="{key}" value="true"{checked}>
  <span>{label}</span>
</label>"#,
                key = crate::html_escape(key),
                checked = checked,
                label = crate::html_escape(label),
            )
        }).collect();
        format!(r#"<div class="customizer-option-list" style="margin-top:1rem;">
{rows}
</div>"#, rows = rows)
    };

    let render_choices_section = |entries: &[&(String, String, String, Vec<(String, String)>, String, String)]| -> String {
        if entries.is_empty() { return String::new(); }
        entries.iter().map(|(key, label, _, choices, current, _)| {
            let key_esc = crate::html_escape(key);
            let radio_rows: String = choices.iter().map(|(choice_key, choice_label)| {
                let checked = if choice_key == current { " checked" } else { "" };
                format!(
                    r#"<label class="customizer-option-row">
  <input type="radio" name="{key}" value="{choice_key}"{checked}>
  <span>{choice_label}</span>
</label>"#,
                    key = key_esc,
                    choice_key = crate::html_escape(choice_key),
                    checked = checked,
                    choice_label = crate::html_escape(choice_label),
                )
            }).collect();
            format!(
                r#"<p class="card-boxed-subheader" style="margin:1rem 0 .6rem;font-weight:600;">{label}</p>
<div class="customizer-option-list">
{rows}
</div>"#,
                label = crate::html_escape(label),
                rows = radio_rows,
            )
        }).collect()
    };

    let render_text_options_section = |entries: &[&(String, String, String, String, String)]| -> String {
        if entries.is_empty() { return String::new(); }
        entries.iter().map(|(key, label, _, value, _)| {
            format!(
                r#"<div class="customizer-text-field" style="margin-top:1rem;">
  <label class="customizer-text-label" for="customizer-text-{key}">{label}</label>
  <input type="text" name="{key}" id="customizer-text-{key}" value="{value}" maxlength="200" class="customizer-text-input">
</div>"#,
                key = crate::html_escape(key),
                label = crate::html_escape(label),
                value = crate::html_escape(value),
            )
        }).collect()
    };

    // Image-picker fields: a hidden input holds the actual value (a media
    // library URL); "Choose Image" opens the shared media picker in
    // 'customizer_image' mode targeting this field's id. Resetting back to
    // the theme's default goes through the same card-header restore icon
    // every other option type uses (render_restore_options_btn) rather than
    // a field-local button, for uniformity — there's no per-field "revert"
    // control anywhere else in the customizer either.
    let render_image_options_section = |entries: &[&(String, String, String, String, String, String, String)]| -> String {
        if entries.is_empty() { return String::new(); }
        entries.iter().map(|(key, label, _, value, preview_url, _default_preview, _)| {
            let key_esc = crate::html_escape(key);
            let input_id = format!("customizer-image-{key_esc}");
            let has_preview = !preview_url.is_empty();
            let bg_style = if has_preview {
                format!(" style=\"background-image:url('{}')\"", crate::html_escape(preview_url))
            } else {
                String::new()
            };
            format!(
                r#"<div class="customizer-image-field" style="margin-top:1rem;">
  <label class="customizer-text-label">{label}</label>
  <div class="customizer-image-picker">
    <div class="customizer-image-preview{has_image_class}" id="{input_id}-preview"{bg_style}></div>
    <div class="customizer-image-actions">
      <button type="button" class="btn btn-primary btn-sm" onclick="openMediaPicker('customizer_image', '{input_id}')">Choose Image</button>
    </div>
  </div>
  <input type="hidden" name="{key}" id="{input_id}" value="{value}">
</div>"#,
                key = key_esc,
                label = crate::html_escape(label),
                input_id = input_id,
                has_image_class = if has_preview { " has-image" } else { "" },
                bg_style = bg_style,
                value = crate::html_escape(value),
            )
        }).collect()
    };

    // Reorderable option groups: drag handle (JS reorders the <li> live on
    // dragover, then rewrites the hidden input's comma-joined value and
    // dispatches a bubbling 'change' event on dragend so the group's shared
    // save button reacts to it exactly like any other field) — same pattern
    // as the menu editor's item reordering, minus the parent/child nesting
    // and the separate auto-save endpoint (this reuses the customizer's
    // existing dirty-check + Save button flow instead).
    let render_order_section = |entries: &[&(String, String, String, Vec<(String, String)>, String)]| -> String {
        entries.iter().map(|(key, label, _, items, _)| {
            let item_rows: String = items.iter().map(|(item_key, item_label)| {
                format!(
                    r#"<li class="customizer-order-row" data-key="{item_key}">
  <span class="drag-handle" title="Drag to reorder" draggable="true"><img src="/admin/static/icons/move.svg" alt=""></span>
  <span>{item_label}</span>
</li>"#,
                    item_key = crate::html_escape(item_key),
                    item_label = crate::html_escape(item_label),
                )
            }).collect();

            let joined_keys: String = items.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(",");
            let input_id = format!("order-input-{}", crate::html_escape(key));

            format!(
                r#"<p class="card-boxed-subheader" style="margin:1rem 0 .6rem;font-weight:600;">{label}</p>
<input type="hidden" name="{key}" id="{input_id}" value="{joined_keys}">
<ol class="customizer-order-list" data-input-id="{input_id}">
{rows}
</ol>"#,
                label = crate::html_escape(label),
                key = crate::html_escape(key),
                input_id = input_id,
                joined_keys = crate::html_escape(&joined_keys),
                rows = item_rows,
            )
        }).collect()
    };

    // Renders one customizer card (header + save/restore icons + form) for a
    // single group's fields. Shared by the main-column cards and the
    // sidebar's image-only cards below, so both get identical save/dirty-
    // check behavior without duplicating the template.
    let render_group_card = |
        group: &str,
        group_colors: &[&(String, String, String, Option<String>)],
        group_options: &[&(String, String, String, bool, String)],
        group_choices: &[&(String, String, String, Vec<(String, String)>, String, String)],
        group_order: &[&(String, String, String, Vec<(String, String)>, String)],
        group_texts: &[&(String, String, String, String, String)],
        group_images: &[&(String, String, String, String, String, String, String)],
    | -> String {
        let gslug = slugify_group(group);
        let form_id = format!("customizer-form-{gslug}");
        let btn_id = format!("customizer-save-btn-{gslug}");

        let option_keys: Vec<String> = group_options.iter().map(|(k, _, _, _, _)| k.clone())
            .chain(group_order.iter().map(|(k, _, _, _, _)| k.clone()))
            .chain(group_choices.iter().map(|(k, _, _, _, _, _)| k.clone()))
            .chain(group_texts.iter().map(|(k, _, _, _, _)| k.clone()))
            .chain(group_images.iter().map(|(k, _, _, _, _, _, _)| k.clone()))
            .collect();

        // Bool-option keys belonging to *this* card only — submitted as a
        // hidden field so customizer-save knows which bool options this form
        // actually covers. Checkboxes only submit when checked, so on their
        // own an absent key is ambiguous between "unchecked" and "not part of
        // this card"; without this list, saving one card would silently zero
        // out every bool option declared in *other* cards too.
        let bool_keys: String = group_options.iter().map(|(k, _, _, _, _)| k.as_str()).collect::<Vec<_>>().join(",");

        let restore = if !group_colors.is_empty() {
            restore_colors_btn.clone()
        } else {
            render_restore_options_btn(&option_keys)
        };

        format!(
            r#"<div class="card-boxed">
  <h2 class="card-boxed-header" style="display:flex;align-items:center;justify-content:space-between;gap:.5rem;">
    <span>{group}</span>
    <div class="customizer-header-actions">
      <button type="submit" form="{form_id}" id="{btn_id}" class="customizer-save-icon-btn" disabled title="Save Changes" aria-label="Save Changes"><img src="/admin/static/icons/save.svg" alt=""></button>
      {restore}
    </div>
  </h2>
  <div class="card-boxed-body">
    <form method="POST" action="/admin/appearance/editor/{theme}/customizer-save" id="{form_id}">
      <input type="hidden" name="source" value="{source}">
      <input type="hidden" name="bool_option_keys" value="{bool_keys}">
      {colors}
      {choices}
      {texts}
      {images}
      {options}
      {order}
    </form>
  </div>
</div>
<script>
(function() {{
  var form = document.getElementById('{form_id}');
  var btn  = document.getElementById('{btn_id}');
  if (!form || !btn) return;
  // Hidden inputs are spec'd as "value mode: default" — setting .value also
  // rewrites the value content attribute, which .defaultValue reflects. So
  // for <input type=hidden> (used for the order-list fields), .value and
  // .defaultValue drift together and are never unequal. Snapshot their
  // original values ourselves instead of relying on .defaultValue.
  var initialHidden = new Map();
  form.querySelectorAll('input[type=hidden]').forEach(function(inp) {{ initialHidden.set(inp, inp.value); }});
  function checkChanged() {{
    var changed = false;
    form.querySelectorAll('input[type=color], input[type=text]').forEach(function(inp) {{
      if (inp.value !== inp.defaultValue) changed = true;
    }});
    initialHidden.forEach(function(orig, inp) {{
      if (inp.value !== orig) changed = true;
    }});
    form.querySelectorAll('input[type=checkbox], input[type=radio]').forEach(function(inp) {{
      if (inp.checked !== inp.defaultChecked) changed = true;
    }});
    btn.disabled = !changed;
  }}
  form.addEventListener('input', checkChanged);
  form.addEventListener('change', checkChanged);
}})();
</script>"#,
            group = crate::html_escape(group),
            theme = theme_esc,
            form_id = form_id,
            source = source_esc,
            bool_keys = crate::html_escape(&bool_keys),
            restore = restore,
            colors = render_colors_section(group_colors),
            choices = render_choices_section(group_choices),
            texts = render_text_options_section(group_texts),
            images = render_image_options_section(group_images),
            options = render_bool_options_section(group_options),
            order = render_order_section(group_order),
            btn_id = btn_id,
        )
    };

    // Distinct group names in first-seen order across every declared kind,
    // paired with which column each renders in. Colors always live in the
    // main column (a swatch grid needs the wider space, and the color
    // mechanism has no placement field of its own); every other kind
    // declares `placement` per-option in theme.toml, defaulting to "main"
    // (or "sidebar" for images). A group's placement is decided by whichever
    // option in it is seen first — themes aren't expected to mix placements
    // within one group name.
    let mut group_order_list: Vec<String> = Vec::new();
    let mut group_placement: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    {
        let mut note = |group: &str, placement: &str| {
            if !group_placement.contains_key(group) {
                group_placement.insert(group.to_string(), placement.to_string());
                group_order_list.push(group.to_string());
            }
        };
        for (_, _, group, _) in &data.colors { note(group, "main"); }
        for (_, _, group, _, placement) in &data.options { note(group, placement); }
        for (_, _, group, _, placement) in &data.order_options { note(group, placement); }
        for (_, _, group, _, _, placement) in &data.choices { note(group, placement); }
        for (_, _, group, _, placement) in &data.texts { note(group, placement); }
        for (_, _, group, _, _, _, placement) in &data.images { note(group, placement); }
    }

    let main_groups: Vec<String> = group_order_list.iter()
        .filter(|g| group_placement.get(*g).map(|p| p != "sidebar").unwrap_or(true))
        .cloned().collect();
    let sidebar_groups: Vec<String> = group_order_list.iter()
        .filter(|g| group_placement.get(*g).map(|p| p == "sidebar").unwrap_or(false))
        .cloned().collect();

    let build_cards = |group_list: &[String]| -> String {
        group_list.iter().map(|group| {
            let group_colors: Vec<&(String, String, String, Option<String>)> = data.colors.iter().filter(|(_, _, g, _)| g == group).collect();
            let group_options: Vec<&(String, String, String, bool, String)> = data.options.iter().filter(|(_, _, g, _, _)| g == group).collect();
            let group_choices: Vec<&(String, String, String, Vec<(String, String)>, String, String)> = data.choices.iter().filter(|(_, _, g, _, _, _)| g == group).collect();
            let group_order: Vec<&(String, String, String, Vec<(String, String)>, String)> = data.order_options.iter().filter(|(_, _, g, _, _)| g == group).collect();
            let group_texts: Vec<&(String, String, String, String, String)> = data.texts.iter().filter(|(_, _, g, _, _)| g == group).collect();
            let group_images: Vec<&(String, String, String, String, String, String, String)> = data.images.iter().filter(|(_, _, g, _, _, _, _)| g == group).collect();
            render_group_card(group, &group_colors, &group_options, &group_choices, &group_order, &group_texts, &group_images)
        }).collect()
    };

    let cards: String = build_cards(&main_groups);
    // Rendered in the sidebar below Theme Details rather than the main
    // column — any option (of any type) whose theme.toml declares
    // `placement = "sidebar"`, not just images.
    let sidebar_cards: String = build_cards(&sidebar_groups);

    let order_script = if data.order_options.is_empty() {
        String::new()
    } else {
        r#"<script>
(function() {
  document.querySelectorAll('.customizer-order-list').forEach(function(list) {
    var hidden = document.getElementById(list.dataset.inputId);
    if (!hidden) return;
    var dragEl = null;

    function update() {
      var keys = Array.from(list.querySelectorAll('li')).map(function(li) { return li.dataset.key; });
      hidden.value = keys.join(',');
      hidden.dispatchEvent(new Event('change', { bubbles: true }));
    }

    list.querySelectorAll('li').forEach(function(li) {
      var handle = li.querySelector('.drag-handle');
      if (!handle) return;
      handle.addEventListener('dragstart', function(e) {
        dragEl = li;
        li.classList.add('dragging');
        e.dataTransfer.effectAllowed = 'move';
      });
      handle.addEventListener('dragend', function() {
        if (dragEl) dragEl.classList.remove('dragging');
        dragEl = null;
        update();
      });
    });

    list.addEventListener('dragover', function(e) {
      if (!dragEl) return;
      e.preventDefault();
      var target = e.target.closest('li');
      if (!target || target === dragEl || target.parentElement !== list) return;
      var rect = target.getBoundingClientRect();
      var after = (e.clientY - rect.top) / rect.height > 0.5;
      list.insertBefore(dragEl, after ? target.nextSibling : target);
    });
  });
})();
</script>"#.to_string()
    };

    let details_card = format!(
        r#"<aside class="card-boxed customizer-details-panel">
  <h2 class="card-boxed-header">Theme Details</h2>
  <div class="card-boxed-body">
    <p style="margin-bottom:.6rem;"><strong>Name:</strong> {name}</p>
    <p style="margin-bottom:.6rem;"><strong>Version:</strong> {version}</p>
    <p style="margin-bottom:.6rem;"><strong>Author:</strong> {author}</p>
    <p style="margin-bottom:0;"><strong>Description:</strong> {description}</p>
    <p class="muted" style="font-size:.8rem;margin-top:1.25rem;">News and update checks are coming soon.</p>
  </div>
</aside>"#,
        name = crate::html_escape(&data.manifest.name),
        version = crate::html_escape(&data.manifest.version),
        author = crate::html_escape(&data.manifest.author),
        description = crate::html_escape(&data.manifest.description),
    );

    let media_picker = if data.images.is_empty() { String::new() } else { crate::media_picker_modal_html() };

    format!(
        r#"<div class="editor-body-row">
  <div class="editor-main-col">{cards}{order_script}</div>
  <div class="customizer-sidebar-col">
    {details_card}
    {sidebar_cards}
  </div>
</div>
{media_picker}"#,
        cards = cards,
        order_script = order_script,
        details_card = details_card,
        sidebar_cards = sidebar_cards,
        media_picker = media_picker,
    )
}

pub fn render_theme_editor(
    theme_name: &str,
    files: &[EditorFile],
    selected: Option<&str>,
    content: &str,
    has_backup: bool,
    flash: Option<&str>,
    ctx: &crate::PageContext,
    is_readonly: bool,
    // Which directory this theme lives in: "site", "global", or "private".
    // Threaded through every form so saves always target the correct copy.
    source: &str,
    // Some() only for themes with [customizer] enabled = true in theme.toml —
    // renders the Colors + Theme Details landing page in place of the plain
    // "Select a file above" hint when no file is selected. None preserves
    // today's behavior exactly for every other theme.
    customizer: Option<&CustomizerData>,
) -> String {
    let theme_esc = crate::html_escape(theme_name);
    let source_esc = crate::html_escape(source);

    // Build <select> options — files with a backup get a ★ marker
    let options: String = {
        let mut o = format!(r#"<option value="">— select a file —</option>"#);
        for f in files {
            let sel = if f.is_selected { " selected" } else { "" };
            let marker = if f.has_backup { " ★" } else { "" };
            o.push_str(&format!(
                r#"<option value="{val}"{sel}>{label}</option>"#,
                val   = crate::html_escape(&f.rel_path),
                sel   = sel,
                label = crate::html_escape(&format!("{}{}", &f.rel_path, marker)),
            ));
        }
        o
    };

    let file_picker = format!(
        r#"<form method="GET" action="/admin/appearance/editor/{theme}" style="display:contents;">
  <input type="hidden" name="source" value="{source}">
  <select name="file" class="editor-file-select" onchange="this.form.submit()"
          aria-label="Select theme file" title="Navigate to file">
    {options}
  </select>
</form>"#,
        theme = theme_esc,
        source = source_esc,
        options = options,
    );

    let new_file_form = if ctx.can_manage_appearance && !is_readonly {
        format!(
            r#"<button type="button" class="btn btn-sm btn-primary"
        onclick="document.getElementById('new-file-form').style.display='flex'">+ New file</button>
<div id="new-file-form" style="display:none;align-items:center;gap:.5rem;flex-wrap:wrap;margin-top:.5rem;">
  <form method="POST" action="/admin/appearance/editor/{theme}/new-file"
        style="display:contents">
    <input type="hidden" name="source" value="{source}">
    <input type="text" name="filename" placeholder="e.g. partials/header or custom"
           required style="flex:1;min-width:180px;">
    <select name="ext" style="width:auto;">
      <option value=".html">.html</option>
      <option value=".css">.css</option>
      <option value=".js">.js</option>
      <option value=".xml">.xml</option>
    </select>
    <button type="submit" class="btn btn-sm btn-primary">Create</button>
    <button type="button" class="btn btn-sm btn-secondary"
            onclick="document.getElementById('new-file-form').style.display='none'">Cancel</button>
  </form>
</div>"#,
            theme = theme_esc,
            source = source_esc,
        )
    } else {
        String::new()
    };

    // Top toolbar — always visible
    let toolbar = format!(
        r#"<div class="editor-topbar">
  {picker}
  {new_file_form}
</div>"#,
        picker = file_picker,
        new_file_form = new_file_form,
    );

    // Read-only notice shown when a site admin views a global theme.
    let readonly_notice = if is_readonly {
        r#"<div class="editor-notice editor-notice--warning">
  <strong>Global theme — read only.</strong>
  This is a shared global theme. Activate it to get your own editable copy.
</div>"#.to_string()
    } else {
        String::new()
    };

    // Editor body — shown only when a file is selected
    let body = if let Some(rel) = selected {
        let rel_esc  = crate::html_escape(rel);
        let content_esc = crate::html_escape(content);

        let restore_btn = if has_backup {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/restore" style="display:contents"
     onsubmit="return confirm('Restore the original backup? Your current edits will be overwritten.')">
  <input type="hidden" name="file" value="{file}">
  <input type="hidden" name="source" value="{source}">
  <button type="submit" class="btn btn-sm btn-secondary">Restore original</button>
</form>"#,
                theme  = theme_esc,
                file   = rel_esc,
                source = source_esc,
            )
        } else {
            String::new()
        };

        let is_required = matches!(rel,
            "templates/base.html" | "templates/index.html" | "templates/single.html" |
            "templates/page.html" | "templates/archive.html" | "templates/search.html" |
            "templates/404.html"
        );
        let delete_btn = if !is_required {
            format!(
                r#"<form method="POST" action="/admin/appearance/editor/{theme}/delete-file" style="display:contents"
     onsubmit="return confirm('Delete {file_js}? This cannot be undone.')">
  <input type="hidden" name="file" value="{file}">
  <input type="hidden" name="source" value="{source}">
  <button type="submit" class="btn btn-sm btn-danger">Delete file</button>
</form>"#,
                theme    = theme_esc,
                file     = rel_esc,
                file_js  = rel_esc,
                source   = source_esc,
            )
        } else {
            String::new()
        };

        // In readonly mode suppress all write actions.
        let (restore_btn, delete_btn) = if is_readonly {
            (String::new(), String::new())
        } else {
            (restore_btn, delete_btn)
        };

        let del_btn2 = delete_btn;
        let ro = if is_readonly { " readonly" } else { "" };
        let save_btn = if is_readonly { "" } else { r#"<button type="submit" form="save-form" class="btn btn-primary" id="save-btn" disabled>Save file</button>"# };
        let edited_at = if has_backup {
            files.iter()
                .find(|f| f.rel_path == rel)
                .and_then(|f| f.edited_at.as_deref())
                .map(|d| format!(r#" <span class="editor-edited-at">Edited: {d}</span>"#))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let is_css = rel.ends_with(".css");
        let color_sidebar = if is_css && !is_readonly {
            r#"<aside class="card-boxed editor-color-sidebar" id="editor-color-sidebar" style="display:none">
  <h2 class="card-boxed-header">Colors</h2>
  <div class="card-boxed-body">
    <div id="editor-color-swatches"></div>
    <p class="editor-color-hint">Pick a color to update its variable below. Click <strong>Save file</strong> to apply.</p>
  </div>
</aside>"#.to_string()
        } else {
            String::new()
        };

        let color_script = if is_css && !is_readonly {
            r#"<script>
(function() {
  var textarea = document.querySelector('.editor-textarea');
  var sidebar = document.getElementById('editor-color-sidebar');
  var list = document.getElementById('editor-color-swatches');
  if (!textarea || !sidebar || !list) return;

  // Friendly labels for common variable names across the built-in themes.
  // Anything not listed here falls back to a title-cased version of the name.
  var LABELS = {
    'orange-primary': 'Primary accent', 'orange-dark': 'Primary accent (dark)',
    'orange-light': 'Primary accent (light)', 'orange-pale': 'Primary accent (pale)',
    'text-dark': 'Body text', 'text-light': 'Muted text',
    'bg-light': 'Background', 'border-color': 'Border', 'placeholder': 'Placeholder text',
    'color-bg': 'Background', 'color-surface': 'Surface', 'color-text': 'Body text',
    'color-muted': 'Muted text', 'color-accent': 'Primary accent',
    'color-accent-dark': 'Primary accent (dark)', 'color-border': 'Border',
    'blue': 'Primary accent', 'blue-dark': 'Primary accent (dark)', 'navy': 'Heading text',
    'charcoal': 'Body text', 'lavender': 'Background tint', 'border': 'Border',
    'white': 'Background', 'muted': 'Muted text'
  };

  function labelFor(name) {
    if (LABELS[name]) return LABELS[name];
    return name.replace(/-/g, ' ').replace(/\b\w/g, function(c) { return c.toUpperCase(); });
  }

  function buildSwatches() {
    list.innerHTML = '';
    var rootMatch = textarea.value.match(/:root\s*{([^}]*)}/);
    if (!rootMatch) { sidebar.style.display = 'none'; return; }
    var varRe = /--([\w-]+)\s*:\s*(#[0-9a-fA-F]{3,8})\s*;/g;
    var match, count = 0;
    while ((match = varRe.exec(rootMatch[1])) !== null) {
      count++;
      var varName = match[1];
      var hex = match[2];
      var row = document.createElement('label');
      row.className = 'editor-color-row';
      var swatch = document.createElement('input');
      swatch.type = 'color';
      swatch.value = hex.length === 4
        ? '#' + hex[1] + hex[1] + hex[2] + hex[2] + hex[3] + hex[3]
        : hex.slice(0, 7);
      swatch.dataset.var = varName;
      swatch.addEventListener('input', function() { applyColor(this.dataset.var, this.value); });
      var text = document.createElement('span');
      text.textContent = labelFor(varName);
      row.appendChild(swatch);
      row.appendChild(text);
      list.appendChild(row);
    }
    sidebar.style.display = count > 0 ? 'block' : 'none';
  }

  function applyColor(varName, newHex) {
    var re = new RegExp('(--' + varName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\s*:\\s*)#[0-9a-fA-F]{3,8}(\\s*;)');
    textarea.value = textarea.value.replace(re, '$1' + newHex + '$2');
    textarea.dispatchEvent(new Event('input'));
  }

  buildSwatches();
})();
</script>"#.to_string()
        } else {
            String::new()
        };

        format!(
            r#"<div class="editor-body-row">
  <div class="editor-main-col">
    <div class="editor-meta">
      <span class="editor-filename">{file}</span>{edited_at}
      {restore}
    </div>
    <form method="POST" action="/admin/appearance/editor/{theme}/save" class="editor-form" id="save-form">
      <input type="hidden" name="file" value="{file}">
      <input type="hidden" name="source" value="{source}">
      <textarea name="content" class="editor-textarea" spellcheck="false" autocorrect="off" autocapitalize="off"{ro}>{content}</textarea>
    </form>
    <div class="editor-actions">
      {save_btn}
      {restore2}
      {del_btn2}
    </div>
    <div class="editor-comment-hint">
      <strong>Tera comments:</strong> <code>&#123;# comment #&#125;</code> — use inside <code>&#123;% block %&#125;</code> tags only.
      <code>&#123;% extends %&#125;</code> must be the very first line of the file — nothing (not even a comment) may appear before it.
      CSS/HTML comments (<code>&lt;!-- --&gt;</code>, <code>/* */</code>) outside of blocks will also break parsing.
    </div>
  </div>
  {color_sidebar}
</div>
<script>
(function() {{
  var textarea = document.querySelector('.editor-textarea');
  var btn = document.getElementById('save-btn');
  if (!textarea || !btn) return;
  var original = textarea.value;
  textarea.addEventListener('input', function() {{
    btn.disabled = textarea.value === original;
  }});
}})();
</script>
{color_script}"#,
            file     = rel_esc,
            theme    = theme_esc,
            content  = content_esc,
            source   = source_esc,
            restore  = restore_btn.clone(),
            restore2 = restore_btn,
            color_sidebar = color_sidebar,
            color_script  = color_script,
        )
    } else if let Some(data) = customizer {
        render_customizer_landing(theme_name, source, data)
    } else {
        r#"<div class="card" style="padding:1.5rem;color:var(--muted)">Select a file above to start editing.</div>"#.to_string()
    };

    let content_html = format!(
        r#"<div class="editor-wrap">{toolbar}{readonly_notice}{body}</div>"#,
        toolbar         = toolbar,
        readonly_notice = readonly_notice,
        body            = body,
    );

    admin_page(
        &format!("Edit Theme: {}", crate::html_escape(theme_name)),
        "/admin/appearance",
        flash,
        &content_html,
        ctx,
    )
}

fn render_card(t: &ThemeInfo, ctx: &crate::PageContext, filter: &str) -> String {
    // name_esc  — folder name, used for all functional references (URLs, forms, DB)
    // label_esc — display name from theme.toml, used only for visible text
    let name_esc  = crate::html_escape(&t.name);
    let label_esc = crate::html_escape(&t.display_name);

    let screenshot_html = if t.has_screenshot {
        format!(
            r#"<div class="theme-screenshot"><img src="/admin/theme-screenshot/{name}" alt="{label} preview"></div>"#,
            name  = name_esc,
            label = label_esc,
        )
    } else {
        format!(
            r#"<div class="theme-screenshot theme-screenshot-placeholder"><span>{label}</span></div>"#,
            label = label_esc,
        )
    };

    // ── Theme card badges ─────────────────────────────────────────────────────
    // All metadata badges live here in the header, right of the version badge.
    // Keep them together: [version] [Private] [site count] [any future badge].
    // Do NOT scatter new badges elsewhere in the card.
    let private_badge = if t.source == "private" || t.is_private_origin {
        r#"<span class="badge badge-private" title="Originated from a private theme">Private</span>"#
    } else {
        ""
    };

    let in_use_badge = if ctx.is_global_admin && t.source == "global" && t.in_use_by > 0 {
        format!(
            r#"<span class="badge" title="Active on {} site(s) — cannot delete">{}</span>"#,
            t.in_use_by, t.in_use_by,
        )
    } else {
        String::new()
    };

    let header = format!(
        r#"<div class="theme-card-header">
    <span class="theme-name">{label}</span>
    <span class="badge">{version}</span>{private_badge}{in_use_badge}
  </div>
  <p class="theme-description">{desc}</p>
  <p class="theme-author">by {author}</p>"#,
        label        = label_esc,
        version      = crate::html_escape(&t.version),
        private_badge = private_badge,
        in_use_badge  = in_use_badge,
        desc         = crate::html_escape(&t.description),
        author       = crate::html_escape(&t.author),
    );

    // ── Global / Private library views ───────────────────────────────────────
    // In these views all users see "Get Theme" to copy to their site folder
    // without activating. Private tab also shows an Edit button so super_admin
    // can edit the private original directly without getting a site copy first.
    if filter == "global" || filter == "private" {
        let source_val = crate::html_escape(&t.source);
        let get_html = if t.has_site_copy {
            r#"<span class="badge badge-in-use">In My Themes</span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/admin/appearance/get-theme" style="display:inline;">
    <input type="hidden" name="theme" value="{name}">
    <input type="hidden" name="source" value="{source}">
    <button type="submit" class="btn btn-primary">Get Theme</button>
</form>"#,
                name   = name_esc,
                source = source_val,
            )
        };

        // Private themes: super_admin can edit the private original directly,
        // publish it to global, or remove it entirely.
        let (edit_html, make_global_html, remove_html) = if filter == "private" {
            let edit = format!(
                r#"<a href="/admin/appearance/editor/{name}?source=private" class="btn btn-primary">Edit</a>"#,
                name = name_esc,
            );

            let confirm_attr = if t.has_global_copy {
                format!(
                    r#" onclick="return confirm('A global theme named &quot;{name}&quot; already exists. Overwrite it with this private version?')""#,
                    name = name_esc,
                )
            } else {
                String::new()
            };

            let make_global = format!(
                r#"<form method="post" action="/admin/appearance/publish-theme" style="display:inline;">
    <input type="hidden" name="theme" value="{name}">
    <button type="submit" class="btn btn-primary"{confirm}>Pub</button>
</form>"#,
                name    = name_esc,
                confirm = confirm_attr,
            );

            let remove = if t.can_delete {
                format!(
                    r#"<form method="post" action="/admin/appearance/delete" style="display:inline;"
     onsubmit="return confirm('Remove private theme &quot;{name}&quot;? This only deletes the private copy — any site copies are unaffected.')">
    <input type="hidden" name="theme" value="{name}">
    <input type="hidden" name="source" value="private">
    <button type="submit" class="btn btn-danger">Remove</button>
</form>"#,
                    name = name_esc,
                )
            } else {
                String::new()
            };

            (edit, make_global, remove)
        } else {
            (String::new(), String::new(), String::new())
        };

        return format!(
            r#"<div class="theme-card">
  {screenshot}
  {header}
  <div class="theme-actions">{get}{make_global}{edit}{remove}</div>
</div>"#,
            screenshot  = screenshot_html,
            header      = header,
            get         = get_html,
            edit        = edit_html,
            make_global = make_global_html,
            remove      = remove_html,
        );
    }

    // ── My Themes view (and super admin everywhere) ───────────────────────────
    let active_class = if t.active { " active" } else { "" };

    let activate_html = if t.active {
        String::new()
    } else {
        let confirm_msg = format!("Activate theme '{}'? This will replace the current active theme for this site.", t.display_name.replace('\'', "\\'"));
        format!(
            r#"<form method="post" action="/admin/appearance/activate" style="display:inline;"
                  data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
    <input type="hidden" name="theme" value="{name}">
    <button type="submit" class="btn btn-primary">Activate</button>
</form>"#,
            name = name_esc,
            confirm_msg = crate::html_escape(&confirm_msg),
        )
    };

    let edit_html = format!(
        r#"<a href="/admin/appearance/editor/{name}?source={source}" class="btn btn-primary">Edit</a>"#,
        name   = name_esc,
        source = crate::html_escape(&t.source),
    );

    let delete_html = if t.can_delete {
        // Site themes use "Remove" language — the user can get a fresh copy from
        // Global Themes any time. Global/private themes are permanently deleted.
        let (btn_label, confirm_msg) = if t.source == "site" {
            (
                "Remove",
                format!(
                    "Remove &quot;{name}&quot; from My Themes?\n\nYour local copy and any edits will be deleted. You can get a fresh copy from Global Themes at any time.",
                    name = name_esc,
                ),
            )
        } else {
            (
                "Delete",
                format!("Permanently delete theme &quot;{name}&quot;? This cannot be undone.", name = name_esc),
            )
        };
        format!(
            r#"<form method="post" action="/admin/appearance/delete" style="display:inline;"
                data-confirm="{confirm}" onsubmit="return confirm(this.dataset.confirm)">
    <input type="hidden" name="theme" value="{name}">
    <input type="hidden" name="source" value="{source}">
    <button type="submit" class="btn btn-danger">{label}</button>
</form>"#,
            confirm = confirm_msg,
            name    = name_esc,
            source  = crate::html_escape(&t.source),
            label   = btn_label,
        )
    } else {
        String::new()
    };

    format!(
        r#"<div class="theme-card{active}">
  {screenshot}
  {header}
  <div class="theme-actions">
    {activate}{edit}{delete}
  </div>
</div>"#,
        active     = active_class,
        screenshot = screenshot_html,
        header     = header,
        activate   = activate_html,
        edit       = edit_html,
        delete     = delete_html,
    )
}
