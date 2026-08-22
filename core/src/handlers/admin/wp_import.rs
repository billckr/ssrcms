//! Import Content from WordPress — parses a WXR (WordPress eXtended RSS)
//! export and imports both its attachments (into the media library) and its
//! posts/pages (into `posts`) in a single pass. Lives on the Site Settings
//! page's "Import Content" tab (not the Media Library) — see
//! `docs/wordpress-migration-pain-points.md` for why, and for the wider
//! migration plan this is a part of.
//!
//! Scope, deliberately: only WP's `post` and `page` post types are
//! imported (custom post types are skipped and counted) — those are the
//! only two Synap actually has admin UI/routing/templates for today. Trash,
//! auto-draft, and inherit-status items are skipped as not real content.
//! Shortcodes and Gutenberg block comments in `content:encoded` are left
//! as-is (not executed or stripped) — Synap has no shortcode runtime, so
//! they'll render as literal text; `<iframe>`/`<script>` tags (WP embeds)
//! are stripped by the existing post-content sanitizer.
//!
//! Authors: matched to an *existing* Synap user by email first; if none
//! exists, a new Author account is created (role `author`, site role
//! `author`, `can_self_publish = false`) with a random password — WP
//! doesn't export password hashes, so there's nothing else to seed it
//! with. Generated credentials are echoed back in the flash message; staff
//! self-service password recovery isn't built (see the pain-points doc's
//! "User accounts & passwords" section), so handing them out is on the
//! importing admin for now. An author with no `<wp:author>` entry at all
//! (some minimal exports omit it) falls back to the importing admin.
//!
//! WP multisite: there's no network-wide WXR export — each subsite exports
//! its own file, so a multisite migration just means running this import
//! once per subsite, into its corresponding Synap site. If the same person
//! authors on more than one subsite, they'll typically share one email
//! across those exports; when a matched-by-email author has no existing
//! access to *this* site (e.g. their account was created on an earlier
//! import into a different site), they're granted `author` access here
//! too — but only if they hold no role on this site yet, so a re-run never
//! clobbers an existing `can_self_publish` grant.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path as StdPath;

use axum::{
    extract::{Multipart, Path, State},
    response::IntoResponse,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use regex_lite::Regex;
use uuid::Uuid;

use crate::app_state::{AppState, WpImportCredential, WpImportPhase, WpImportProgress};
use crate::middleware::admin_auth::AdminUser;
use crate::models::post::{CreatePost, PostStatus, PostType, UpdatePost};
use crate::models::site_user::{self, SiteRole};
use crate::models::taxonomy::{CreateTaxonomy, Taxonomy, TaxonomyType};
use crate::models::user::{self, CreateUser, UserRole};
use super::media_store::{store_and_create, StoreInput};
use super::sanitize_media_text;
use super::sites::require_site_manager;

/// Postmeta keys that are purely WP-internal bookkeeping with no meaning in
/// Synap — skipped rather than copied into `post_meta` as clutter.
/// Everything else (including plugin-prefixed keys like `_yoast_wpseo_*` or
/// ACF field keys) is imported verbatim; mapping those to real Synap
/// features is future work (see the pain-points doc).
const SKIP_META_KEYS: &[&str] = &[
    "_thumbnail_id", // handled specially — becomes featured_image_id
    "_edit_lock",
    "_edit_last",
    "_wp_old_slug",
    "_wp_old_date",
    "_wp_desired_post_slug",
];

/// WP `post_type`s that are internal to WP's block editor / site editor,
/// never content in any meaningful sense, and never going to be imported —
/// as opposed to a genuine custom post type a plugin registered, which is
/// still worth flagging to the admin as skipped. Kept out of the flash
/// message's "skipped (unsupported type)" count so it doesn't read as a
/// migration gap; still logged at import time for anyone who wants detail.
const WP_INTERNAL_TYPES: &[&str] = &[
    "nav_menu_item",
    "wp_navigation",
    "wp_global_styles",
    "wp_template",
    "wp_template_part",
    "wp_block",
    "wp_font_family",
    "wp_font_face",
    "custom_css",
    "customize_changeset",
    "oembed_cache",
    "user_request",
];

#[derive(Default, Clone)]
struct WxrCategory {
    domain: String,
    nicename: String,
    name: String,
}

#[derive(Default, Clone)]
struct WxrItem {
    wp_post_id: String,
    post_type: String,
    title: String,
    content: String,
    excerpt: String,
    slug: String,
    status: String,
    post_date: String,
    post_date_gmt: String,
    creator: String,
    post_parent: String,
    comment_status: String,
    attachment_url: String,
    categories: Vec<WxrCategory>,
    postmeta: Vec<(String, String)>,
}

/// Parses every `<item>` in a WXR export plus the channel-level
/// `<wp:author>` blocks. Namespaced tags (`wp:`, `content:`, `excerpt:`,
/// `dc:`) are matched as literal prefixed names, same as the original
/// attachment-only parser this replaces — WXR always emits them exactly
/// this way, so full namespace resolution isn't needed.
fn parse_wxr(xml: &str) -> (Vec<WxrItem>, HashMap<String, String>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut items = Vec::new();
    let mut authors: HashMap<String, String> = HashMap::new();

    let mut in_item = false;
    let mut in_author = false;
    let mut in_postmeta = false;
    let mut in_category = false;
    let mut cur_tag: Vec<u8> = Vec::new();

    let mut item = WxrItem::default();
    let mut meta_key = String::new();
    let mut meta_value = String::new();
    let mut pending_category = WxrCategory::default();
    let mut author_login = String::new();
    let mut author_email = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("WXR parse error, stopping early: {:?}", e);
                break;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                if name == b"item" {
                    in_item = true;
                    item = WxrItem::default();
                } else if !in_item && name == b"wp:author" {
                    in_author = true;
                    author_login.clear();
                    author_email.clear();
                } else if in_item && name == b"wp:postmeta" {
                    in_postmeta = true;
                    meta_key.clear();
                    meta_value.clear();
                } else if in_item && name == b"category" {
                    in_category = true;
                    pending_category = WxrCategory::default();
                    for attr in e.attributes().flatten() {
                        let val = attr.unescape_value().unwrap_or_default().into_owned();
                        match attr.key.as_ref() {
                            b"domain" => pending_category.domain = val,
                            b"nicename" => pending_category.nicename = val,
                            _ => {}
                        }
                    }
                }
                cur_tag = name;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().map(|c| c.into_owned()).unwrap_or_default();
                assign_text(in_item, in_author, in_postmeta, in_category, &cur_tag, &text,
                    &mut item, &mut author_login, &mut author_email, &mut meta_key, &mut meta_value, &mut pending_category);
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(&e.into_inner()).into_owned();
                assign_text(in_item, in_author, in_postmeta, in_category, &cur_tag, &text,
                    &mut item, &mut author_login, &mut author_email, &mut meta_key, &mut meta_value, &mut pending_category);
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                if name == b"wp:postmeta" {
                    if !meta_key.trim().is_empty() {
                        item.postmeta.push((meta_key.clone(), meta_value.clone()));
                    }
                    in_postmeta = false;
                } else if name == b"category" {
                    if !pending_category.name.trim().is_empty() {
                        item.categories.push(pending_category.clone());
                    }
                    in_category = false;
                } else if name == b"wp:author" {
                    if !author_login.trim().is_empty() {
                        authors.insert(author_login.clone(), author_email.clone());
                    }
                    in_author = false;
                } else if name == b"item" {
                    items.push(item.clone());
                    in_item = false;
                }
            }
            _ => {}
        }
        buf.clear();
    }

    (items, authors)
}

