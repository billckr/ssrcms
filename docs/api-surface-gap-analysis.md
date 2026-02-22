# Plugin API Surface — Gap Analysis
*From building the SEO & Structured Data plugin (Phase 2)*

This document records every gap, workaround, and missing piece encountered while building
the SEO plugin. Each item is classified and linked to the resolution.

---

## Gaps Found

### G-01 — `get_posts()` and `get_terms()` functions were not implemented
**Severity:** Critical
**Discovered:** SEO plugin needed `get_posts()` for the sitemap and related post widgets need it too.
**Symptom:** Functions documented in plugin-api-v1.md §6 but not registered in the Tera engine.
**Resolution:** Implemented `GetPostsFunction` and `GetTermsFunction` as Tera `Function` impls
with DB access via `tokio::task::block_in_place`. See `core/src/templates/functions.rs`.
**Status:** Fixed in Phase 2.

### G-02 — Hook sentinel resolution used wrong context key
**Severity:** Critical
**Discovered:** Hooks never rendered any output in theme templates.
**Symptom:** `{{ hook(name="head_end") | safe }}` always produced empty string.
**Root cause:** `HookFunction::call()` returned `[[HOOK:__hook_output__head_end]]`. The resolver
regex captured `"head_end"` but then looked up context key `"head_end"` instead of
`"__hook_output__head_end"`.
**Resolution:** Fixed the context key lookup in `resolve_hook_sentinels()`.
See `core/src/templates/loader.rs`.
**Status:** Fixed in Phase 2.

### G-03 — Plugin-registered routes had no dispatcher
**Severity:** Major
**Discovered:** SEO plugin declares `/sitemap.xml` in `plugin.toml` but no route handler existed.
**Symptom:** Requests to `/sitemap.xml` returned 404.
**Resolution:** Added `handlers/plugin_route.rs` — a dispatcher that reads the plugin route
registry from `AppState`, fetches the appropriate data (all published posts + pages),
builds a context, and renders the plugin template. Plugin routes are registered in the Axum
router at startup from `AppState.plugin_routes`.
**Status:** Fixed in Phase 2.

### G-04 — Navigation menus always empty
**Severity:** Moderate
**Discovered:** `nav.primary.items` and `nav.footer.items` are always empty lists.
**Root cause:** `NavContext::default()` is used everywhere; no menus table or configuration
mechanism exists yet.
**Impact:** Theme templates that loop over `nav.primary.items` render nothing. This is
functional (no crash) but incomplete.
**Resolution (deferred to Phase 3):** Navigation menus require an admin UI to manage.
For Phase 1/2, themes should either use a hardcoded fallback or make nav optional.
The Phase 3 admin UI will populate the menus table.
**API change required:** Add a `menus` table and a `nav_menus` site setting, or allow plugins
to register nav items programmatically. Tracked as v1.1 API addition (additive, non-breaking).
**Status:** Documented; deferred.

### G-05 — `post.content` requires `| safe` — sanitization contract not explicit
**Severity:** Moderate (documentation gap)
**Discovered:** `single.html` uses `{{ post.content | safe }}` to render HTML content. Without
`| safe`, Tera auto-escapes the HTML making it render as literal tags.
**Root cause:** Post content is stored as raw HTML. Tera's auto-escape is correct for user
input, but post content from the admin editor is trusted HTML.
**Risk:** If content is not sanitized before storage, `| safe` creates an XSS vector.
**Resolution:** Document the contract explicitly: the core MUST sanitize post content with
`ammonia` before storing it. The admin editor (Phase 3) must run content through the sanitizer.
Template authors using `| safe` on `post.content` are relying on this contract.
Add a `sanitize_content()` call in the `create` and `update` model functions.
**Status:** Contract documented. Code enforcement tracked for Phase 3 admin editor.

### G-06 — `archive_author` context variable not in API surface doc
**Severity:** Minor (documentation gap)
**Discovered:** `archive.html` handler injects `archive_author` (a `UserContext`) for author
archives, but the API surface doc §4.4 only documents `archive_term`.
**Resolution:** Update plugin-api-v1.md §4.4 to document `archive_author`.
**Status:** Fixed in v1.1 doc update.

### G-07 — XML auto-escape affects JSON-LD output in SEO plugin
**Severity:** Minor
**Discovered:** The SEO plugin's `seo/meta.html` is registered as `.html` and Tera
auto-escapes it. The JSON-LD `<script>` block uses `{{ post.title | json_encode }}` which
is already JSON-escaped. Tera's HTML auto-escaping on top of that double-escapes quotes.
**Root cause:** Tera's `json_encode` built-in filter produces valid JSON with escaped quotes.
Then HTML auto-escape replaces `"` with `&quot;`, producing invalid JSON-LD.
**Resolution:** Use `{{ post.title | json_encode | safe }}` in JSON-LD blocks to prevent
double-escaping. Document this pattern in the plugin authoring guide.
**Status:** Fixed in SEO plugin template. Pattern documented.

### G-08 — No title override hook
**Severity:** Minor (API surface gap)
**Discovered:** The SEO plugin wants to control the `<title>` element format (e.g.
"Post Title | Site Name"). But the `<title>` tag is rendered directly in `base.html`
without a hook point.
**Current workaround:** Theme authors can override the `title` block via `{% block title %}`.
Plugins cannot override it — they can only inject content before/after via `head_start`/`head_end`.
**Proper resolution:** Add a `title_tag` hook, or change the architecture so the title is
computed in the context builder (where plugins can register a title formatter). Alternatively,
add a `page_title` context variable that plugins can influence by registering a Tera filter.
**Decision:** The cleanest approach for v1.1 is to add `page_title` to the global context
(pre-computed by the handler) and let plugins modify it via the `seo_title` meta field
(already supported). Theme templates should use `{{ page_title }}` instead of `{{ post.title }}`.
**Status:** Design decision pending. Tracked for v1.1.

---

## Summary of API Surface Changes for v1.1

| Change | Type | Breaking? |
|--------|------|-----------|
| Add `archive_author` to archive context doc | Doc only | No |
| Add `get_posts()` implementation | Feature | No |
| Add `get_terms()` implementation | Feature | No |
| Add plugin route dispatcher | Feature | No |
| Add `page_title` global context variable | Additive | No |
| Document `json_encode | safe` pattern for JSON-LD | Doc only | No |
| Document `post.content | safe` sanitization contract | Doc only | No |
| Nav menus: add menus table + context population | Feature | No (additive) |

No breaking changes. Version bump to **v1.1**.

---

*Gap analysis completed: 2026-02-21*
