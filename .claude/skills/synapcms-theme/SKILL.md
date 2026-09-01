---
name: synapcms-theme
description: Scaffold a brand-new SynapCMS theme (optionally from a --url design reference) or audit/complete an existing custom theme against the CMS's baseline requirements — required templates, the color/option customizer contract, CSS organization, post-display consistency, and the nav/menu system. Use whenever asked to create a new theme, convert a design into a theme, "finish"/"align"/"bring inline" an in-progress theme, or sanity-check a theme against what SynapCMS expects. Triggers: /synapcms-theme, "new theme", "audit this theme", "does this theme have everything it needs".
---

# SynapCMS Theme Baseline

Everything a theme needs to actually work with the CMS's systems (customizer, archives, caching), independent of whatever custom visual design it wears. Two modes below: **new theme** (optionally from a `--url` reference) and **audit** an existing one. Read the relevant mode, but the "Baseline Requirements" section applies to both — it's the thing being built toward or checked against.

## Baseline Requirements

### 1. Required templates

Every theme must have all of: `base.html`, `index.html`, `single.html`, `page.html`, `archive.html`, `search.html`, `404.html` under `templates/`. This is enforced by the app only at zip-upload time, so a hand-edited theme can silently be missing one — always verify by listing `templates/`, don't assume.

`archive.html` is the one most often skipped and the one that matters most: it's the *single* template that powers three already-existing routes — `/category/{slug}`, `/tag/{slug}`, `/author/{username}` (`core/src/handlers/archive.rs`). **Never add new backend routes for these — they already exist.** The handler passes `archive_type` (`"category"`/`"tag"`/`"author"`), `archive_term` (category/tag) or `archive_author` (author), `posts`, and `pagination`; branch the template on `archive_type`. There is deliberately no generic "all posts, no category/tag/author filter" route — don't invent a `/blog`-style route unless explicitly asked; the app's convention is that `index.html` *is* the paginated post list unless the theme deliberately repurposes it (see "Custom homepage" below).

Every other template extends `base.html`:
```
{% extends "base.html" %}
{% block title %}...{% endblock title %}
{% block content %}...{% endblock content %}
```

**`sitemap.xml` is a live route despite being outside this required set — don't skip it.** `GET /sitemap.xml` is unconditionally registered (`core/src/router.rs`) and renders `sitemap.xml` straight from the active theme; the zip-upload validator doesn't check for it, so a theme missing this file installs and activates fine and only 500s the first time something actually hits that URL.

**The "extra" `.html` files real themes ship (`contact-page.html`, `newsletter.html`, `subscribe-page.html`, `feed.html`, etc.) aren't reserved filenames with fixed routes — they're an auto-discovered, admin-selectable Page template system.** Any `.html` file directly under `templates/` (excluding `partials/`) whose name isn't one of the required set becomes a choice in the "Template" dropdown when an admin creates/edits a Page (`core/src/handlers/admin/posts.rs`, `scan_templates`); there's no fixed URL like `/contact` baked into the app — it only renders wherever an admin actually assigns it to a Page, under the Tera variable `page` (same as the default `page.html`, **not** `post`). The special name `"feed"` additionally gets a `posts` list (20 most recent) injected into context. Two sharp edges worth knowing before assuming these files matter:
- **`/subscribe` does not use `subscribe-page.html`.** It's 100%-hardcoded admin-panel Rust HTML (`admin/src/pages/subscribe.rs`), styled with the admin's own theme toggle, not the site's color customizer. A theme's `subscribe-page.html` only does anything if an admin manually assigns it to some other Page.
- **`503.html` is dead code.** Maintenance mode is also hardcoded Rust HTML (`core/src/middleware/maintenance.rs`) — there are zero references to it anywhere in `core/src`. Every real theme ships this file for no functional reason; it's fine to include for completeness but don't spend design effort on it.