#[allow(clippy::too_many_arguments)]
fn assign_text(
    in_item: bool,
    in_author: bool,
    in_postmeta: bool,
    in_category: bool,
    cur_tag: &[u8],
    text: &str,
    item: &mut WxrItem,
    author_login: &mut String,
    author_email: &mut String,
    meta_key: &mut String,
    meta_value: &mut String,
    pending_category: &mut WxrCategory,
) {
    if in_author {
        match cur_tag {
            b"wp:author_login" => author_login.push_str(text),
            b"wp:author_email" => author_email.push_str(text),
            _ => {}
        }
        return;
    }
    if !in_item {
        return;
    }
    if in_postmeta {
        match cur_tag {
            b"wp:meta_key" => meta_key.push_str(text),
            b"wp:meta_value" => meta_value.push_str(text),
            _ => {}
        }
        return;
    }
    if in_category {
        pending_category.name.push_str(text);
        return;
    }
    match cur_tag {
        b"title" => item.title.push_str(text),
        b"wp:post_id" => item.wp_post_id.push_str(text),
        b"wp:post_type" => item.post_type.push_str(text),
        b"wp:post_name" => item.slug.push_str(text),
        b"wp:status" => item.status.push_str(text),
        b"wp:post_date" => item.post_date.push_str(text),
        b"wp:post_date_gmt" => item.post_date_gmt.push_str(text),
        b"wp:post_parent" => item.post_parent.push_str(text),
        b"wp:comment_status" => item.comment_status.push_str(text),
        b"wp:attachment_url" => item.attachment_url.push_str(text),
        b"content:encoded" => item.content.push_str(text),
        b"excerpt:encoded" => item.excerpt.push_str(text),
        b"dc:creator" => item.creator.push_str(text),
        _ => {}
    }
}

/// "2024-03-15 10:22:00" → UTC `DateTime`. Prefers `post_date_gmt` (already
/// UTC); falls back to `post_date` treated as UTC, which is only exact if
/// the source WP site's timezone was UTC — a real but minor caveat for
/// sites configured otherwise, since WXR doesn't carry the site's timezone
/// offset outside the gmt fields.
fn parse_wp_datetime(post_date: &str, post_date_gmt: &str) -> Option<DateTime<Utc>> {
    let candidate = if !post_date_gmt.trim().is_empty() && post_date_gmt.trim() != "0000-00-00 00:00:00" {
        post_date_gmt.trim()
    } else if !post_date.trim().is_empty() && post_date.trim() != "0000-00-00 00:00:00" {
        post_date.trim()
    } else {
        return None;
    };
    NaiveDateTime::parse_from_str(candidate, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|ndt| ndt.and_utc())
}

/// Minimal percent-decoder for the filename segment pulled off an
/// attachment URL (e.g. "My%20Photo.jpg" → "My Photo.jpg"). Not a full URL
/// decoder — just enough for filenames, which is all this ever sees.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A local fallback for attachment bytes, built from an optional
/// `wp-content/uploads/`-style zip uploaded alongside the WXR file — for
/// migrations where the old WordPress site is no longer reachable over
/// HTTP. Every non-directory entry is indexed by its lowercased path
/// components so it can be matched against an attachment URL regardless of
/// how deep the zip nests the uploads folder (e.g. `uploads.zip` containing
/// either `2024/03/photo.jpg` or `wordpress/wp-content/uploads/2024/03/photo.jpg`
/// both match a URL ending in `.../uploads/2024/03/photo.jpg`).
struct ZipMediaIndex {
    entries: Vec<(Vec<String>, Vec<u8>)>,
}

impl ZipMediaIndex {
    /// Finds the best-matching file for a WP attachment URL: prefers an
    /// unambiguous match on the `{year}/{month}/{filename}` suffix (WP's
    /// upload path shape), falling back to a match on filename alone when
    /// exactly one zip entry has that name.
    fn find(&self, attachment_url: &str) -> Option<&[u8]> {
        let wanted = wp_upload_path_components(attachment_url);
        if wanted.is_empty() {
            return None;
        }

        let suffix_matches: Vec<&(Vec<String>, Vec<u8>)> = self.entries.iter()
            .filter(|(comp, _)| {
                comp.len() >= wanted.len() && comp[comp.len() - wanted.len()..] == wanted[..]
            })
            .collect();
        if let [only] = suffix_matches.as_slice() {
            return Some(&only.1);
        }
        if suffix_matches.len() > 1 {
            // Multiple entries share the same year/month/filename tail
            // (e.g. the zip has duplicate nested copies) — take the first
            // rather than guessing wrong silently in either direction.
            return Some(&suffix_matches[0].1);
        }

        let filename = wanted.last()?;
        let filename_matches: Vec<&(Vec<String>, Vec<u8>)> = self.entries.iter()
            .filter(|(comp, _)| comp.last().map(|c| c == filename).unwrap_or(false))
            .collect();
        if let [only] = filename_matches.as_slice() {
            return Some(&only.1);
        }
        None
    }
}

/// Splits the `{year}/{month}/{filename}` tail off a WP attachment URL
/// (everything after `wp-content/uploads/`, case-insensitively), percent-
/// decoded and lowercased for matching. Falls back to just the filename if
/// the URL doesn't contain that marker (some exports rewrite media through
/// a CDN/proxy path that drops it).
fn wp_upload_path_components(attachment_url: &str) -> Vec<String> {
    let marker = "wp-content/uploads/";
    let lower = attachment_url.to_lowercase();
    let tail = match lower.find(marker) {
        Some(idx) => &attachment_url[idx + marker.len()..],
        None => attachment_url.rsplit('/').next().unwrap_or(""),
    };
    percent_decode(tail)
        .to_lowercase()
        .split('/')
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string())
        .collect()
}

/// Per-entry decompression cap — a defense against zip bombs (a tiny
/// compressed file that expands to gigabytes), not a real expectation about
/// media file size. Enforced by bounding the *read*, not by trusting the
/// entry's declared uncompressed size (a crafted zip can lie about that) —
/// `Read::take` stops pulling bytes from the decompressor once the limit is
/// hit regardless of what the header claims.
const MAX_ZIP_ENTRY_BYTES: u64 = 200 * 1024 * 1024; // 200 MiB

/// Aggregate cap across every entry in one zip — guards against many
/// individually-under-the-cap entries still adding up to unbounded memory.
const MAX_ZIP_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

/// Builds a `ZipMediaIndex` from raw zip bytes (the "media files zip"
/// upload). Skips directory entries and macOS zip cruft (`__MACOSX/`,
/// `.DS_Store`). Oversized entries (see `MAX_ZIP_ENTRY_BYTES`) are skipped
/// rather than failing the whole import — the affected attachment(s) just
/// fall back to the normal HTTP fetch, same as anything else missing from
/// the zip.
fn build_zip_media_index(zip_bytes: &[u8]) -> Result<ZipMediaIndex, String> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "Media zip is not a valid zip archive.".to_string())?;

    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("failed to read zip entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.starts_with("__MACOSX/") || name.rsplit('/').next().is_some_and(|f| f.starts_with('.')) {
            continue;
        }
        if total_bytes >= MAX_ZIP_TOTAL_BYTES {
            tracing::warn!("media zip: aggregate size cap ({} bytes) reached, skipping remaining entries", MAX_ZIP_TOTAL_BYTES);
            break;
        }

        // Read one more byte than the cap so we can tell "exactly at the
        // cap" apart from "would have kept going" without ever buffering
        // more than MAX_ZIP_ENTRY_BYTES + 1.
        let mut buf = Vec::new();
        entry.by_ref().take(MAX_ZIP_ENTRY_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("failed to read zip entry '{name}': {e}"))?;
        if buf.len() as u64 > MAX_ZIP_ENTRY_BYTES {
            tracing::warn!("media zip: entry '{}' exceeds the {}-byte cap, skipped", name, MAX_ZIP_ENTRY_BYTES);
            continue;
        }

        total_bytes += buf.len() as u64;
        let components: Vec<String> = name.to_lowercase()
            .split('/')
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string())
            .collect();
        if components.is_empty() {
            continue;
        }
        entries.push((components, buf));
    }
    Ok(ZipMediaIndex { entries })
}

