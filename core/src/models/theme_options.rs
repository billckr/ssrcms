//! Theme customizer layout options: a theme declares a schema of boolean
//! knobs in theme.toml (`[customizer.options.*]`), and the site's chosen
//! values are stored per-site in the `theme_options` table. This is separate
//! from the color contract (which rewrites the theme's CSS file directly) —
//! layout options change template branching, so the resolved value is
//! injected into the Tera context as `theme_options` on every render instead.
//!
//! Theme templates and theme dev code never touch this table directly; core
//! resolves schema + stored override into a plain context variable before
//! rendering, the same trust boundary as any other context data.

use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

/// Default card name for any declared option/color that doesn't set `group`.
pub const DEFAULT_GROUP: &str = "Layout Options";

fn read_group(def: &toml::Table, default: &str) -> String {
    def.get("group").and_then(|v| v.as_str()).unwrap_or(default).to_string()
}

/// One declared option from a theme's `[customizer.options.*]` table.
#[derive(Debug, Clone)]
pub struct ThemeOptionDef {
    pub key: String,
    /// Only "bool" is supported today.
    pub option_type: String,
    pub default: bool,
    pub label: String,
    /// Which admin customizer card this renders in — declared per-option via
    /// `group = "..."`, defaulting to [`DEFAULT_GROUP`].
    pub group: String,
}

/// Parse the `[customizer.options.*]` table out of an already-parsed theme.toml.
/// Returns an empty list for themes that declare no options (or aren't
/// customizer-enabled at all) — every caller treats that as a cheap no-op.
pub fn parse_option_defs(parsed: &toml::Table) -> Vec<ThemeOptionDef> {
    let Some(options) = parsed
        .get("customizer")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("options"))
        .and_then(|v| v.as_table())
    else {
        return Vec::new();
    };

    options
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_table()?;
            let option_type = def
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("bool")
                .to_string();
            if option_type != "bool" {
                return None;
            }
            let default = def.get("default").and_then(|v| v.as_bool()).unwrap_or(false);
            let label = def
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or(key)
                .to_string();
            let group = read_group(def, DEFAULT_GROUP);
            Some(ThemeOptionDef { key: key.clone(), option_type, default, label, group })
        })
        .collect()
}

/// Fetch this site's stored overrides for `theme_name`, keyed by option_key.
async fn load_stored_values(pool: &PgPool, site_id: Uuid, theme_name: &str) -> HashMap<String, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT option_key, value FROM theme_options WHERE site_id = $1 AND theme_name = $2",
    )
    .bind(site_id)
    .bind(theme_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter().collect()
}

/// Resolve the final bool value for every option a theme declares — this
/// site's stored override if present, else the schema's own default. Used
/// both to render the customizer's checkboxes and (via
/// [`build_theme_options_context`]) to gate template branches on every request.
pub async fn resolve_options(
    pool: &PgPool,
    theme_dir: &Path,
    site_id: Uuid,
    theme_name: &str,
) -> Vec<(ThemeOptionDef, bool)> {
    let Ok(toml_content) = std::fs::read_to_string(theme_dir.join("theme.toml")) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) else {
        return Vec::new();
    };
    let defs = parse_option_defs(&parsed);
    if defs.is_empty() {
        return Vec::new();
    }
    let stored = load_stored_values(pool, site_id, theme_name).await;
    defs.into_iter()
        .map(|def| {
            let value = stored.get(&def.key).map(|v| v == "true").unwrap_or(def.default);
            (def, value)
        })
        .collect()
}

/// Build the `theme_options` map to inject into the Tera context: option key
/// -> resolved bool. Returns an empty map (cheap, not an error) when the
/// theme declares no options, isn't found, or has no theme.toml.
pub async fn build_theme_options_context(
    pool: &PgPool,
    theme_dir: Option<&Path>,
    site_id: Uuid,
    theme_name: &str,
) -> HashMap<String, bool> {
    let Some(theme_dir) = theme_dir else { return HashMap::new(); };
    resolve_options(pool, theme_dir, site_id, theme_name)
        .await
        .into_iter()
        .map(|(def, value)| (def.key, value))
        .collect()
}