Also: switching a site from a theme that has `contact-page.html`/similar to one that lacks matching filenames will 500 any existing Page whose stored `template` still points at the old name — a real caution when building a *replacement* theme for a site that already has content, not just a brand-new site.

### 2. The color customizer contract

A theme opts in with `[customizer] enabled = true` in `theme.toml`. Colors work completely differently from every other option type — get this exactly right:

- Declare with `[customizer.colors.{key}]` (plus a `label`) in `theme.toml`.
- `{key}` must correspond to a CSS custom property `--{key}` defined inside **the first (and only) `:root { ... }` block** in `static/css/style.css`.
- The value **must be a bare 6-digit hex** (`#RRGGBB`) **on its own line**. The admin's save/read/restore mechanism is regex-based against this exact shape — malformed values are silently ignored rather than erroring, so a typo just quietly doesn't work.
- Colors are **never** stored in the database. They're read live from `style.css` to populate the swatch inputs, and a save rewrites the hex value in place in that same file.
- "Restore original" for colors is a **separate mechanism** from every other option type: a `.bak` sibling of `style.css` is created on the *first* save and consumed (copied back, then deleted) on restore. It only ever holds the state from before the very first customizer-driven edit — editing the file directly (bypassing the admin UI, e.g. by hand during development) never touches `.bak` at all.

### 3. Other customizer option types

Declared under `[customizer.options.{key}]` with a `type`, `default`, `label`, and `group` (which customizer card it renders in — cards are grouped by first-seen `group` string, default `"Layout Options"`). These *are* DB-backed (`theme_options` table, per site+theme+key) and read in templates as:

| `type` | Tera access |
|---|---|
| `bool` | `{% if theme_options.key %}` |
| `order` (needs a `[customizer.options.key.items]` table) | `{% for item in theme_option_lists.key %}` |
| `choice` (needs a `[customizer.options.key.choices]` table) | `{{ theme_option_choices.key }}` |
| `text` | `{{ theme_option_texts.key }}` |
| `image` | `{{ theme_option_images.key }}` |

A theme feature an end user might reasonably want to toggle or restyle (a slider, a ticker, an accent color not already covered) should become one of these rather than staying hardcoded — that's the entire point of the customizer system: uniform, no app-level code needed per theme.