/// Guesses a MIME type from a filename's extension. Only needed for zip-
/// sourced attachments — a live HTTP fetch gets its MIME from the
/// `Content-Type` response header instead.
fn guess_mime_from_extension(filename: &str) -> String {
    let ext = StdPath::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "zip" => "application/zip",
        "txt" => "text/plain",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// "2024-03-15 10:22:00" (WXR's `<wp:post_date>` format) → Some("2024-03").
fn parse_year_month(post_date: &str) -> Option<String> {
    let s = post_date.trim();
    if s.len() < 7 {
        return None;
    }
    let year = &s[0..4];
    let month = &s[5..7];
    if year.chars().all(|c| c.is_ascii_digit()) && month.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("{}-{}", year, month))
    } else {
        None
    }
}

/// Mirrors the base-URL resolution posts.rs uses for the editor's "View
/// live" link — a real per-site `base_url` setting if one's configured,
/// otherwise `http://{hostname}`.
fn site_base_url(state: &AppState, site_id: Uuid) -> String {
    state.get_site_by_id(site_id)
        .map(|(site, settings)| {
            if settings.base_url != "http://localhost:3000" {
                settings.base_url
            } else {
                format!("http://{}", site.hostname)
            }
        })
        .unwrap_or_default()
}

/// Reads the live progress for site_id, if any import has ever run for it
/// this process's lifetime. Doesn't distinguish "never run" from "state was
/// lost on restart" — both just mean no polling data exists yet.
fn read_progress(state: &AppState, site_id: Uuid) -> Option<WpImportProgress> {
    state.wp_import_progress.read().unwrap().get(&site_id).cloned()
}

fn write_progress(state: &AppState, site_id: Uuid, f: impl FnOnce(&mut WpImportProgress)) {
    if let Some(entry) = state.wp_import_progress.write().unwrap().get_mut(&site_id) {
        f(entry);
    }
}

/// GET /admin/sites/{id}/import-wp/status — polled by the Import Content
/// modal's progress bar while a background import (spawned by `import`,
/// below) is running.
pub async fn status(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(site_id): Path<Uuid>,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, site_id).await {
        Ok(s) => s,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Site not found."}))).into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden."}))).into_response();
    }

    match read_progress(&state, site_id) {
        Some(progress) => axum::Json(progress).into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "No import has been started for this site."}))).into_response(),
    }
}