/// Upsert one option's raw stored value for a site+theme.
async fn save_raw_value(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO theme_options (site_id, theme_name, option_key, value, updated_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (site_id, theme_name, option_key)
         DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(site_id)
    .bind(theme_name)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert one bool option's value for a site+theme — called from the
/// customizer's save-options route.
pub async fn save_option(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    value: bool,
) -> Result<(), sqlx::Error> {
    save_raw_value(pool, site_id, theme_name, key, if value { "true" } else { "false" }).await
}

/// One declared `type = "order"` option from theme.toml: a fixed set of named
/// items (`[customizer.options.<key>.items]`, item key -> display label) the
/// site can reorder, plus the schema's own default order.
#[derive(Debug, Clone)]
pub struct ThemeOrderDef {
    pub key: String,
    pub label: String,
    /// (item_key, item_label) — declared items; order here is not meaningful
    /// (toml tables don't preserve source order), only used as a label lookup.
    pub items: Vec<(String, String)>,
    /// The schema's own default ordering of item keys.
    pub default: Vec<String>,
    pub group: String,
}

/// Parse every `type = "order"` entry out of `[customizer.options.*]`.
pub fn parse_order_defs(parsed: &toml::Table) -> Vec<ThemeOrderDef> {
    let Some(options) = parsed
        .get("customizer")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("options"))
        .and_then(|v| v.as_table())
    else {
        return Vec::new();
    };

    options
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_table()?;
            let option_type = def.get("type").and_then(|v| v.as_str()).unwrap_or("bool");
            if option_type != "order" {
                return None;
            }
            let label = def.get("label").and_then(|v| v.as_str()).unwrap_or(key).to_string();
            let default: Vec<String> = def
                .get("default")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let items: Vec<(String, String)> = def
                .get("items")
                .and_then(|v| v.as_table())
                .map(|t| {
                    t.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(k).to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let group = read_group(def, DEFAULT_GROUP);
            Some(ThemeOrderDef { key: key.clone(), label, items, default, group })
        })
        .collect()
}

/// Resolve the final item order for every `order`-type option a theme
/// declares — this site's stored override if present (filtered to still-
/// declared items, with any newly-declared items appended), else the
/// schema's own default order.
pub async fn resolve_order(
    pool: &PgPool,
    theme_dir: &Path,
    site_id: Uuid,
    theme_name: &str,
) -> Vec<(ThemeOrderDef, Vec<String>)> {
    let Ok(toml_content) = std::fs::read_to_string(theme_dir.join("theme.toml")) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) else {
        return Vec::new();
    };
    let defs = parse_order_defs(&parsed);
    if defs.is_empty() {
        return Vec::new();
    }
    let stored = load_stored_values(pool, site_id, theme_name).await;
    defs.into_iter()
        .map(|def| {
            let value = stored
                .get(&def.key)
                .map(|raw| {
                    let mut order: Vec<String> = raw
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| def.items.iter().any(|(k, _)| k == s))
                        .collect();
                    for (k, _) in &def.items {
                        if !order.contains(k) {
                            order.push(k.clone());
                        }
                    }
                    order
                })
                .unwrap_or_else(|| def.default.clone());
            (def, value)
        })
        .collect()
}

/// Build the `theme_option_lists` map to inject into the Tera context:
/// order-option key -> resolved item-key order. Returns an empty map when
/// the theme declares no order-type options.
pub async fn build_theme_option_lists_context(
    pool: &PgPool,
    theme_dir: Option<&Path>,
    site_id: Uuid,
    theme_name: &str,
) -> HashMap<String, Vec<String>> {
    let Some(theme_dir) = theme_dir else { return HashMap::new(); };
    resolve_order(pool, theme_dir, site_id, theme_name)
        .await
        .into_iter()
        .map(|(def, value)| (def.key, value))
        .collect()
}

