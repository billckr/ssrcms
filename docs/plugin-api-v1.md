# Synaptic Signals Plugin API — v1.1

> **Status:** v1.1 — updated based on SEO plugin build gap analysis. See `docs/api-surface-gap-analysis.md`.
> **Stability guarantee:** Context variable names, types, and hook names in this document are stable.
> Breaking changes require a major version bump and a migration guide.
>
> **Changes from v1.0 → v1.1 (all additive, non-breaking):**
> - `get_posts()` and `get_terms()` functions now implemented (were documented-only in v1.0)
> - `archive_author` variable documented in §4.4
> - `page_title` global context variable added (§3)
> - JSON-LD `json_encode | safe` pattern documented (§5)
> - Plugin route dispatch fully implemented

---

## Cardinal Security Rule

> **User-supplied content (post titles, comments, custom field values) MUST always enter templates
> as context variables. They must NEVER be rendered as Tera template source strings.**
>
> Rendering user content as a template string is a template injection vulnerability.
> This rule is non-negotiable and must be enforced in every code review.

---

## Table of Contents

1. [Plugin Manifest (`plugin.toml`)](#1-plugin-manifest-plugintoml)
2. [Theme Manifest (`theme.toml`)](#2-theme-manifest-themetoml)
3. [Template Context — All Types](#3-template-context--all-types)
4. [Per-Template-Type Context](#4-per-template-type-context)
5. [Tera Filters](#5-tera-filters)
6. [Tera Functions](#6-tera-functions)
7. [Hook System](#7-hook-system)
8. [Plugin-Registered Routes](#8-plugin-registered-routes)
9. [Custom Fields](#9-custom-fields)
10. [Security Boundaries](#10-security-boundaries)
11. [Versioning Policy](#11-versioning-policy)

---

## 1. Plugin Manifest (`plugin.toml`)

Every plugin directory must contain a `plugin.toml` manifest at its root.

```toml
[plugin]
name        = "seo"
version     = "1.0.0"
api_version = "1"
description = "SEO meta tags, canonical URLs, JSON-LD, and XML sitemap."
author      = "Your Name <you@example.com>"

# Hooks this plugin registers. Each entry maps a hook name to a template partial.
[hooks]
head_end = "seo/meta.html"

# Custom metadata fields this plugin declares.
# These become accessible as post.meta.<key> in templates.
[meta_fields]
seo_title       = { label = "SEO Title", type = "text", description = "Override the page <title>" }
seo_description = { label = "Meta Description", type = "textarea", description = "Override the meta description" }
seo_noindex     = { label = "No Index", type = "boolean", description = "Prevent search engine indexing" }
```

### Plugin directory layout

```
plugins/
  seo/
    plugin.toml
    seo/
      meta.html
      canonical.html
      jsonld.html
    static/
      (plugin static assets, served at /plugins/seo/static/)
```

---

## 2. Theme Manifest (`theme.toml`)

Every theme directory must contain a `theme.toml` manifest and the required template files.

```toml
[theme]
name        = "default"
version     = "1.0.0"
api_version = "1"
description = "Minimal default theme."
author      = "Synaptic Signals"
```

### Required template files

| File             | Purpose                               |
|------------------|---------------------------------------|
| `base.html`      | Base layout template (all pages extend this) |
| `index.html`     | Home page                             |
| `single.html`    | Single post                           |
| `page.html`      | Single page (static content)          |
| `archive.html`   | Category / tag / author archive       |
| `search.html`    | Search results                        |
| `404.html`       | Not found                             |

### Theme directory layout

```
themes/
  default/
    theme.toml
    templates/
      base.html
      index.html
      single.html
      page.html
      archive.html
      search.html
      404.html
      partials/
        (optional shared partials)
    static/
      (CSS, JS, images — served at /theme/static/)
```

---

## 3. Template Context — All Types

These variables are available in **every** template, regardless of type.

| Variable | Type | Description | Example |
|----------|------|-------------|---------|
| `site.name` | string | Site display name | `"Acme Blog"` |
| `site.description` | string | Site tagline / description | `"Thoughts on Rust"` |
| `site.url` | string | Canonical base URL (no trailing slash) | `"https://example.com"` |
| `site.language` | string | BCP-47 language code | `"en-US"` |
| `site.theme` | string | Active theme name | `"default"` |
| `site.post_count` | integer | Count of published posts | `142` |
| `site.page_count` | integer | Count of published pages | `8` |
| `request.url` | string | Full request URL | `"https://example.com/blog/my-post"` |
| `request.path` | string | Request path | `"/blog/my-post"` |
| `request.query` | map | Query string parameters | `{ "page": "2" }` |
| `session.is_logged_in` | bool | Whether the user has an active session | `false` |
| `session.user` | User \| null | Logged-in user (null if anonymous) | see User type |
| `nav.primary` | NavMenu | Primary navigation menu | see NavMenu type |
| `nav.footer` | NavMenu | Footer navigation menu | see NavMenu type |
| `pagination` | Pagination \| null | Pagination data if applicable | see Pagination type |
| `page_title` | string | Pre-computed page title (post title, archive name, or site name) | `"Hello World"` |

### User type

| Field | Type | Description |
|-------|------|-------------|
| `user.id` | UUID string | User identifier |
| `user.username` | string | Login username |
| `user.display_name` | string | Public display name |
| `user.role` | string | `"admin"`, `"editor"`, `"author"`, `"subscriber"` |

### NavMenu type

| Field | Type | Description |
|-------|------|-------------|
| `menu.items` | list of NavItem | Ordered list of menu items |

### NavItem type

| Field | Type | Description |
|-------|------|-------------|
| `item.label` | string | Display label |
| `item.url` | string | Target URL |
| `item.target` | string | Link target (`"_self"`, `"_blank"`) |
| `item.is_current` | bool | True if this item matches the current URL |
| `item.children` | list of NavItem | Sub-items (for dropdown menus) |

### Pagination type

| Field | Type | Description |
|-------|------|-------------|
| `pagination.current_page` | integer | Current page number (1-indexed) |
| `pagination.total_pages` | integer | Total number of pages |
| `pagination.per_page` | integer | Items per page |
| `pagination.total_items` | integer | Total item count |
| `pagination.prev_url` | string \| null | URL of previous page |
| `pagination.next_url` | string \| null | URL of next page |

---

## 4. Per-Template-Type Context

### 4.1 Home (`index.html`)

Additional variables beyond the global context:

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list of Post | Paginated list of published posts, newest first |
| `featured_post` | Post \| null | Pinned/featured post if any |

### 4.2 Single Post (`single.html`)

| Variable | Type | Description |
|----------|------|-------------|
| `post` | Post | The current post |
| `related_posts` | list of Post | Up to 5 posts sharing a taxonomy term |
| `prev_post` | Post \| null | Chronologically previous published post |
| `next_post` | Post \| null | Chronologically next published post |

### 4.3 Single Page (`page.html`)

| Variable | Type | Description |
|----------|------|-------------|
| `page` | Post | The current page (post_type = "page") |

### 4.4 Archive (`archive.html`)

| Variable | Type | Description |
|----------|------|-------------|
| `archive_type` | string | `"category"`, `"tag"`, `"author"`, `"date"` |
| `archive_term` | Term \| null | The taxonomy term (category or tag) for this archive; null for author archives |
| `archive_author` | Author \| null | The author for author archives (`archive_type = "author"`); null for taxonomy archives |
| `posts` | list of Post | Paginated posts in this archive |

### 4.5 Search Results (`search.html`)

| Variable | Type | Description |
|----------|------|-------------|
| `query` | string | The search query string |
| `results` | list of Post | Matching posts |
| `result_count` | integer | Total number of matches |

### 4.6 404 (`404.html`)

No additional variables beyond the global context. The `request.path` variable indicates what was not found.

---

### Post type

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `post.id` | UUID string | Post identifier | `"550e8400-..."` |
| `post.title` | string | Post title | `"Hello World"` |
| `post.slug` | string | URL slug | `"hello-world"` |
| `post.content` | string | Rendered HTML content | `"<p>...</p>"` |
| `post.excerpt` | string | Auto-generated or manual excerpt | `"First 55 words..."` |
| `post.status` | string | `"published"`, `"draft"`, `"scheduled"`, `"trashed"` | `"published"` |
| `post.post_type` | string | `"post"` or `"page"` | `"post"` |
| `post.url` | string | Canonical URL for this post | `"https://example.com/blog/hello-world"` |
| `post.published_at` | string | ISO 8601 publish datetime | `"2026-01-15T09:00:00Z"` |
| `post.updated_at` | string | ISO 8601 last-updated datetime | `"2026-01-20T14:30:00Z"` |
| `post.author` | Author | Post author | see Author type |
| `post.categories` | list of Term | Assigned categories | see Term type |
| `post.tags` | list of Term | Assigned tags | see Term type |
| `post.featured_image` | Media \| null | Featured image | see Media type |
| `post.reading_time` | integer | Estimated reading time in minutes | `4` |
| `post.comment_count` | integer | Number of approved comments | `12` |
| `post.meta` | map | Plugin-registered custom fields | `{ "seo_title": "Custom Title" }` |

### Author type

| Field | Type | Description |
|-------|------|-------------|
| `author.id` | UUID string | Author identifier |
| `author.username` | string | Username |
| `author.display_name` | string | Public display name |
| `author.bio` | string | Author bio / description |
| `author.url` | string | Author archive URL |
| `author.avatar_url` | string \| null | Avatar image URL |

### Term type (category or tag)

| Field | Type | Description |
|-------|------|-------------|
| `term.id` | UUID string | Term identifier |
| `term.name` | string | Display name |
| `term.slug` | string | URL slug |
| `term.taxonomy` | string | `"category"` or `"tag"` |
| `term.url` | string | Archive URL for this term |
| `term.post_count` | integer | Number of published posts with this term |

### Media type

| Field | Type | Description |
|-------|------|-------------|
| `media.id` | UUID string | Media identifier |
| `media.url` | string | Full URL to the file |
| `media.filename` | string | Original filename |
| `media.mime_type` | string | MIME type |
| `media.alt_text` | string | Accessibility alt text |
| `media.width` | integer \| null | Image width in pixels |
| `media.height` | integer \| null | Image height in pixels |

---

## 5. Tera Filters

Filters are called with the pipe syntax: `{{ value | filter_name(arg=value) }}`.

| Filter | Input | Args | Output | Description |
|--------|-------|------|--------|-------------|
| `date_format` | string (ISO 8601) | `format`: strftime string | string | Format a datetime string. Default: `"%B %-d, %Y"` |
| `excerpt` | string (HTML) | `words`: integer (default 55) | string | Strip HTML and truncate to N words, append `"..."` |
| `strip_html` | string | — | string | Remove all HTML tags |
| `reading_time` | string (HTML or plain) | `wpm`: integer (default 200) | integer | Estimate reading time in minutes (minimum 1) |
| `slugify` | string | — | string | Convert to URL-safe slug |
| `truncate_words` | string | `count`: integer | string | Truncate to N words |
| `absolute_url` | string (path) | — | string | Prepend `site.url` to a relative path |

Tera also provides the built-in `json_encode` filter. When used inside `<script type="application/ld+json">` blocks, combine it with `| safe` to prevent double-escaping:
```html
"headline": {{ post.title | json_encode | safe }}
```

### Examples

```html
{# Format a date #}
{{ post.published_at | date_format(format="%B %-d, %Y") }}

{# Generate an excerpt #}
{{ post.content | excerpt(words=30) }}

{# Reading time #}
{{ post.content | reading_time }} min read

{# Absolute URL from path #}
{{ "/blog/hello" | absolute_url }}
```

---

## 6. Tera Functions

Functions are called like: `{{ function_name(arg=value) }}`.

| Function | Args | Return type | Description |
|----------|------|-------------|-------------|
| `hook(name)` | `name`: string | string (HTML) | Invoke all partials registered for this hook point. Returns concatenated HTML output. |
| `get_posts(...)` | see below | list of Post | Query posts with filters |
| `get_terms(taxonomy)` | `taxonomy`: `"category"` or `"tag"` | list of Term | Fetch all terms of a taxonomy |
| `url_for(type, slug)` | `type`: string, `slug`: string | string | Generate a canonical URL for a given resource type and slug |

### `get_posts` arguments

| Arg | Type | Default | Description |
|-----|------|---------|-------------|
| `post_type` | string | `"post"` | `"post"` or `"page"` |
| `status` | string | `"published"` | Post status filter |
| `category` | string | — | Filter by category slug |
| `tag` | string | — | Filter by tag slug |
| `author` | string | — | Filter by author username |
| `limit` | integer | 10 | Maximum results |
| `offset` | integer | 0 | Skip N results |
| `order_by` | string | `"published_at"` | Field to order by |
| `order` | string | `"desc"` | `"asc"` or `"desc"` |

### Examples

```html
{# Invoke all partials registered for head_end #}
{{ hook(name="head_end") }}

{# Fetch 5 most recent posts in "news" category #}
{% set news = get_posts(category="news", limit=5) %}
{% for post in news %}
  <a href="{{ post.url }}">{{ post.title }}</a>
{% endfor %}

{# Generate URL for a category archive #}
{{ url_for(type="category", slug="rust") }}
```

---

## 7. Hook System

Hooks are named injection points in the template rendering pipeline. A plugin registers a template partial for a hook in its `plugin.toml` manifest. When the `hook()` function is called in a template, all registered partials fire in registration order (alphabetical by plugin name), and their HTML output is concatenated.

### Available hook points

| Hook name | Location | Typical use |
|-----------|----------|-------------|
| `head_start` | Inside `<head>`, before anything | High-priority head tags |
| `head_end` | Inside `<head>`, before `</head>` | Meta tags, scripts, styles |
| `body_start` | Inside `<body>`, before any content | Skip navigation, analytics |
| `body_end` | Inside `<body>`, before `</body>` | Scripts, chat widgets |
| `before_content` | Before main content area | Breadcrumbs, notices |
| `after_content` | After main content area | Related posts, comments |
| `footer` | Inside footer | Footer widgets, copyright |

### Hook partial context

When a hook partial is rendered, it receives the full template context of the calling template. A hook partial registered on a single post page has access to `post`, `site`, `session`, etc.

### Registering a hook in `plugin.toml`

```toml
[hooks]
head_end = "seo/meta.html"
# Multiple hooks:
# after_content = "related/widget.html"
# footer = "analytics/tracker.html"
```

### Calling a hook in a theme template

```html
<!-- In base.html -->
<head>
  {{ hook(name="head_start") }}
  <title>{{ site.name }}</title>
  {{ hook(name="head_end") }}
</head>
```

---

## 8. Plugin-Registered Routes

Plugins can register custom HTTP routes (e.g. `/sitemap.xml`) by declaring them in `plugin.toml`. Plugin routes are handled by a Rust dispatcher in the core; the plugin provides a Tera template that renders the response body.

```toml
[routes]
"/sitemap.xml" = { template = "seo/sitemap.xml", content_type = "application/xml" }
```

The route template receives the standard global context plus a `route_context` variable populated by the core dispatcher (e.g. all published posts for a sitemap).

> **Note:** Plugin-registered routes are presentation-only. They render a template with data provided by the core. Plugins cannot implement arbitrary route handlers.

---

## 9. Custom Fields

Plugins declare custom fields in their manifest. These fields are stored in the `post_meta` table and are accessible in templates as `post.meta.<key>`.

```toml
[meta_fields]
seo_title = { label = "SEO Title", type = "text" }
```

### Supported field types

| Type | Template value | Description |
|------|---------------|-------------|
| `text` | string | Single-line text |
| `textarea` | string | Multi-line text |
| `boolean` | bool | True/false flag |
| `integer` | integer | Whole number |
| `url` | string | URL (validated on input) |
| `image` | Media or null | Reference to a media item |

### Accessing custom fields in templates

```html
{# With fallback to empty string #}
{% set seo_title = post.meta.seo_title | default(value="") %}
{% if seo_title %}
  <title>{{ seo_title }} | {{ site.name }}</title>
{% else %}
  <title>{{ post.title }} | {{ site.name }}</title>
{% endif %}

{# Boolean field #}
{% if post.meta.seo_noindex %}
  <meta name="robots" content="noindex, nofollow">
{% endif %}
```

---

## 10. Security Boundaries

### What the Tera structural sandbox prevents

The following are structurally impossible in a Tera template — there is no mechanism in the language to do them:

- Execute operating system commands
- Access the database directly
- Make HTTP or network requests
- Read files from the server filesystem (outside of template rendering)
- Write files to the server filesystem
- Read environment variables or application secrets
- Install persistent code or backdoors

### Context discipline (host responsibility)

The security of the system depends on what the Rust core puts into the context. The core MUST NOT expose:

- Password hashes
- Session tokens or secret keys
- API credentials or private configuration
- Any field not explicitly documented in this API surface

Any data placed in the context is potentially accessible to theme and plugin templates.

### Template injection prevention

User-supplied content (post title, content, author name, comment text, custom field values) MUST be passed to templates as context variables and escaped with Tera's default auto-escaping. They must never be passed to `tera.render_str()` or any function that treats the value as template source.

---

## 11. Versioning Policy

- Plugins and themes declare `api_version = "1"` in their manifest.
- The core supports all versions it has ever shipped. Old plugins do not break when the core is updated.
- New variables may be added to any context without a version bump (additive changes are safe).
- Renaming, removing, or changing the type of an existing variable is a breaking change and requires a major version bump.
- New hook names, filters, and functions are additive and do not require a version bump.
- Removing a hook, filter, or function requires a major version bump.

---

*Synaptic Signals Plugin API v1.0 — last updated 2026-02-21*