/// POST /admin/sites/{id}/import-wp — parses the uploaded WXR file, kicks
/// off the actual import (`run_import`) as a background task, and returns
/// immediately so the browser can show a progress modal that polls `status`
/// above rather than blocking on one long request. See this module's doc
/// comment for the import's scope/behavior — only what's needed to start
/// the background task and report why it *didn't* start lives here now.
pub async fn import(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(site_id): Path<Uuid>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, site_id).await {
        Ok(s) => s,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Site not found."}))).into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "Forbidden."}))).into_response();
    }

    if let Some(existing) = read_progress(&state, site_id) {
        if !matches!(existing.phase, WpImportPhase::Done | WpImportPhase::Error) {
            return (axum::http::StatusCode::CONFLICT, axum::Json(serde_json::json!({"error": "An import is already running for this site."}))).into_response();
        }
    }

    let mut xml_bytes: Option<Vec<u8>> = None;
    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().unwrap_or("") {
            "wxr_file" => xml_bytes = field.bytes().await.ok().map(|b| b.to_vec()),
            "media_zip" => {
                zip_bytes = field.bytes().await.ok().map(|b| b.to_vec()).filter(|b| !b.is_empty());
            }
            _ => {}
        }
    }

    let Some(xml_bytes) = xml_bytes else {
        return (axum::http::StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error": "No file uploaded."}))).into_response();
    };

    let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
    let (items, authors) = parse_wxr(&xml);

    if items.is_empty() {
        tracing::warn!("WP import (site {}): 0 items parsed from a {}-byte export — likely not a WXR file, or an empty one", site_id, xml_bytes.len());
        return (axum::http::StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error": "No content found in that export file."}))).into_response();
    }

    tracing::info!(
        "WP import (site {}) starting: {} items parsed ({} authors listed) from a {}-byte export",
        site_id, items.len(), authors.len(), xml_bytes.len(),
    );
    {
        let mut type_counts: HashMap<&str, usize> = HashMap::new();
        for item in &items {
            *type_counts.entry(item.post_type.as_str()).or_insert(0) += 1;
        }
        tracing::info!("WP import (site {}): item post_types breakdown: {:?}", site_id, type_counts);
    }

    // Optional local fallback for attachment bytes, for when the old WP site
    // is no longer reachable over HTTP — see ZipMediaIndex's docs. A bad zip
    // just means every attachment falls back to the HTTP fetch, same as if
    // no zip had been uploaded at all; it doesn't abort the import.
    let zip_index = match &zip_bytes {
        Some(bytes) => match build_zip_media_index(bytes) {
            Ok(idx) => {
                tracing::info!("WP import (site {}): media zip uploaded, {} file(s) indexed", site_id, idx.entries.len());
                Some(idx)
            }
            Err(e) => {
                tracing::warn!("WP import (site {}): media zip ignored — {}", site_id, e);
                None
            }
        },
        None => None,
    };

    let media_total = items.iter().filter(|i| i.post_type == "attachment").count();
    let content_total = items.iter().filter(|i| i.post_type == "post" || i.post_type == "page").count();
    state.wp_import_progress.write().unwrap().insert(site_id, WpImportProgress {
        phase: WpImportPhase::Media,
        media_total,
        media_done: 0,
        content_total,
        content_done: 0,
        message: None,
        credentials: Vec::new(),
        new_author_count: 0,
        granted_author_access: 0,
        error: None,
    });

    let admin_user_id = admin.user.id;
    let bg_state = state.clone();
    tokio::spawn(async move {
        run_import(bg_state, site_id, admin_user_id, items, authors, zip_index).await;
    });

    axum::Json(serde_json::json!({"started": true})).into_response()
}

/// Does the actual import work (spawned by `import`, above), updating
/// `state.wp_import_progress[site_id]` as it goes so `status` has something
/// live to report. Scope/behavior notes for the import itself live in this
/// module's top doc comment, not here.
async fn run_import(
    state: AppState,
    site_id: Uuid,
    admin_user_id: Uuid,
    items: Vec<WxrItem>,
    authors: HashMap<String, String>,
    zip_index: Option<ZipMediaIndex>,
) {
    // ── Pass 1: attachments ──────────────────────────────────────────────
    // Redirects disabled deliberately — see is_safe_import_url's doc comment:
    // the SSRF host check only covers the request's initial target, and
    // following a redirect would let a same-origin-looking URL 302 its way
    // to an internal address after passing that check.
    let client = reqwest::Client::builder()
        .user_agent("SynapCMS-WP-Importer/1.0")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut media_ok = 0usize;
    let mut media_from_zip = 0usize;
    let mut media_reused = 0usize;
    let mut media_failed = 0usize;
    // wp_post_id (of the attachment item) → its old URL — needed to resolve
    // _thumbnail_id postmeta on posts/pages, which references this ID.
    let mut attachment_id_to_url: HashMap<String, String> = HashMap::new();
    // old URL → new media_id, for this run — merged with persisted mappings
    // below before content/thumbnail rewriting.
    let mut run_media_map: HashMap<String, Uuid> = HashMap::new();

    for item in items.iter().filter(|i| i.post_type == "attachment") {
        if item.attachment_url.trim().is_empty() {
            write_progress(&state, site_id, |p| p.media_done += 1);
            continue;
        }
        attachment_id_to_url.insert(item.wp_post_id.clone(), item.attachment_url.clone());

        match crate::models::wp_import::find(&state.db, site_id, &item.attachment_url).await {
            Ok(Some(existing_id)) => {
                media_reused += 1;
                run_media_map.insert(item.attachment_url.clone(), existing_id);
                write_progress(&state, site_id, |p| p.media_done += 1);
                continue;
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("wp_import_media_map lookup failed for {}: {:?}", item.attachment_url, e),
        }

        match import_attachment(&state, &client, site_id, admin_user_id, item, zip_index.as_ref()).await {
            Ok((media_id, from_zip)) => {
                media_ok += 1;
                if from_zip {
                    media_from_zip += 1;
                }
                run_media_map.insert(item.attachment_url.clone(), media_id);
            }
            Err(e) => {
                media_failed += 1;
                tracing::warn!("WP media import failed for {}: {}", item.attachment_url, e);
            }
        }
        write_progress(&state, site_id, |p| p.media_done += 1);
    }

    // Merge with anything already recorded from a previous run (e.g. a
    // different export from the same site, or media imported before posts
    // existed) so thumbnail/content-image rewriting has full coverage.
    let persisted_media_map = crate::models::wp_import::map_for_site(&state.db, site_id)
        .await
        .unwrap_or_default();
    let mut media_map = persisted_media_map;
    media_map.extend(run_media_map);

    let base_url = site_base_url(&state, site_id);
    let (exact_rewrite, fuzzy_rewrite) = build_rewrite_maps(&state, &media_map, &base_url).await;

    // ── Pass 1.5: authors — match or create ──────────────────────────────
    let (author_map, new_author_creds, granted_author_access) = ensure_author_accounts(&state, site_id, admin_user_id, &authors).await;

    write_progress(&state, site_id, |p| p.phase = WpImportPhase::Content);

    // ── Pass 2: posts/pages — create ─────────────────────────────────────
    let mut posts_ok = 0usize;
    let mut posts_failed = 0usize;
    let mut posts_skipped_status = 0usize;
    let mut posts_skipped_type = 0usize;
    let mut authors_unmatched = 0usize;
    // Old WP post ID → new Synap post UUID, for the parent-linking pass
    // below (WXR item order isn't guaranteed to put parents before
    // children, so parent_id has to be patched in a second pass).
    let mut post_id_map: HashMap<String, Uuid> = HashMap::new();
    // Items that need a parent_id patched in pass 3: (new post id, old parent wp id).
    let mut pending_parents: Vec<(Uuid, String)> = Vec::new();

    for item in items.iter() {
        if item.post_type != "post" && item.post_type != "page" {
            if item.post_type != "attachment" {
                if WP_INTERNAL_TYPES.contains(&item.post_type.as_str()) {
                    tracing::info!(
                        "WP import (site {}): skipped wp_post_id={} title={:?} — WP-internal post_type={:?} (block-theme/site-editor data, never imported, not counted toward the summary)",
                        site_id, item.wp_post_id, item.title, item.post_type,
                    );
                } else {
                    posts_skipped_type += 1;
                    tracing::info!(
                        "WP import (site {}): skipped wp_post_id={} title={:?} — unsupported post_type={:?} (only post/page are imported)",
                        site_id, item.wp_post_id, item.title, item.post_type,
                    );
                }
            }
            continue;
        }
        match import_post(&state, site_id, admin_user_id, item, &author_map, &attachment_id_to_url, &media_map, &exact_rewrite, &fuzzy_rewrite).await {
            Ok(ImportedPost { post_id, author_matched, skipped_status }) => {
                if skipped_status {
                    posts_skipped_status += 1;
                    tracing::info!(
                        "WP import (site {}): skipped wp_post_id={} title={:?} — WP status={:?} has no Synap equivalent worth importing (trash/auto-draft/inherit/unrecognized)",
                        site_id, item.wp_post_id, item.title, item.status,
                    );
                    write_progress(&state, site_id, |p| p.content_done += 1);
                    continue;
                }
                posts_ok += 1;
                if !author_matched {
                    authors_unmatched += 1;
                    tracing::info!(
                        "WP import (site {}): post {} (wp_post_id={}) — dc:creator={:?} had no email on file, assigned to importing admin",
                        site_id, post_id, item.wp_post_id, item.creator,
                    );
                }
                post_id_map.insert(item.wp_post_id.clone(), post_id);
                if item.post_parent.trim() != "0" && !item.post_parent.trim().is_empty() {
                    pending_parents.push((post_id, item.post_parent.clone()));
                }
            }
            Err(e) => {
                posts_failed += 1;
                tracing::warn!("WP post import failed for wp_post_id={}: {}", item.wp_post_id, e);
            }
        }
        write_progress(&state, site_id, |p| p.content_done += 1);
    }

    // ── Pass 3: patch parent_id now that every post has a new UUID ───────
    let mut parents_unresolved = 0usize;
    for (post_id, old_parent_id) in pending_parents {
        match post_id_map.get(&old_parent_id) {
            Some(&new_parent_id) => {
                let update = UpdatePost {
                    title: None, slug: None, content: None, content_format: None, excerpt: None,
                    status: None, featured_image_id: None, clear_featured_image: false,
                    published_at: None, template: None, clear_post_password: false,
                    new_post_password_hash: None, comments_enabled: None,
                    parent_id: Some(Some(new_parent_id)),
                    sources: None, sources_public: None,
                };
                if let Err(e) = crate::models::post::update(&state.db, post_id, &update).await {
                    tracing::warn!("failed to set parent on imported post {}: {:?}", post_id, e);
                }
            }
            None => {
                parents_unresolved += 1;
                tracing::info!(
                    "WP import (site {}): post {} — parent wp_post_id={} was never imported (deleted, unsupported type, or skipped status), left without a parent",
                    site_id, post_id, old_parent_id,
                );
            }
        }
    }

    // Imported posts are created directly via the DB (create_post_unique_slug),
    // not the normal admin post handlers, so they never go through the
    // per-post index_post() upsert those handlers call on publish. A single
    // full rebuild here (same call the admin UI's "Rebuild Search Index"
    // button and `synap search reindex` use) is cheap — one batch commit —
    // and means imported content is searchable immediately instead of only
    // after the next restart or a manual reindex.
    match crate::search::indexer::rebuild_index((*state.search_index).clone(), state.db.clone()).await {
        Some(indexed) => tracing::info!("WP import (site {}): search index rebuilt, {} document(s) indexed", site_id, indexed),
        None => tracing::warn!("WP import (site {}): search index rebuild failed — imported content won't be searchable until the next restart or a manual reindex", site_id),
    }

    // Full item-by-item breakdown goes to the server log only — the modal
    // shows just a short success/partial/failure line plus any new author
    // credentials (the one thing that can't be recovered from the log,
    // since passwords are deliberately not logged in plaintext).
    tracing::info!(
        "WP import (site {}) done: media {} imported ({} from zip)/{} reused/{} failed; content {} imported/{} failed/{} skipped-status/{} skipped-type; {} author(s) unmatched, {} new author account(s), {} existing author(s) granted site access, {} parent link(s) unresolved",
        site_id, media_ok, media_from_zip, media_reused, media_failed,
        posts_ok, posts_failed, posts_skipped_status, posts_skipped_type,
        authors_unmatched, new_author_creds.len(), granted_author_access, parents_unresolved,
    );

    let total_ok = media_ok + posts_ok;
    let total_failed = media_failed + posts_failed;
    let msg = if total_failed == 0 {
        "Import completed successfully.".to_string()
    } else if total_ok == 0 {
        "Import failed. Check server logs for details.".to_string()
    } else {
        format!(
            "Import completed with some failures ({} item(s) failed). Check server logs for details.",
            total_failed,
        )
    };
    let credentials: Vec<WpImportCredential> = new_author_creds.into_iter()
        .map(|(username, password)| WpImportCredential { username, password })
        .collect();
    let new_author_count = credentials.len();

    write_progress(&state, site_id, |p| {
        p.phase = WpImportPhase::Done;
        p.message = Some(msg);
        p.credentials = credentials;
        p.new_author_count = new_author_count;
        p.granted_author_access = granted_author_access;
    });
}

/// RFC 4180 CSV field escaping — same rule `forms::export_csv` uses.
fn csv_escape(s: &str) -> String {
    if s.contains('"') || s.contains(',') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// GET /admin/sites/{id}/import-wp/credentials.csv — downloads the new
/// author accounts created by the most recent import as a CSV, then drains
/// them from the progress state so the same one-time passwords can't be
/// re-downloaded. 404s if no import has completed with new accounts (either
/// none were created, or they were already downloaded once).
pub async fn credentials_csv(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(site_id): Path<Uuid>,
) -> impl IntoResponse {
    let site = match crate::models::site::get_by_id(&state.db, site_id).await {
        Ok(s) => s,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, "Site not found.").into_response(),
    };
    if !require_site_manager(&state, &admin, &site).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden.").into_response();
    }

    let credentials = {
        let mut map = state.wp_import_progress.write().unwrap();
        match map.get_mut(&site_id) {
            Some(entry) => std::mem::take(&mut entry.credentials),
            None => Vec::new(),
        }
    };

    if credentials.is_empty() {
        return (axum::http::StatusCode::NOT_FOUND, "No new-account credentials available (none were created, or they were already downloaded).").into_response();
    }

    let mut csv = String::from("username,password\n");
    for c in &credentials {
        csv.push_str(&csv_escape(&c.username));
        csv.push(',');
        csv.push_str(&csv_escape(&c.password));
        csv.push('\n');
    }

    (
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"wp-import-credentials.csv\"".to_string()),
        ],
        csv,
    ).into_response()
}