/// Upsert one order option's item sequence for a site+theme — called from
/// the customizer's save-order route.
pub async fn save_order(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    order: &[String],
) -> Result<(), sqlx::Error> {
    save_raw_value(pool, site_id, theme_name, key, &order.join(",")).await
}

/// One declared `type = "choice"` option: a fixed set of named string values
/// (`[customizer.options.<key>.choices]`, choice key -> display label) the
/// site picks exactly one of — e.g. a button-shape picker ("square"/"rounded").
/// Distinct from `type = "order"` (a ranked sequence of all items) in that
/// only one choice is ever selected, and from `type = "bool"` in that the
/// stored/resolved value is an arbitrary declared string, not true/false.
#[derive(Debug, Clone)]
pub struct ThemeChoiceDef {
    pub key: String,
    pub label: String,
    /// (choice_key, choice_label) — declared choices; order here is not
    /// meaningful (toml tables don't preserve source order).
    pub choices: Vec<(String, String)>,
    pub default: String,
    pub group: String,
}

/// Parse every `type = "choice"` entry out of `[customizer.options.*]`.
pub fn parse_choice_defs(parsed: &toml::Table) -> Vec<ThemeChoiceDef> {
    let Some(options) = parsed
        .get("customizer")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("options"))
        .and_then(|v| v.as_table())
    else {
        return Vec::new();
    };

    options
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_table()?;
            let option_type = def.get("type").and_then(|v| v.as_str()).unwrap_or("bool");
            if option_type != "choice" {
                return None;
            }
            let label = def.get("label").and_then(|v| v.as_str()).unwrap_or(key).to_string();
            let choices: Vec<(String, String)> = def
                .get("choices")
                .and_then(|v| v.as_table())
                .map(|t| {
                    t.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(k).to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let default = def
                .get("default")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| choices.first().map(|(k, _)| k.as_str()).unwrap_or(""))
                .to_string();
            let group = read_group(def, DEFAULT_GROUP);
            Some(ThemeChoiceDef { key: key.clone(), label, choices, default, group })
        })
        .collect()
}

/// Resolve the final selected choice for every `choice`-type option a theme
/// declares — this site's stored override if present and still a declared
/// choice, else the schema's own default.
pub async fn resolve_choices(
    pool: &PgPool,
    theme_dir: &Path,
    site_id: Uuid,
    theme_name: &str,
) -> Vec<(ThemeChoiceDef, String)> {
    let Ok(toml_content) = std::fs::read_to_string(theme_dir.join("theme.toml")) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) else {
        return Vec::new();
    };
    let defs = parse_choice_defs(&parsed);
    if defs.is_empty() {
        return Vec::new();
    }
    let stored = load_stored_values(pool, site_id, theme_name).await;
    defs.into_iter()
        .map(|def| {
            let value = stored
                .get(&def.key)
                .filter(|v| def.choices.iter().any(|(k, _)| &k == v))
                .cloned()
                .unwrap_or_else(|| def.default.clone());
            (def, value)
        })
        .collect()
}

/// Build the `theme_option_choices` map to inject into the Tera context:
/// choice-option key -> resolved choice key. Returns an empty map when the
/// theme declares no choice-type options.
pub async fn build_theme_option_choices_context(
    pool: &PgPool,
    theme_dir: Option<&Path>,
    site_id: Uuid,
    theme_name: &str,
) -> HashMap<String, String> {
    let Some(theme_dir) = theme_dir else { return HashMap::new(); };
    resolve_choices(pool, theme_dir, site_id, theme_name)
        .await
        .into_iter()
        .map(|(def, value)| (def.key, value))
        .collect()
}

/// Upsert one choice option's selected value for a site+theme — called from
/// the customizer's save-choices route. Caller is responsible for validating
/// `value` against the option's declared choices first.
pub async fn save_choice(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    save_raw_value(pool, site_id, theme_name, key, value).await
}

