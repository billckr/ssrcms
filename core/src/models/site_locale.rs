//! Which locales a site has opted in to serving translated posts under
//! (e.g. `/es/my-post`). Stored as a single comma-separated `site_settings`
//! row (key `enabled_locales`) — the same convention as other per-site list
//! toggles (e.g. `ip_allowlist`), built on the generic
//! `app_state::get_site_setting`/`set_site_setting` helpers rather than a
//! dedicated table.
//!
//! Deliberately an explicit, admin-opted-in list rather than one derived
//! automatically from whatever `post_translations` rows happen to exist:
//! enabling a locale reserves its code as a URL path prefix for the whole
//! site (see `handlers::page::single_page`), so a real top-level post/page
//! whose slug exactly matches an enabled locale code becomes unreachable at
//! its own bare URL. That's a real trade-off (the same one WPML/Polylang
//! accept) an admin should knowingly opt into, not something that happens
//! silently the first time they translate a post.

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::Result;

const SETTING_KEY: &str = "enabled_locales";

/// The locale codes this site has enabled, in the order they were saved.
pub async fn enabled_locales_for_site(pool: &PgPool, site_id: Uuid) -> Vec<String> {
    crate::app_state::get_site_setting(pool, site_id, SETTING_KEY)
        .await
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub async fn is_locale_enabled(pool: &PgPool, site_id: Uuid, locale: &str) -> bool {
    enabled_locales_for_site(pool, site_id)
        .await
        .iter()
        .any(|l| l == locale)
}

pub async fn set_enabled_locales(pool: &PgPool, site_id: Uuid, codes: &[String]) -> Result<()> {
    let value = codes.join(",");
    crate::app_state::set_site_setting(pool, site_id, SETTING_KEY, &value).await
}