/// Ensures every WP author referenced in the export has a Synap user:
/// matched by email if one already exists, otherwise created fresh (role
/// `author`, granted site role `author` with `can_self_publish = false`,
/// random password). Returns a WP login → Synap user id map for
/// `import_post` to consume, plus the (username, password) pairs for any
/// account actually created (so the caller can hand them to the admin).
///
/// Authors with no email on file (some minimal exports omit it) are left
/// out of the returned map entirely — `import_post` falls back to the
/// importing admin for those, same as an unrecognized login.
async fn ensure_author_accounts(
    state: &AppState,
    site_id: Uuid,
    admin_user_id: Uuid,
    authors: &HashMap<String, String>,
) -> (HashMap<String, Uuid>, Vec<(String, String)>, usize) {
    let mut login_to_id: HashMap<String, Uuid> = HashMap::new();
    let mut created: Vec<(String, String)> = Vec::new();
    let mut granted_access = 0usize;

    for (login, email) in authors {
        let email = email.trim();
        if email.is_empty() {
            continue;
        }
        if let Ok(existing) = user::get_by_email(&state.db, email).await {
            login_to_id.insert(login.clone(), existing.id);
            // A user matched by email may be a real Synap account with no
            // access to *this* site at all — e.g. someone who authors on
            // more than one WP subsite of the same network, matched here
            // by an import into a second Synap site. Grant them 'author'
            // access on this site too, but only if they hold no role here
            // yet — site_user::add() upserts can_self_publish on conflict,
            // so blindly re-calling it on every run would silently reset an
            // existing author's self-publish flag back to false.
            match site_user::has_any_role(&state.db, site_id, existing.id).await {
                Ok(false) => {
                    match site_user::add(&state.db, site_id, existing.id, SiteRole::Author, Some(admin_user_id), false).await {
                        Ok(_) => granted_access += 1,
                        Err(e) => tracing::warn!("failed to grant site access to matched author {} <{}>: {:?}", login, email, e),
                    }
                }
                Ok(true) => {}
                Err(e) => tracing::warn!("failed to check existing site access for matched author {} <{}>: {:?}", login, email, e),
            }
            continue;
        }

        let username = unique_import_username(state, login).await;
        let password = user::generate_password();
        let create = CreateUser {
            username: username.clone(),
            email: email.to_string(),
            display_name: if login.trim().is_empty() { username.clone() } else { login.trim().to_string() },
            password: password.clone(),
            role: UserRole::Author,
        };
        match user::create(&state.db, &create).await {
            Ok(new_user) => {
                if let Err(e) = site_user::add(&state.db, site_id, new_user.id, SiteRole::Author, Some(admin_user_id), false).await {
                    tracing::warn!("failed to grant site access to imported author {}: {:?}", username, e);
                }
                login_to_id.insert(login.clone(), new_user.id);
                created.push((username, password));
            }
            Err(e) => tracing::warn!("failed to create account for WP author '{}' <{}>: {:?}", login, email, e),
        }
    }

    (login_to_id, created, granted_access)
}

/// Derives a valid, unique username from a WP author login: lowercased,
/// non-conforming characters collapsed to hyphens, padded/truncated to
/// `validate_username`'s 5-15 char rule, then suffixed with a short random
/// string on collision (WP logins can't be trusted to be unique against
/// Synap's existing users, or even valid Synap usernames at all).
async fn unique_import_username(state: &AppState, login: &str) -> String {
    let mut base: String = login.trim().to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '-' })
        .collect();
    while base.starts_with('-') { base.remove(0); }
    while base.ends_with('-') { base.pop(); }
    if base.is_empty() {
        base = "wpauthor".to_string();
    }
    if base.len() > 15 {
        base.truncate(15);
        while base.ends_with('-') { base.pop(); }
    }
    while base.len() < 5 {
        base.push('0');
    }

    if user::get_by_username_include_inactive(&state.db, &base).await.is_err() {
        return base;
    }
    for _ in 0..25 {
        let suffix = rand_suffix(4);
        let max_base_len = 15 - 1 - suffix.len();
        let mut trimmed = base.clone();
        if trimmed.len() > max_base_len {
            trimmed.truncate(max_base_len);
            while trimmed.ends_with('-') { trimmed.pop(); }
        }
        let candidate = format!("{trimmed}-{suffix}");
        if user::get_by_username_include_inactive(&state.db, &candidate).await.is_err() {
            return candidate;
        }
    }
    format!("wp{}", &Uuid::new_v4().simple().to_string()[..13])
}

fn rand_suffix(n: usize) -> String {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = StdRng::from_entropy();
    (0..n).map(|_| chars[rng.gen_range(0..chars.len())] as char).collect()
}

/// Rejects any IP that isn't routable on the public internet — loopback,
/// RFC1918/CGNAT private ranges, link-local (this is what closes off cloud
/// metadata endpoints like `169.254.169.254`), unspecified, multicast,
/// broadcast, and their IPv6 equivalents (including IPv4-mapped IPv6
/// addresses, checked against the same IPv4 rules).
fn is_public_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || is_cgnat_v4(v4))
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_public_ip(&std::net::IpAddr::V4(mapped));
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || is_unique_local_v6(v6)
                || is_link_local_v6(v6))
        }
    }
}

/// 100.64.0.0/10 — carrier-grade NAT range, not covered by `Ipv4Addr::is_private()`.
fn is_cgnat_v4(v4: &std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000
}

/// fc00::/7 — IPv6 unique local addresses (the IPv6 analog of RFC1918).
fn is_unique_local_v6(v6: &std::net::Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xfe00) == 0xfc00
}

/// fe80::/10 — IPv6 link-local.
fn is_link_local_v6(v6: &std::net::Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xffc0) == 0xfe80
}

/// SSRF guard for attachment URLs pulled straight out of an uploaded WXR
/// file: only `http`/`https` are allowed, and every IP the host resolves to
/// must be public — rejects if *any* resolved address is internal, since an
/// attacker only needs one A/AAAA record to point somewhere sensitive.
/// Literal IPs in the URL are checked directly, no DNS lookup needed.
///
/// This only checks the request's initial target — the `reqwest::Client`
/// this guards is built with redirects disabled (`Policy::none()`)
/// specifically so a same-origin-looking URL can't 302 its way to an
/// internal address after passing this check. A source site whose media
/// URLs actually require a redirect will fail the fetch; the zip-upload
/// fallback (`ZipMediaIndex`) is the supported way to import media from a
/// site like that anyway.
async fn is_safe_import_url(url_str: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url_str) else { return false };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    let Some(host) = parsed.host_str().map(|h| h.to_string()) else { return false };

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return is_public_ip(&ip);
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let lookup = tokio::net::lookup_host((host.as_str(), port)).await;
    let result = match lookup {
        Ok(addrs) => {
            let mut saw_any = false;
            let mut all_public = true;
            for addr in addrs {
                saw_any = true;
                if !is_public_ip(&addr.ip()) {
                    all_public = false;
                    break;
                }
            }
            saw_any && all_public
        }
        Err(_) => false,
    };
    result
}