/// One declared `type = "text"` option: a free-form string a site can type
/// into a customizer text field — e.g. a hero tagline. Distinct from
/// `type = "choice"` in that the value isn't constrained to a declared set.
#[derive(Debug, Clone)]
pub struct ThemeTextDef {
    pub key: String,
    pub label: String,
    pub default: String,
    pub group: String,
}

/// Cap on a stored text option's length — generous enough for a tagline or
/// short blurb, but bounded so a theme can't be used to stash arbitrary
/// amounts of data in the DB via this column.
pub const TEXT_OPTION_MAX_LEN: usize = 200;

/// Parse every `type = "text"` entry out of `[customizer.options.*]`.
pub fn parse_text_defs(parsed: &toml::Table) -> Vec<ThemeTextDef> {
    let Some(options) = parsed
        .get("customizer")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("options"))
        .and_then(|v| v.as_table())
    else {
        return Vec::new();
    };

    options
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_table()?;
            let option_type = def.get("type").and_then(|v| v.as_str()).unwrap_or("bool");
            if option_type != "text" {
                return None;
            }
            let label = def.get("label").and_then(|v| v.as_str()).unwrap_or(key).to_string();
            let default = def.get("default").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let group = read_group(def, DEFAULT_GROUP);
            Some(ThemeTextDef { key: key.clone(), label, default, group })
        })
        .collect()
}

/// Resolve the final string for every `text`-type option a theme declares —
/// this site's stored override if present, else the schema's own default.
pub async fn resolve_texts(
    pool: &PgPool,
    theme_dir: &Path,
    site_id: Uuid,
    theme_name: &str,
) -> Vec<(ThemeTextDef, String)> {
    let Ok(toml_content) = std::fs::read_to_string(theme_dir.join("theme.toml")) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) else {
        return Vec::new();
    };
    let defs = parse_text_defs(&parsed);
    if defs.is_empty() {
        return Vec::new();
    }
    let stored = load_stored_values(pool, site_id, theme_name).await;
    defs.into_iter()
        .map(|def| {
            let value = stored.get(&def.key).cloned().unwrap_or_else(|| def.default.clone());
            (def, value)
        })
        .collect()
}

/// Build the `theme_option_texts` map to inject into the Tera context:
/// text-option key -> resolved string. Returns an empty map when the theme
/// declares no text-type options.
pub async fn build_theme_option_texts_context(
    pool: &PgPool,
    theme_dir: Option<&Path>,
    site_id: Uuid,
    theme_name: &str,
) -> HashMap<String, String> {
    let Some(theme_dir) = theme_dir else { return HashMap::new(); };
    resolve_texts(pool, theme_dir, site_id, theme_name)
        .await
        .into_iter()
        .map(|(def, value)| (def.key, value))
        .collect()
}

/// Upsert one text option's value for a site+theme — called from the
/// customizer's save-text route. Truncates to [`TEXT_OPTION_MAX_LEN`] rather
/// than rejecting, since this is free-form presentational copy, not data
/// that needs strict validation.
pub async fn save_text(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    let trimmed = value.trim();
    let truncated: String = trimmed.chars().take(TEXT_OPTION_MAX_LEN).collect();
    save_raw_value(pool, site_id, theme_name, key, &truncated).await
}

/// Which of this site's declared options for `theme_name` currently have a
/// stored override row (regardless of whether that value equals the schema
/// default) — used to gate the customizer's per-card "Restore original"
/// button so it only appears once a setting has actually been changed.
pub async fn overridden_keys(pool: &PgPool, site_id: Uuid, theme_name: &str) -> std::collections::HashSet<String> {
    load_stored_values(pool, site_id, theme_name).await.into_keys().collect()
}

/// Delete this site's stored overrides for the given option keys (any mix of
/// bool/order/choice/text/image) — used by "Restore original" on a customizer card with
/// no colors. Once a row is gone, `resolve_options`/`resolve_order`/
/// `resolve_choices` fall back to the schema's own `default` on next read.
pub async fn delete_options(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    keys: &[String],
) -> Result<(), sqlx::Error> {
    if keys.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "DELETE FROM theme_options WHERE site_id = $1 AND theme_name = $2 AND option_key = ANY($3)",
    )
    .bind(site_id)
    .bind(theme_name)
    .bind(keys)
    .execute(pool)
    .await?;
    Ok(())
}

