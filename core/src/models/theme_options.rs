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

/// One declared option from a theme's `[customizer.options.*]` table.
#[derive(Debug, Clone)]
pub struct ThemeOptionDef {
    pub key: String,
    /// Only "bool" is supported today.
    pub option_type: String,
    pub default: bool,
    pub label: String,
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
            let default = def.get("default").and_then(|v| v.as_bool()).unwrap_or(false);
            let label = def
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or(key)
                .to_string();
            Some(ThemeOptionDef { key: key.clone(), option_type, default, label })
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
            Some(ThemeOrderDef { key: key.clone(), label, items, default })
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
