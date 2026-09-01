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

`base.html`'s nav renders `nav.primary.items` (admin-configurable via the menu builder) with dropdown children, `target="_blank"` handling, `aria-current="page"` highlighting, a mobile hamburger + off-canvas drawer, and the logged-in user bar. A custom-styled nav (e.g. on a custom homepage) must still loop over `nav.primary.items` — never hardcode placeholder link labels/URLs. Check `theme_option_choices.nav_dropdown_trigger` (hover vs. click) if the theme declares it. If reskinning rather than reusing `base.html`'s nav markup directly, mirror its full feature set (dropdowns, mobile drawer, current-page state, user bar), not just the happy path.

### 7. Reveal/animation conventions

`base.html`'s shared script drives a generic one-shot scroll reveal off `[data-reveal]` (IntersectionObserver adds `.is-visible`): `<div data-reveal style="--d:0">` — the `--d` var feeds a stagger delay already defined in `style.css`. Any page extending `base.html` should use this rather than inventing a new mechanism. A fully custom homepage may run its own on-load cascade (e.g. a `.loaded` class from its own script) for its own hero/entrance choreography — but if it defines reusable component classes (a card grid, say) that get reused on *other* pages via this skill's "decide once, reuse everywhere" principle, remember those other pages only have `base.html`'s `[data-reveal]` system available, not the homepage's own script — give the reused class `data-reveal` there instead of relying on `.loaded`.

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

1. **Required templates.** `ls templates/` against the required set (Baseline §1). Build whatever's missing — `archive.html` first if absent, since category/tag/author links are probably already live and just 404ing or falling back oddly.
2. **Inline CSS.** Grep every template for `<style`. For each hit, check for Tera syntax inside it (`{{`/`{%`) — if none, migrate to `style.css` per Baseline §4, renaming/reusing customizer color vars where the value already matches (a pure rename, zero visual change) rather than leaving a parallel palette.
3. **Customizer wiring.** For every `[customizer.colors.*]`/`[customizer.options.*]` key in `theme.toml`, grep templates and `style.css` for actual usage. Remove declared-but-unused keys (dead cruft, often leftover from copying another theme as a starting point). Conversely, look for hardcoded values in templates that read like they *should* be user-configurable (an on/off feature, a color not already covered) and consider promoting them to a real option.
4. **Nav.** Confirm the nav loops over `nav.primary.items` rather than hardcoded links, on every template — a custom homepage's own nav markup is the most common place for this to have been frozen as placeholder content during design work.
5. **Post-display consistency.** Confirm every place posts appear uses the same card/list treatment (Baseline §5) — a design pass that only touched the homepage is a common half-finished state.
6. **`:root` sanity.** Exactly one `:root { ... }` block in `style.css`; every customizer-declared color present as a bare 6-digit hex on its own line inside it.
7. **Cache-busting.** Every `<link rel="stylesheet">` across every tier (`themes/global/`, `themes/private/`, every site copy) carries `?theme={{ site.theme }}`.

## Mode: New theme (optional `--url <reference>` / `-url <reference>`)

1. If a reference URL is given: fetch it (WebFetch) to extract the *visual identity* — palette, typography, layout rhythm, notable interactions/animation feel, overall mood — not literal copy or exact pixel layout. State back what you extracted before building, so it can be corrected before a lot of work goes into it.
2. Scaffold `theme.toml` with the customizer section from the start (Baseline §2–3) — don't bolt it on afterward. At minimum declare the site's core palette as customizer colors (background, panel/card background, header background, body text, muted text, primary accent + hover, border) so every theme ships with the same baseline configurability regardless of its specific design.
3. Build the required template set (Baseline §1), everything extending `base.html` except a deliberately custom homepage if the design calls for one.
4. Apply Baseline §4–7 from the start: one `style.css`, no inline styles, one shared post-card treatment reused everywhere, the real nav system, `[data-reveal]` for scroll-ins outside any custom homepage script.
5. Before calling it done, run the **Audit** mode above against your own output — a new theme should pass its own audit on the first try.