/// One declared `type = "image"` option: a site picks a URL (via the media
/// library) that overrides a theme's built-in default image — e.g. a hero
/// background. An empty resolved value means "use the theme's own default",
/// same convention as `type = "text"` with an empty default.
#[derive(Debug, Clone)]
pub struct ThemeImageDef {
    pub key: String,
    pub label: String,
    pub default: String,
    pub group: String,
    /// Optional URL (typically under `/theme/static/...`) shown as the
    /// customizer preview when no override is stored — purely a display
    /// convenience; the resolved *value* used by templates stays empty until
    /// a site actually picks something, so theme authors don't need to touch
    /// their template's fallback logic.
    pub default_preview: Option<String>,
}

/// Parse every `type = "image"` entry out of `[customizer.options.*]`.
pub fn parse_image_defs(parsed: &toml::Table) -> Vec<ThemeImageDef> {
    let Some(options) = parsed
        .get("customizer")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("options"))
        .and_then(|v| v.as_table())
    else {
        return Vec::new();
    };

    options
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_table()?;
            let option_type = def.get("type").and_then(|v| v.as_str()).unwrap_or("bool");
            if option_type != "image" {
                return None;
            }
            let label = def.get("label").and_then(|v| v.as_str()).unwrap_or(key).to_string();
            let default = def.get("default").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let group = read_group(def, DEFAULT_GROUP);
            let default_preview = def.get("default_preview").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(ThemeImageDef { key: key.clone(), label, default, group, default_preview })
        })
        .collect()
}

/// Resolve the final URL for every `image`-type option a theme declares —
/// this site's stored override if present, else the schema's own default
/// (typically empty, meaning "fall back to the theme's built-in image").
pub async fn resolve_images(
    pool: &PgPool,
    theme_dir: &Path,
    site_id: Uuid,
    theme_name: &str,
) -> Vec<(ThemeImageDef, String)> {
    let Ok(toml_content) = std::fs::read_to_string(theme_dir.join("theme.toml")) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Table>(&toml_content) else {
        return Vec::new();
    };
    let defs = parse_image_defs(&parsed);
    if defs.is_empty() {
        return Vec::new();
    }
    let stored = load_stored_values(pool, site_id, theme_name).await;
    defs.into_iter()
        .map(|def| {
            let value = stored.get(&def.key).cloned().unwrap_or_else(|| def.default.clone());
            (def, value)
        })
        .collect()
}

/// Build the `theme_option_images` map to inject into the Tera context:
/// image-option key -> resolved URL (empty string if unset). Returns an
/// empty map when the theme declares no image-type options.
pub async fn build_theme_option_images_context(
    pool: &PgPool,
    theme_dir: Option<&Path>,
    site_id: Uuid,
    theme_name: &str,
) -> HashMap<String, String> {
    let Some(theme_dir) = theme_dir else { return HashMap::new(); };
    resolve_images(pool, theme_dir, site_id, theme_name)
        .await
        .into_iter()
        .map(|(def, value)| (def.key, value))
        .collect()
}

/// Upsert one image option's value for a site+theme — called from the
/// customizer's save-image route. `value` must be empty (reset to the
/// theme's default) or contain "/uploads/", i.e. actually come from this
/// site's media library rather than an arbitrary hotlinked or `javascript:`
/// URL — the media picker is the only supported way to set this, by design.
pub async fn save_image(
    pool: &PgPool,
    site_id: Uuid,
    theme_name: &str,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    let trimmed = value.trim();
    if !trimmed.is_empty() && !trimmed.contains("/uploads/") {
        return Ok(()); // silently ignore anything that didn't come from the media picker
    }
    save_raw_value(pool, site_id, theme_name, key, trimmed).await
}