**`image` has no automatic fallback — a theme must handle the unset case itself.** `default_preview` is admin-picker-only (shown in the customizer's image picker before a real choice is made); it is *never* injected into the live Tera context, unlike a `bool`/`choice`/`text` default. Always guard with `{% if theme_option_images.key %}...{% else %}<the same path as default_preview, hardcoded>{% endif %}` — skip the `{% else %}` and an unset image option silently renders as nothing (no image, no placeholder, no error).

### 4. CSS lives in `static/css/style.css` — nothing inline unless it truly can't

Default assumption: **all** CSS belongs in `static/css/style.css`, linked once from `base.html`. Before leaving anything inline in a `<style>` block, check whether it actually contains Tera syntax (`{{`/`{%`) that requires server-side templating — if it doesn't (the common case), it can move to the shared file with zero behavior change, since CSS location never affects how rules apply.

Every stylesheet `<link>` must carry `?theme={{ site.theme }}`:
```html
<link rel="stylesheet" href="/theme/static/css/style.css?theme={{ site.theme }}">
```
This is the only cache-busting the URL has (see the caching gotchas below) — a plain `href="/theme/static/..."` silently reintroduces a stale-CSS-after-theme-switch bug. Check this on *every* tier a template can live in (global, private, and each site's own copy) since they're separate files.

**Custom homepage exception**: a theme's `index.html` can deliberately be a fully custom landing page instead of the paginated post list (a portfolio/case-study reel rather than a blog front page) — that's a legitimate design choice, not a gap. But it still shouldn't carry its own inline `<style>` or its own separate color/font variable names. Put its page-specific CSS in the same `style.css`, and if any of it touches bare selectors shared with every other page (`body`, `a`, `img`, `html`) in a way that would leak, scope those specific rules under a class on `<body>` (e.g. `body.flow-home { ... }`) rather than overriding the shared rule directly. Reuse the site's actual `--color-*`/`--font-*` customizer variables for anything that has an equivalent, rather than inventing a parallel palette — otherwise the color customizer silently won't affect that page.

### 5. Post display: decide once, reuse everywhere

Wherever posts are shown — the home page (curated or full list), `archive.html`, related-posts on `single.html`, search results — use the *same* card/list treatment and the same post-context fields, so the design reads as one system instead of "the homepage got attention and everything else didn't." Available fields on a post context object: `title`, `slug`, `url`, `excerpt`, `content`, `published_at`, `author` (display_name, url, ...), `categories`/`tags` (each: `name`, `slug`, `url`, `post_count`), `featured_image` (`url`, `alt_text`, `width`, `height`, optional — guard with `{% if post.featured_image %}`), `reading_time`, `comment_count`.

`GET /` already passes a full `posts` list (paginated per the site's `posts_per_page` setting) — a custom homepage that only wants a handful can just `{% for post in posts | slice(end=N) %}`; no backend change needed for that.

### 6. Nav: use the real menu system

First, confirm a nav exists at all. A fully custom page (a bespoke homepage, most often) can end up with **no `<nav>`/menu markup whatsoever** — not hardcoded placeholder links, just nothing — if the design pass focused on hero/visual content and nav got dropped entirely. That's a distinct, more severe failure from "nav is hardcoded" (below): it means a visitor landing on that page has no way to reach the rest of the site. Check for this explicitly, don't just check what an *existing* nav does.

Once a nav is confirmed present: `base.html`'s nav renders `nav.primary.items` (admin-configurable via the menu builder) with dropdown children, `target="_blank"` handling, `aria-current="page"` highlighting, a mobile hamburger + off-canvas drawer, and the logged-in user bar. A custom-styled nav (e.g. on a custom homepage) must still loop over `nav.primary.items` — never hardcode placeholder link labels/URLs. Check `theme_option_choices.nav_dropdown_trigger` (hover vs. click) if the theme declares it. If reskinning rather than reusing `base.html`'s nav markup directly, mirror its full feature set (dropdowns, mobile drawer, current-page state, user bar), not just the happy path.

Also check `[nav_locations]` in `theme.toml` (e.g. a declared `footer = "Footer Links"` location) against what templates actually render — a declared nav location an admin can build a menu for, but that no template ever loops over (`nav.footer.items`, etc.), is a silent dead end just like an unused customizer option.

### 7. Reveal/animation conventions

`base.html`'s shared script drives a generic one-shot scroll reveal off `[data-reveal]` (IntersectionObserver adds `.is-visible`): `<div data-reveal style="--d:0">` — the `--d` var feeds a stagger delay already defined in `style.css`. Any page extending `base.html` should use this rather than inventing a new mechanism. A fully custom homepage may run its own on-load cascade (e.g. a `.loaded` class from its own script) for its own hero/entrance choreography — but if it defines reusable component classes (a card grid, say) that get reused on *other* pages via this skill's "decide once, reuse everywhere" principle, remember those other pages only have `base.html`'s `[data-reveal]` system available, not the homepage's own script — give the reused class `data-reveal` there instead of relying on `.loaded`.

### 8. Plugin hook points — omitting one fails silently

`base.html` must call `{{ hook(name="...") }}` at all 7 points every content route passes to `render_hooks_for_theme`: `head_start`, `head_end`, `body_start`, `before_content`, `after_content`, `footer`, `body_end`. Installed plugins inject through these — e.g. the `seo` plugin writes meta tags/OG/JSON-LD/canonical at `head_end`. **There is no error or warning if a hook call is missing** — the plugin's output is just silently dropped, nothing renders, nothing logs. This is the single easiest thing to lose when writing `base.html` from scratch rather than starting from an existing theme's copy — double-check all 7 are present by name if doing that.

### 9. Comment form: an exact, undocumented-elsewhere contract

If `single.html` renders a comment form, it must match `core/src/handlers/comment.rs`'s `CommentForm` exactly: `POST` to `/{{ post.slug }}/comment`, a required field named **`body`** (not `content` — an easy wrong guess), an optional hidden `parent_id` (empty string for a top-level comment, the parent comment's id for a reply), and a required checkbox named `human_check` (a real "I'm human" confirmation the user must check — not a hidden honeypot). Submitting requires an authenticated session; an unauthenticated POST redirects to login rather than erroring. `post.comment_count`/`post.comments_enabled` are available on the post context for gating whether to show the form/count at all.

### 10. The Puck visual page builder is fully theme-independent

When a site has an active builder composition for the homepage or archive (`page_composition::get_homepage`/`get_archive_template`, checked before `index.html`/`archive.html` ever renders), the response is a self-contained document the composer generates itself — it never extends `base.html`, never links the theme's `style.css`, never touches the theme's nav. A theme needs to do nothing to support this and nothing in a theme can affect how a builder-composed page looks — worth knowing so no effort gets spent "supporting" it.

### 11. Recommended, not required

A cookie-consent banner (`#cookie-banner` + a `cookieConsent()` handler, present in every real theme's `base.html`) is pure theme-author boilerplate — the app never reads or depends on it existing anywhere in `core/src`. Including one is a reasonable default for a real deployment (and matches what every existing theme already does, so it's expected for consistency), but it's a courtesy for site owners, not something the CMS itself checks for or relies on — don't treat its absence as a defect the way a missing required template would be.

## Workflow gotchas — read before touching any theme file

**1. Site-copy sync.** Once a site activates a theme, it gets its own independent copy at `sites/{site_id}/themes/{name}/` — editing `themes/global/{name}/...` (or `themes/private/{name}/...`) has **no effect** on a site that already has its own copy. After every edit to a global/private theme file, find and mirror the change into every site copy of that theme too:
```bash
find /path/to/sites -maxdepth 3 -path "*/themes/<theme-name>"
```
Use `command cp -f` (not bare `cp`) if `cp` is aliased to `-i` in the shell — an interactive prompt with no stdin will just silently no-op the copy.

**2. Template engine cache.** The backend caches parsed Tera templates in memory, keyed per (theme directory, site). Editing an `.html` file on disk directly (as opposed to through the admin UI's own save, which explicitly invalidates the cache) is **not picked up** until the cache entry is cleared — restart the app (`./app.sh restart`, or the project's equivalent) after every template edit before verifying.

**3. Static asset browser cache.** `/theme/static/*` (CSS/JS/images) is served fresh from disk on every request server-side (no server cache) but with `Cache-Control: max-age=300` — the URL itself carries no content-hash cache-busting beyond `?theme=<name>`, so a browser can serve a stale copy for up to 5 minutes after a real fix is live. Always verify a fix with `curl` against the actual served endpoint (and check `Content-Type`/status/grep the body) rather than trusting a browser screenshot taken immediately after the change — and tell whoever's watching in a browser that they may need to hard-refresh or just wait.

**Standard loop for every theme file change**: edit → sync to every site copy → restart the app → `curl`-verify the live response (right status code, right content, `grep` the log for Tera errors) → only then report done.

## Mode: Audit an existing theme

Work through in order; fix as you go, syncing/restarting/verifying after each fix per the workflow above.

1. **Required templates.** `ls templates/` against the required set (Baseline §1). Build whatever's missing — `archive.html` first if absent, since category/tag/author links are probably already live and just 404ing or falling back oddly. Also confirm `sitemap.xml` exists (a live route despite being outside the required set — Baseline §1) and, if the theme ships any of the "extra" auto-discovered Page templates (`contact-page.html` etc.), sanity-check them against the caveats in Baseline §1 (`/subscribe` doesn't use `subscribe-page.html`; `503.html` is dead code either way).
2. **Inline CSS.** Grep every template for `<style`. For each hit, check for Tera syntax inside it (`{{`/`{%`) — if none, migrate to `style.css` per Baseline §4, renaming/reusing customizer color vars where the value already matches (a pure rename, zero visual change) rather than leaving a parallel palette.
3. **Customizer wiring.** For every `[customizer.colors.*]`/`[customizer.options.*]` key in `theme.toml`, grep templates and `style.css` for actual usage. Remove declared-but-unused keys (dead cruft, often leftover from copying another theme as a starting point). Conversely, look for hardcoded values in templates that read like they *should* be user-configurable (an on/off feature, a color not already covered) and consider promoting them to a real option. If any key is `type = "image"`, confirm the template guards the unset case (Baseline §3) rather than assuming `default_preview` renders automatically.
4. **Nav.** First confirm a `<nav>`/menu exists at all on every template, especially a custom homepage — check for absence before checking behavior, since a design pass can drop nav entirely rather than just freezing it as hardcoded placeholder links. Once present, confirm it loops over `nav.primary.items` rather than hardcoded links. Also check any other declared `[nav_locations]` (e.g. a footer menu) actually gets rendered somewhere (`nav.footer.items`, etc.) — a declared location nothing reads is a dead end same as an unused customizer option.
5. **Post-display consistency.** Confirm every place posts appear uses the same card/list treatment (Baseline §5) — a design pass that only touched the homepage is a common half-finished state.
6. **`:root` sanity.** Exactly one `:root { ... }` block in `style.css`; every customizer-declared color present as a bare 6-digit hex on its own line inside it.
7. **Cache-busting.** Every `<link rel="stylesheet">` across every tier (`themes/global/`, `themes/private/`, every site copy) carries `?theme={{ site.theme }}`.
8. **Plugin hooks.** Grep `base.html` for `hook(name=` and confirm all 7 points from Baseline §8 are present — a missing one has no error to find it by, only a plugin whose output silently never shows up.
9. **Comment form.** If `single.html` renders a comment form, check its field names against the exact contract in Baseline §9 (`body`, `parent_id`, `human_check`) — a mismatched field name fails silently from the theme's side (the POST just doesn't do what's expected) rather than raising an error in the template itself.

## Mode: New theme (optional `--url <reference>` / `-url <reference>`)

1. If a reference URL is given: fetch it (WebFetch) to extract the *visual identity* — palette, typography, layout rhythm, notable interactions/animation feel, overall mood — not literal copy or exact pixel layout. State back what you extracted before building, so it can be corrected before a lot of work goes into it.
2. Scaffold `theme.toml` with the customizer section from the start (Baseline §2–3) — don't bolt it on afterward. At minimum declare the site's core palette as customizer colors (background, panel/card background, header background, body text, muted text, primary accent + hover, border) so every theme ships with the same baseline configurability regardless of its specific design.
3. Build the required template set (Baseline §1) plus `sitemap.xml`, everything extending `base.html` except a deliberately custom homepage if the design calls for one. Include all 7 plugin hook points (Baseline §8) in `base.html` from the start — easiest to get right by copying an existing theme's `base.html` `<head>`/`<body>` skeleton rather than writing it from scratch.
4. Apply Baseline §4–7 and §9–11 from the start: one `style.css`, no inline styles, one shared post-card treatment reused everywhere, the real nav system, `[data-reveal]` for scroll-ins outside any custom homepage script, the correct comment-form field names if `single.html` has one, and a cookie-consent banner (recommended, not required — Baseline §11).
5. Before calling it done, run the **Audit** mode above against your own output — a new theme should pass its own audit on the first try.