async fn import_attachment(
    state: &AppState,
    client: &reqwest::Client,
    site_id: Uuid,
    uploaded_by: Uuid,
    item: &WxrItem,
    zip_index: Option<&ZipMediaIndex>,
) -> Result<(Uuid, bool), String> {
    let (bytes, mime, from_zip) = if let Some(local) = zip_index.and_then(|idx| idx.find(&item.attachment_url)) {
        let filename_guess = item.attachment_url.rsplit('/').next().unwrap_or("attachment");
        (local.to_vec(), guess_mime_from_extension(filename_guess), true)
    } else {
        if !is_safe_import_url(&item.attachment_url).await {
            return Err("refused: URL does not resolve to a public address".to_string());
        }
        let resp = client.get(&item.attachment_url).send().await.map_err(|e| format!("fetch failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("fetch failed: HTTP {}", resp.status()));
        }
        let mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let bytes = resp.bytes().await.map_err(|e| format!("read body failed: {e}"))?.to_vec();
        (bytes, mime, false)
    };

    let filename = item
        .attachment_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .unwrap_or_else(|| "attachment".to_string());
    let alt_text = item.postmeta.iter()
        .find(|(k, _)| k == "_wp_attachment_image_alt")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    let title = if item.title.trim().is_empty() { filename.clone() } else { item.title.clone() };

    let folder_name = parse_year_month(&item.post_date).unwrap_or_else(|| "unsorted".to_string());
    let folder = crate::models::media_folder::get_or_create(&state.db, site_id, &folder_name)
        .await
        .map_err(|e| format!("folder lookup failed: {e}"))?;

    let input = StoreInput {
        filename: filename.clone(),
        mime,
        bytes,
        alt_text: sanitize_media_text(alt_text),
        title: sanitize_media_text(&title),
        caption: String::new(),
        folder_id: Some(folder.id),
    };

    let media = store_and_create(state, Some(site_id), uploaded_by, input)
        .await
        .map_err(|e| format!("store failed: {e}"))?;

    if let Err(e) = crate::models::wp_import::record(&state.db, site_id, &item.attachment_url, media.id).await {
        tracing::warn!("failed to record wp_import_media_map for {}: {:?}", item.attachment_url, e);
    }

    Ok((media.id, from_zip))
}

/// Builds two lookup tables from old attachment URL → new Synap media URL:
/// an exact-match table, and a "fuzzy" table keyed by the filename with
/// WP's `-{width}x{height}` resize suffix stripped (e.g.
/// "sunset-photo-300x200.jpg" → "sunset-photo.jpg"), since WP post content
/// almost always references a specific resized variant rather than the
/// original file Synap actually imported.
async fn build_rewrite_maps(state: &AppState, media_map: &HashMap<String, Uuid>, base_url: &str) -> (HashMap<String, String>, HashMap<String, String>) {
    let ids: Vec<Uuid> = media_map.values().copied().collect();
    let media_rows = crate::models::media::get_by_ids(&state.db, &ids).await.unwrap_or_default();
    let url_by_id: HashMap<Uuid, String> = media_rows.iter()
        .map(|m| (m.id, m.url(base_url)))
        .collect();

    let size_suffix = Regex::new(r"-\d+x\d+(\.\w+)$").unwrap();

    let mut exact = HashMap::new();
    let mut fuzzy = HashMap::new();
    for (old_url, media_id) in media_map {
        let Some(new_url) = url_by_id.get(media_id) else { continue };
        exact.insert(old_url.clone(), new_url.clone());
        let stripped = size_suffix.replace(old_url, "$1").into_owned();
        fuzzy.entry(stripped).or_insert_with(|| new_url.clone());
    }
    (exact, fuzzy)
}

/// Rewrites every attachment URL found in `content` to its imported Synap
/// equivalent. Exact matches first, then the size-suffix-stripped fuzzy
/// match; anything neither table covers (an external image, or an
/// attachment that failed to import) is left untouched.
fn rewrite_content_urls(content: &str, exact: &HashMap<String, String>, fuzzy: &HashMap<String, String>) -> String {
    let size_suffix = Regex::new(r"-\d+x\d+(\.\w+)$").unwrap();
    let src_re = Regex::new(r#"(src|href)="([^"]+)""#).unwrap();

    src_re.replace_all(content, |caps: &regex_lite::Captures| {
        let attr = &caps[1];
        let url = &caps[2];
        if let Some(new_url) = exact.get(url) {
            return format!(r#"{attr}="{new_url}""#);
        }
        let stripped = size_suffix.replace(url, "$1").into_owned();
        if let Some(new_url) = fuzzy.get(&stripped) {
            return format!(r#"{attr}="{new_url}""#);
        }
        caps[0].to_string()
    }).into_owned()
}

struct ImportedPost {
    post_id: Uuid,
    author_matched: bool,
    skipped_status: bool,
}

async fn get_or_create_term(state: &AppState, site_id: Uuid, taxonomy: TaxonomyType, slug: &str, name: &str) -> Result<Taxonomy, String> {
    match crate::models::taxonomy::get_by_slug(&state.db, Some(site_id), slug, taxonomy.clone()).await {
        Ok(t) => Ok(t),
        Err(_) => {
            crate::models::taxonomy::create(&state.db, &CreateTaxonomy {
                site_id: Some(site_id),
                name: name.to_string(),
                slug: slug.to_string(),
                taxonomy,
                description: None,
            }).await.map_err(|e| format!("taxonomy create failed: {e}"))
        }
    }
}

/// Creates the post row with a unique slug, retrying with a numeric suffix
/// on a slug collision (agencies migrating real client sites will have
/// collisions — WP's own auto-uniquify does the same "-2", "-3" thing).
async fn create_post_unique_slug(state: &AppState, mut create: CreatePost) -> Result<crate::models::post::Post, String> {
    let base_slug = create.slug.clone().unwrap_or_else(|| crate::utils::slugify::slugify(&create.title));
    for attempt in 0..25 {
        create.slug = Some(if attempt == 0 { base_slug.clone() } else { format!("{}-{}", base_slug, attempt + 1) });
        match crate::models::post::create(&state.db, &create).await {
            Ok(post) => return Ok(post),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("duplicate key") || msg.contains("unique") {
                    continue;
                }
                return Err(format!("create failed: {e}"));
            }
        }
    }
    Err("could not find a unique slug after 25 attempts".to_string())
}

async fn import_post(
    state: &AppState,
    site_id: Uuid,
    admin_user_id: Uuid,
    item: &WxrItem,
    author_map: &HashMap<String, Uuid>,
    attachment_id_to_url: &HashMap<String, String>,
    media_map: &HashMap<String, Uuid>,
    exact_rewrite: &HashMap<String, String>,
    fuzzy_rewrite: &HashMap<String, String>,
) -> Result<ImportedPost, String> {
    let status = match item.status.as_str() {
        "publish" => PostStatus::Published,
        "draft" => PostStatus::Draft,
        "pending" => PostStatus::Pending,
        // WP "private" has no direct Synap equivalent (no password/visibility
        // gate is set automatically) — importing as Draft is the safer
        // default so nothing meant to be private goes live unreviewed.
        "private" => PostStatus::Draft,
        "future" => PostStatus::Scheduled,
        // trash / auto-draft / inherit (attachments' own status) / anything
        // unrecognized: not real content, skip.
        _ => {
            return Ok(ImportedPost { post_id: Uuid::nil(), author_matched: true, skipped_status: true });
        }
    };

    let post_type = if item.post_type == "page" { PostType::Page } else { PostType::Post };
    let published_at = parse_wp_datetime(&item.post_date, &item.post_date_gmt);

    // Author: dc:creator (a WP login) → the Synap user id resolved by
    // ensure_author_accounts (matched-by-email or freshly created). No
    // entry (no email on file, or no <wp:author> block at all) falls back
    // to the importing admin.
    let mut author_matched = true;
    let author_id = match author_map.get(&item.creator) {
        Some(&id) => id,
        None => { author_matched = false; admin_user_id }
    };

    // Featured image via _thumbnail_id → attachment's old URL → media_id.
    let featured_image_id = item.postmeta.iter()
        .find(|(k, _)| k == "_thumbnail_id")
        .and_then(|(_, v)| attachment_id_to_url.get(v.trim()))
        .and_then(|url| media_map.get(url))
        .copied();

    let content = rewrite_content_urls(&item.content, exact_rewrite, fuzzy_rewrite);
    let comments_enabled = item.comment_status != "closed";

    let create = CreatePost {
        site_id: Some(site_id),
        title: if item.title.trim().is_empty() { "(untitled)".to_string() } else { item.title.clone() },
        slug: if item.slug.trim().is_empty() { None } else { Some(item.slug.clone()) },
        content,
        content_format: None,
        excerpt: if item.excerpt.trim().is_empty() { None } else { Some(item.excerpt.clone()) },
        status: status.clone(),
        post_type,
        author_id,
        featured_image_id,
        published_at,
        template: None,
        post_password_hash: None,
        comments_enabled,
        parent_id: None, // patched in pass 3, once every item has a new UUID
        sources: vec![],
        sources_public: false,
    };

    // Re-running the same export (e.g. this time with a media zip attached,
    // to backfill images that failed the first time) updates the post this
    // WXR item became last time instead of creating a duplicate. The slug is
    // deliberately left untouched on update — it's a public URL, not
    // something a re-import should ever shift out from under existing links.
    let existing_post_id = crate::models::wp_import::find_post(&state.db, site_id, &item.wp_post_id)
        .await
        .unwrap_or(None);

    let post_id = match existing_post_id {
        Some(post_id) => {
            let update = UpdatePost {
                title: Some(create.title),
                slug: None,
                content: Some(create.content),
                content_format: None,
                excerpt: create.excerpt,
                status: Some(status),
                // None here means "leave unchanged" — a featured image that
                // resolved on a prior run is never regressed to unset just
                // because this run's export doesn't (re-)resolve it; it's
                // only ever filled in, never cleared, by a re-import.
                featured_image_id,
                clear_featured_image: false,
                published_at,
                template: None,
                clear_post_password: false,
                new_post_password_hash: None,
                comments_enabled: Some(comments_enabled),
                parent_id: None, // patched in pass 3, same as on first import
                sources: None,
                sources_public: None,
            };
            if let Err(e) = crate::models::post::update(&state.db, post_id, &update).await {
                return Err(format!("update failed: {e}"));
            }
            post_id
        }
        None => {
            let post = create_post_unique_slug(state, create).await?;
            if let Err(e) = crate::models::wp_import::record_post(&state.db, site_id, &item.wp_post_id, post.id).await {
                tracing::warn!("failed to record wp_import_post_map for wp_post_id={}: {:?}", item.wp_post_id, e);
            }
            post.id
        }
    };

    // Categories/tags — only WP's two built-in taxonomies map to anything
    // in Synap; any other <category domain="..."> (a custom taxonomy a
    // plugin registered) is skipped. `attach_to_post` is `ON CONFLICT DO
    // NOTHING`, so re-attaching on a re-import is harmless.
    for cat in &item.categories {
        let taxonomy = match cat.domain.as_str() {
            "category" => TaxonomyType::Category,
            "post_tag" => TaxonomyType::Tag,
            _ => continue,
        };
        let slug = if cat.nicename.trim().is_empty() { crate::utils::slugify::slugify(&cat.name) } else { cat.nicename.clone() };
        match get_or_create_term(state, site_id, taxonomy, &slug, &cat.name).await {
            Ok(term) => {
                if let Err(e) = crate::models::taxonomy::attach_to_post(&state.db, post_id, term.id).await {
                    tracing::warn!("failed to attach taxonomy '{}' to imported post {}: {:?}", cat.name, post_id, e);
                }
            }
            Err(e) => tracing::warn!("failed to resolve taxonomy '{}' for imported post {}: {}", cat.name, post_id, e),
        }
    }

    // Custom fields — copy every other postmeta key verbatim. `set_meta`
    // upserts by (post_id, meta_key), so a re-import just refreshes values.
    for (key, value) in &item.postmeta {
        if SKIP_META_KEYS.contains(&key.as_str()) || value.trim().is_empty() {
            continue;
        }
        if let Err(e) = crate::models::post::set_meta(&state.db, post_id, key, value).await {
            tracing::warn!("failed to set postmeta '{}' on imported post {}: {:?}", key, post_id, e);
        }
    }

    Ok(ImportedPost { post_id, author_matched, skipped_status: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WXR: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<rss version="2.0" xmlns:wp="http://wordpress.org/export/1.2/" xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:excerpt="http://wordpress.org/export/1.2/excerpt/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<channel>
<title>My Old Site</title>
<wp:author>
  <wp:author_login><![CDATA[jsmith]]></wp:author_login>
  <wp:author_email><![CDATA[jsmith@example.com]]></wp:author_email>
</wp:author>
<item>
  <title>A blog post</title>
  <content:encoded><![CDATA[<p>Hello <img src="https://old-site.example/wp-content/uploads/2024/03/sunset-photo-300x200.jpg"></p>]]></content:encoded>
  <excerpt:encoded><![CDATA[A short excerpt]]></excerpt:encoded>
  <dc:creator><![CDATA[jsmith]]></dc:creator>
  <wp:post_id>101</wp:post_id>
  <wp:post_date>2024-03-16 08:00:00</wp:post_date>
  <wp:post_date_gmt>2024-03-16 08:00:00</wp:post_date_gmt>
  <wp:post_type><![CDATA[post]]></wp:post_type>
  <wp:status><![CDATA[publish]]></wp:status>
  <wp:post_parent>0</wp:post_parent>
  <wp:post_name><![CDATA[a-blog-post]]></wp:post_name>
  <wp:comment_status><![CDATA[open]]></wp:comment_status>
  <category domain="category" nicename="news"><![CDATA[News]]></category>
  <category domain="post_tag" nicename="sunsets"><![CDATA[Sunsets]]></category>
  <wp:postmeta>
    <wp:meta_key>_thumbnail_id</wp:meta_key>
    <wp:meta_value>55</wp:meta_value>
  </wp:postmeta>
  <wp:postmeta>
    <wp:meta_key>_yoast_wpseo_metadesc</wp:meta_key>
    <wp:meta_value><![CDATA[A great description]]></wp:meta_value>
  </wp:postmeta>
</item>
<item>
  <title>Trashed draft</title>
  <wp:post_id>102</wp:post_id>
  <wp:post_type>post</wp:post_type>
  <wp:status>trash</wp:status>
</item>
<item>
  <title>sunset-photo</title>
  <wp:post_type>attachment</wp:post_type>
  <wp:post_id>55</wp:post_id>
  <wp:post_date>2024-03-15 09:30:00</wp:post_date>
  <wp:attachment_url><![CDATA[https://old-site.example/wp-content/uploads/2024/03/sunset-photo.jpg]]></wp:attachment_url>
</item>
</channel>
</rss>"#;

    #[test]
    fn parses_items_and_authors() {
        let (items, authors) = parse_wxr(SAMPLE_WXR);
        assert_eq!(items.len(), 3);
        assert_eq!(authors.get("jsmith").map(|s| s.as_str()), Some("jsmith@example.com"));
    }

    #[test]
    fn extracts_post_fields_categories_and_meta() {
        let (items, _) = parse_wxr(SAMPLE_WXR);
        let post = items.iter().find(|i| i.wp_post_id == "101").unwrap();
        assert_eq!(post.title, "A blog post");
        assert_eq!(post.post_type, "post");
        assert_eq!(post.status, "publish");
        assert_eq!(post.creator, "jsmith");
        assert_eq!(post.slug, "a-blog-post");
        assert!(post.content.contains("sunset-photo-300x200.jpg"));
        assert_eq!(post.categories.len(), 2);
        assert_eq!(post.categories[0].domain, "category");
        assert_eq!(post.categories[0].nicename, "news");
        assert_eq!(post.categories[1].domain, "post_tag");
        let thumb = post.postmeta.iter().find(|(k, _)| k == "_thumbnail_id").unwrap();
        assert_eq!(thumb.1, "55");
        let desc = post.postmeta.iter().find(|(k, _)| k == "_yoast_wpseo_metadesc").unwrap();
        assert_eq!(desc.1, "A great description");
    }

    #[test]
    fn parse_wp_datetime_prefers_gmt() {
        let dt = parse_wp_datetime("2024-03-16 08:00:00", "2024-03-16 08:00:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-03-16T08:00:00+00:00");
        assert!(parse_wp_datetime("", "").is_none());
        assert!(parse_wp_datetime("0000-00-00 00:00:00", "0000-00-00 00:00:00").is_none());
    }

    #[test]
    fn content_url_rewriting_strips_size_suffix() {
        let mut exact = HashMap::new();
        exact.insert(
            "https://old-site.example/wp-content/uploads/2024/03/sunset-photo.jpg".to_string(),
            "https://pong.com/uploads/sunset-photo-a1b2c3d4.jpg".to_string(),
        );
        let mut fuzzy = HashMap::new();
        fuzzy.insert(
            "https://old-site.example/wp-content/uploads/2024/03/sunset-photo.jpg".to_string(),
            "https://pong.com/uploads/sunset-photo-a1b2c3d4.jpg".to_string(),
        );
        let content = r#"<img src="https://old-site.example/wp-content/uploads/2024/03/sunset-photo-300x200.jpg">"#;
        let rewritten = rewrite_content_urls(content, &exact, &fuzzy);
        assert!(rewritten.contains("https://pong.com/uploads/sunset-photo-a1b2c3d4.jpg"));
        assert!(!rewritten.contains("old-site.example"));
    }

    #[test]
    fn content_url_rewriting_leaves_unmapped_urls_alone() {
        let exact = HashMap::new();
        let fuzzy = HashMap::new();
        let content = r#"<img src="https://external.example/not-imported.jpg">"#;
        let rewritten = rewrite_content_urls(content, &exact, &fuzzy);
        assert_eq!(rewritten, content);
    }

    #[test]
    fn percent_decode_handles_spaces() {
        assert_eq!(percent_decode("sunset%20photo.jpg"), "sunset photo.jpg");
        assert_eq!(percent_decode("no-escapes.png"), "no-escapes.png");
    }

    #[test]
    fn parse_year_month_rejects_malformed_dates() {
        assert_eq!(parse_year_month("2024-03-15 09:30:00"), Some("2024-03".to_string()));
        assert_eq!(parse_year_month(""), None);
        assert_eq!(parse_year_month("bogus"), None);
    }

    #[test]
    fn wp_upload_path_components_extracts_year_month_filename() {
        assert_eq!(
            wp_upload_path_components("https://old-site.example/wp-content/uploads/2024/03/Sunset%20Photo.jpg"),
            vec!["2024", "03", "sunset photo.jpg"],
        );
        // No wp-content/uploads marker (e.g. rewritten through a CDN) — falls
        // back to just the filename.
        assert_eq!(
            wp_upload_path_components("https://cdn.example/img/abc123/sunset-photo.jpg"),
            vec!["sunset-photo.jpg"],
        );
    }

    #[test]
    fn zip_media_index_matches_by_year_month_filename_regardless_of_nesting() {
        let index = ZipMediaIndex {
            entries: vec![
                (
                    vec!["wordpress".into(), "wp-content".into(), "uploads".into(), "2024".into(), "03".into(), "sunset-photo.jpg".into()],
                    b"deep-nested-bytes".to_vec(),
                ),
                (
                    vec!["2024".into(), "05".into(), "other.jpg".into()],
                    b"unrelated-bytes".to_vec(),
                ),
            ],
        };
        let found = index.find("https://old-site.example/wp-content/uploads/2024/03/sunset-photo.jpg");
        assert_eq!(found, Some(b"deep-nested-bytes".as_slice()));
    }

    #[test]
    fn zip_media_index_falls_back_to_unique_filename_match() {
        // Zip was flattened (no year/month subfolders) — the year/month
        // suffix won't match, but the filename alone is unambiguous.
        let index = ZipMediaIndex {
            entries: vec![(vec!["sunset-photo.jpg".into()], b"flat-bytes".to_vec())],
        };
        let found = index.find("https://old-site.example/wp-content/uploads/2024/03/sunset-photo.jpg");
        assert_eq!(found, Some(b"flat-bytes".as_slice()));
    }

    #[test]
    fn zip_media_index_refuses_ambiguous_filename_fallback() {
        let index = ZipMediaIndex {
            entries: vec![
                (vec!["2024".into(), "03".into(), "photo.jpg".into()], b"march".to_vec()),
                (vec!["2024".into(), "05".into(), "photo.jpg".into()], b"may".to_vec()),
            ],
        };
        // Neither entry's year/month tail matches "2024/07", and the
        // filename alone is ambiguous between the two — no match, not a
        // guess.
        let found = index.find("https://old-site.example/wp-content/uploads/2024/07/photo.jpg");
        assert_eq!(found, None);
    }

    #[test]
    fn guess_mime_from_extension_covers_common_types() {
        assert_eq!(guess_mime_from_extension("photo.JPG"), "image/jpeg");
        assert_eq!(guess_mime_from_extension("photo.png"), "image/png");
        assert_eq!(guess_mime_from_extension("clip.mp4"), "video/mp4");
        assert_eq!(guess_mime_from_extension("mystery.xyz"), "application/octet-stream");
    }

    #[test]
    fn is_public_ip_rejects_internal_ranges() {
        let internal = [
            "127.0.0.1",      // loopback
            "10.0.0.5",       // RFC1918
            "172.16.0.1",     // RFC1918
            "192.168.1.1",    // RFC1918
            "169.254.169.254", // link-local — cloud metadata endpoint
            "100.64.0.1",     // CGNAT
            "0.0.0.0",        // unspecified
            "::1",            // IPv6 loopback
            "fc00::1",        // IPv6 unique local
            "fe80::1",        // IPv6 link-local
            "::ffff:127.0.0.1", // IPv4-mapped IPv6 loopback
        ];
        for ip in internal {
            assert!(!is_public_ip(&ip.parse().unwrap()), "{ip} should be rejected");
        }
    }

    #[test]
    fn is_public_ip_accepts_public_ranges() {
        let public = ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"];
        for ip in public {
            assert!(is_public_ip(&ip.parse().unwrap()), "{ip} should be accepted");
        }
    }

    #[tokio::test]
    async fn is_safe_import_url_rejects_non_http_schemes_and_literal_internal_ips() {
        assert!(!is_safe_import_url("file:///etc/passwd").await);
        assert!(!is_safe_import_url("ftp://example.com/f.jpg").await);
        assert!(!is_safe_import_url("http://127.0.0.1/f.jpg").await);
        assert!(!is_safe_import_url("http://169.254.169.254/latest/meta-data/").await);
        assert!(!is_safe_import_url("not a url").await);
    }
}
