# Synaptic Signals — Theme Authoring Guide

> **API version:** 1
> **Last updated:** 2026-02-22

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Prerequisites](#2-prerequisites)
3. [Theme Directory Structure](#3-theme-directory-structure)
4. [The theme.toml Manifest](#4-the-themetoml-manifest)
5. [Required Template Files](#5-required-template-files)
6. [Template Inheritance](#6-template-inheritance)
7. [Hook Points](#7-hook-points)
8. [Tera Filters](#8-tera-filters)
9. [Tera Functions](#9-tera-functions)
10. [Pagination](#10-pagination)
11. [Navigation Menus](#11-navigation-menus)
12. [Static Assets](#12-static-assets)
13. [Testing Your Theme](#13-testing-your-theme)
14. [Theme Authoring Checklist](#14-theme-authoring-checklist)

---

## 1. Introduction

In Synaptic Signals, **themes control layout and presentation**. A theme decides how content looks: the HTML structure, typography, navigation chrome, header, footer, and how posts, pages, and archives are rendered to the browser.

**Plugins add functionality.** A plugin might inject SEO meta tags into the document head, add a comment system after post content, or register a sitemap route. Plugins do not own layout; they participate in it through named hook points that themes expose.

This separation is intentional. A theme author does not need to know which plugins are installed. A plugin author does not need to fork a theme to get their output into the page. They communicate through the hook system described in Section 7.

Themes are written as [Tera](https://keats.github.io/tera/) templates — a Jinja2-compatible templating language compiled and rendered by the Rust core. There is no build step, no Node.js, no transpilation. You write HTML files with `{{ }}` expressions and `{% %}` control tags, drop the directory in the right place, and the CMS picks them up.

---

## 2. Prerequisites

You need working knowledge of the following before writing a theme:

**Jinja2 / Twig / Tera template syntax**

Tera's syntax is a strict subset of Jinja2. If you have used Jinja2 (Python), Twig (PHP), or Django templates, you already know 95% of what you need. Key syntax points:

- `{{ expression }}` — output a value
- `{% tag %}` — control flow: `if`, `for`, `block`, `extends`, `include`, `set`
- `{# comment #}` — comment (not rendered to output)
- `{{ value | filter }}` — apply a filter to a value
- `{{ function(arg=value) }}` — call a function
- Auto-escaping is enabled for all `.html` files. Output is HTML-escaped by default. Use `| safe` only when the value is already trusted, sanitized HTML produced by the CMS core (never user-supplied raw strings).

**HTML and CSS**

Themes produce HTML documents. You are responsible for the full document structure, including the `<html>`, `<head>`, and `<body>` elements in your base template. There is no opinion about CSS methodology; bring whatever you prefer (plain CSS, a utility framework, a preprocessed stylesheet compiled separately).

**What you do not need**

You do not need Rust, JavaScript, a build pipeline, or any server-side programming knowledge to write a theme. The Tera sandbox intentionally prevents templates from executing code, accessing the filesystem, or making network requests.

---

## 3. Theme Directory Structure

Themes are stored under the configured `themes/` folder, organised into two subdirectories:

| Directory | Purpose | Who can upload |
|-----------|---------|----------------|
| `themes/global/` | Available to all sites | Super Admin only |
| `themes/sites/<site_id>/` | Scoped to one specific site | Site Admin (or Super Admin) |

On first server startup, any themes that previously lived in the flat `themes/` root are
automatically migrated to `themes/global/`.

Within either subdirectory, each theme has its own named folder. The directory name becomes
the theme's identifier and must match the `name` field in `theme.toml`.

```
themes/global/
  your-theme-name/
    theme.toml                   ← required: manifest
    screenshot.png               ← optional: preview image (1200×900 recommended)
    templates/
      base.html                  ← required
      index.html                 ← required
      single.html                ← required
      page.html                  ← required
      archive.html               ← required
      search.html                ← required
      404.html                   ← required
      partials/                  ← optional: shared template fragments
        post-card.html
        author-bio.html
        pagination.html
    static/
      css/
        style.css
      js/
        main.js
      images/
        logo.svg
```

**Required files**

Every theme must supply all seven template files listed above. The CMS will refuse to activate a theme that is missing any of them.

**The `partials/` directory**

Partials are optional Tera templates that you include from other templates using `{% include "partials/post-card.html" %}`. They are not called by the CMS directly — they only exist to help you avoid repeating markup. There are no required partials.

**The `static/` directory**

Any file placed under `static/` is served verbatim at the URL path `/theme/static/<path-relative-to-static>`. For example, `static/css/style.css` is served at `/theme/static/css/style.css`. The directory structure is preserved. See Section 12 for details.

---

## 4. The theme.toml Manifest

Every theme must have a `theme.toml` file at its root. The manifest identifies the theme to the CMS and declares which API version it was written against.

```toml
[theme]
name        = "your-theme-name"
version     = "1.0.0"
api_version = "1"
description = "A short description of your theme."
author      = "Your Name <you@example.com>"
```

**Field reference**

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Must match the directory name exactly. Used as the theme identifier. |
| `version` | yes | Semantic version string for your theme. |
| `api_version` | yes | The Plugin API version this theme targets. Use `"1"` for the current API. |
| `description` | yes | One-line description shown in the admin theme browser. |
| `author` | yes | Author name and optional email in `Name <email>` format. |

**Screenshot**

You may optionally include a `screenshot.png` at the theme root (alongside `theme.toml`). This image is displayed in the admin Appearance page so users can preview the theme before activating it. Themes without a screenshot show a labelled placeholder instead.

- Recommended size: **1200×900 px** (4:3 aspect ratio)
- Format: PNG
- Content: a representative browser-window screenshot of the theme rendering a typical post or home page
- The screenshot is included in the theme zip when distributing your theme

The `api_version` field tells the CMS which context variables, filters, and functions your theme expects. If a future CMS release introduces a breaking change (which requires a major API version bump), the core will continue to serve themes that declare an older `api_version` using a compatibility layer. Always set this to the version you developed against.

---

## 5. Required Template Files

This section documents each required template file, what it renders, and which context variables it receives beyond the global context.

### Global context (available in every template)

These variables are present in all templates without exception:

| Variable | Type | Description |
|----------|------|-------------|
| `site.name` | string | Site display name |
| `site.description` | string | Site tagline |
| `site.url` | string | Canonical base URL, no trailing slash |
| `site.language` | string | BCP-47 language code, e.g. `"en-US"` |
| `site.theme` | string | Active theme name |
| `site.post_count` | integer | Count of published posts |
| `site.page_count` | integer | Count of published pages |
| `request.url` | string | Full request URL |
| `request.path` | string | Request path only |
| `request.query` | map | Query string parameters |
| `session.is_logged_in` | bool | Whether a user session is active |
| `session.user` | User or null | Logged-in user object, null if anonymous |
| `nav.primary` | NavMenu | Primary navigation menu |
| `nav.footer` | NavMenu | Footer navigation menu |
| `pagination` | Pagination or null | Pagination data when applicable |

### 5.1 base.html

`base.html` is not rendered directly. It defines the outer shell of the HTML document and declares named `{% block %}` regions that child templates fill in. Every other required template must extend `base.html` using `{% extends "base.html" %}`.

At minimum `base.html` must define a `{% block content %}{% endblock content %}` block. The default theme also defines a `{% block title %}` block so child templates can override the `<title>` element individually.

`base.html` has access to the full global context only. It does not receive post or page data directly — those live in child templates.

### 5.2 index.html

Rendered for the site home page (usually `/`). Displays a paginated list of published posts in reverse chronological order.

**Additional context beyond global:**

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list of Post | Paginated published posts, newest first |
| `featured_post` | Post or null | Pinned/featured post if one is set |

The `pagination` variable from the global context is populated on this page whenever there are more posts than the configured per-page limit.

### 5.3 single.html

Rendered for individual blog posts (URL pattern: `/posts/<slug>`).

**Additional context beyond global:**

| Variable | Type | Description |
|----------|------|-------------|
| `post` | Post | The current post |
| `related_posts` | list of Post | Up to 5 posts sharing a taxonomy term with this post |
| `prev_post` | Post or null | Chronologically previous published post |
| `next_post` | Post or null | Chronologically next published post |

`prev_post` and `next_post` are null when there is no adjacent post. Always guard with `{% if prev_post %}` before accessing their fields.

### 5.4 page.html

Rendered for static pages (URL pattern: `/pages/<slug>`). A "page" in Synaptic Signals is a post with `post_type = "page"` — content that is not part of the chronological post stream, like an About or Contact page.

**Additional context beyond global:**

| Variable | Type | Description |
|----------|------|-------------|
| `page` | Post | The current page (note: uses `page`, not `post`, as the variable name) |

There is no pagination, no related posts, and no prev/next navigation for pages.

### 5.5 archive.html

Rendered for category archives (`/categories/<slug>`), tag archives (`/tags/<slug>`), author archives (`/authors/<username>`), and date archives.

**Additional context beyond global:**

| Variable | Type | Description |
|----------|------|-------------|
| `archive_type` | string | One of `"category"`, `"tag"`, `"author"`, `"date"` |
| `archive_term` | Term or null | The taxonomy term for category/tag archives; null for author and date archives |
| `archive_author` | Author or null | The author for author archives; null for taxonomy and date archives |
| `posts` | list of Post | Paginated posts in this archive |

Check `archive_type` to decide what heading to render. The `archive_term` and `archive_author` fields are mutually exclusive — only one will be non-null for a given archive type.

### 5.6 search.html

Rendered for the search results page (`/search?q=<query>`). This template is also used when no query has been submitted yet (the `query` variable will be an empty string), so it should render a usable search form in both states.

**Additional context beyond global:**

| Variable | Type | Description |
|----------|------|-------------|
| `query` | string | The search query string; empty string if no query |
| `results` | list of Post | Matching posts; empty list if no query or no matches |
| `result_count` | integer | Total number of matches (may exceed `results` length if paginated) |

There is no `pagination` for search results in the v1 API. All results are returned in a single page.

### 5.7 404.html

Rendered whenever the CMS cannot find a matching route for the request.

**No additional context beyond global.** Use `request.path` to tell the user which URL was not found.

---

### Post type reference

All templates that work with post data use the following field structure:

| Field | Type | Description |
|-------|------|-------------|
| `post.id` | UUID string | Post identifier |
| `post.title` | string | Post title |
| `post.slug` | string | URL slug |
| `post.content` | string | Rendered HTML content (trusted; use `\| safe`) |
| `post.excerpt` | string | Auto-generated or manual excerpt (plain text) |
| `post.status` | string | `"published"`, `"draft"`, `"scheduled"`, `"trashed"` |
| `post.post_type` | string | `"post"` or `"page"` |
| `post.url` | string | Canonical URL for this post |
| `post.published_at` | string | ISO 8601 publish datetime |
| `post.updated_at` | string | ISO 8601 last-updated datetime |
| `post.author` | Author | Post author |
| `post.categories` | list of Term | Assigned categories |
| `post.tags` | list of Term | Assigned tags |
| `post.featured_image` | Media or null | Featured image; null if not set |
| `post.reading_time` | integer | Estimated reading time in minutes |
| `post.comment_count` | integer | Number of approved comments |
| `post.meta` | map | Plugin-registered custom field values |

---

## 6. Template Inheritance

Tera's inheritance system works through two paired mechanisms: `{% extends %}` and `{% block %}`. Understanding this is the foundation of building a coherent theme.

### How base.html defines the document shell

`base.html` defines the full HTML document and marks regions where child templates can inject content. Here is the default theme's `base.html` annotated:

```html
<!DOCTYPE html>
<html lang="{{ site.language }}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  {{ hook(name="head_start") | safe }}
  <title>{% block title %}{{ site.name }}{% endblock title %}</title>
  <link rel="stylesheet" href="/theme/static/css/style.css">
  {{ hook(name="head_end") | safe }}
</head>
<body>
  {{ hook(name="body_start") | safe }}

  <header class="site-header">
    <nav class="site-nav">
      <a class="site-title" href="/">{{ site.name }}</a>
      <ul class="nav-links">
        {% for item in nav.primary.items %}
          <li>
            <a href="{{ item.url }}"
               {% if item.target == "_blank" %} target="_blank" rel="noopener"{% endif %}
               {% if item.is_current %} aria-current="page"{% endif %}>
              {{ item.label }}
            </a>
          </li>
        {% endfor %}
      </ul>
    </nav>
  </header>

  <main class="site-main">
    {{ hook(name="before_content") | safe }}
    {% block content %}{% endblock content %}
    {{ hook(name="after_content") | safe }}
  </main>

  <footer class="site-footer">
    <p>&copy; {{ site.name }}</p>
    {{ hook(name="footer") | safe }}
  </footer>

  {{ hook(name="body_end") | safe }}
</body>
</html>
```

There are two blocks declared here:

- `{% block title %}` — defaults to `{{ site.name }}` if a child does not override it
- `{% block content %}` — empty by default; every child template must fill this

### How single.html inherits from base.html

A child template opens with `{% extends "base.html" %}` and then only defines the blocks it wants to override. Everything in `base.html` that is outside a block renders unchanged.

```html
{% extends "base.html" %}

{% block title %}{{ post.title }} — {{ site.name }}{% endblock title %}

{% block content %}
<article class="single-post">
  <header class="post-header">
    {% if post.featured_image %}
      <img src="{{ post.featured_image.url }}"
           alt="{{ post.featured_image.alt_text }}"
           class="featured-image">
    {% endif %}
    <h1 class="post-title">{{ post.title }}</h1>
    <div class="post-meta">
      <time datetime="{{ post.published_at }}">
        {% if post.published_at %}{{ post.published_at | date_format }}{% endif %}
      </time>
      <span class="post-author">
        by <a href="{{ post.author.url }}">{{ post.author.display_name }}</a>
      </span>
      {% if post.categories | length > 0 %}
        <span class="post-categories">
          in {% for cat in post.categories %}
            <a href="{{ cat.url }}">{{ cat.name }}</a>{% if not loop.last %}, {% endif %}
          {% endfor %}
        </span>
      {% endif %}
      <span class="reading-time">{{ post.reading_time }} min read</span>
    </div>
  </header>

  <div class="post-content">
    {{ post.content | safe }}
  </div>

  {% if post.tags | length > 0 %}
    <footer class="post-tags">
      <span>Tags:</span>
      {% for tag in post.tags %}
        <a href="{{ tag.url }}" class="tag">{{ tag.name }}</a>
      {% endfor %}
    </footer>
  {% endif %}
</article>

{% if prev_post or next_post %}
  <nav class="post-nav" aria-label="Post navigation">
    {% if prev_post %}
      <div class="post-nav-prev">
        <span>← Previous</span>
        <a href="{{ prev_post.url }}">{{ prev_post.title }}</a>
      </div>
    {% endif %}
    {% if next_post %}
      <div class="post-nav-next">
        <span>Next →</span>
        <a href="{{ next_post.url }}">{{ next_post.title }}</a>
      </div>
    {% endif %}
  </nav>
{% endif %}

{% if related_posts | length > 0 %}
  <section class="related-posts">
    <h2>Related posts</h2>
    <ul>
      {% for rp in related_posts %}
        <li><a href="{{ rp.url }}">{{ rp.title }}</a></li>
      {% endfor %}
    </ul>
  </section>
{% endif %}
{% endblock content %}
```

When this template is rendered, Tera merges it with `base.html`: the document shell, `<head>`, navigation, and footer come from `base.html`, and the two block regions (`title` and `content`) are replaced with the child's definitions.

### Using partials for shared markup

If the same post-card markup appears in both `index.html` and `archive.html`, extract it into a partial:

```html
{# templates/partials/post-card.html #}
<article class="post-card">
  <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
  <time datetime="{{ post.published_at }}">
    {{ post.published_at | date_format }}
  </time>
  <p>{{ post.excerpt }}</p>
  <a href="{{ post.url }}">Read more →</a>
</article>
```

Then include it from the parent template:

```html
{% for post in posts %}
  {% include "partials/post-card.html" %}
{% endfor %}
```

The `include` tag renders the partial within the current scope, so the loop variable `post` is visible inside `post-card.html`.

### Block inheritance rules

- A child can only define blocks that the parent has declared. Attempting to define a block that does not exist in any ancestor is silently ignored.
- The default content inside a `{% block %}...{% endblock %}` in the parent is used verbatim if the child does not override it.
- You can call `{{ super() }}` inside a child block to render the parent's default content and then append to it. This is useful for adding per-page CSS classes without discarding the parent's `<title>` defaults.

---

## 7. Hook Points

Hook points are named locations in the template rendering pipeline where plugins can inject HTML. A theme author's job with hooks is simple: place each `hook()` call at the appropriate location in `base.html`. Plugins take care of the rest.

### How hooks work

When the `hook()` function is called in a template, the CMS finds all plugin template partials registered for that hook name, renders each one with the current template context, and returns their concatenated HTML output as a string.

### Calling a hook

```html
{{ hook(name="head_end") | safe }}
```

**The `| safe` filter is mandatory.** Without it, Tera's auto-escaping will HTML-encode the output of the hook, turning `<meta name="description" ...>` into `&lt;meta name=&quot;description&quot; ...&gt;`. Because hook output is HTML generated by the CMS from registered plugin templates — not from user input — it is safe to mark as such. Do not apply `| safe` to user-supplied strings; only use it on output that the CMS itself has produced.

If no plugin has registered a partial for a given hook, `hook()` returns an empty string. The `| safe` filter on an empty string is a no-op. You can safely include all hook calls in your base template even when no plugins are installed.

### Available hook points

| Hook name | Correct location in base.html | Typical use |
|-----------|-------------------------------|-------------|
| `head_start` | First line inside `<head>` | High-priority head tags, charset declarations |
| `head_end` | Last line inside `<head>`, before `</head>` | Meta tags, Open Graph, canonical links, stylesheets |
| `body_start` | First line inside `<body>` | Skip navigation links, analytics initialization |
| `body_end` | Last line inside `<body>`, before `</body>` | Deferred scripts, chat widgets |
| `before_content` | Immediately before `{% block content %}` | Breadcrumbs, admin edit bar, alert banners |
| `after_content` | Immediately after `{% endblock content %}` | Comment systems, related content widgets |
| `footer` | Inside the `<footer>` element | Footer widgets, legal text, social links |

### Minimum required hook placement

A theme is not required to include every hook. However, for plugin compatibility, your `base.html` should at minimum include `head_end` and `body_end`. Many plugins depend on these two to function correctly. A theme that omits them will prevent installed plugins from injecting their required markup.

The recommended minimum `base.html` hook placement looks like this:

```html
<head>
  <meta charset="UTF-8">
  {{ hook(name="head_start") | safe }}
  <title>{% block title %}{{ site.name }}{% endblock title %}</title>
  <link rel="stylesheet" href="/theme/static/css/style.css">
  {{ hook(name="head_end") | safe }}
</head>
<body>
  {{ hook(name="body_start") | safe }}

  {# ... your site header and navigation ... #}

  <main>
    {{ hook(name="before_content") | safe }}
    {% block content %}{% endblock content %}
    {{ hook(name="after_content") | safe }}
  </main>

  <footer>
    {{ hook(name="footer") | safe }}
  </footer>

  {{ hook(name="body_end") | safe }}
</body>
```

### Hook context

Hook partials receive the full template context of the page being rendered. A plugin partial that fires on a single post page has access to `post`, `site`, `session`, `nav`, and all other variables in scope. Theme authors do not need to pass any special variables to hooks; the context flows through automatically.

---

## 8. Tera Filters

Filters transform a value using the pipe syntax: `{{ value | filter_name }}` or `{{ value | filter_name(arg=value) }}`. The CMS registers the following custom filters beyond Tera's built-in set.

### date_format

Formats an ISO 8601 datetime string into a human-readable date. The `format` argument accepts strftime format codes.

```html
{# Default format: "January 15, 2026" #}
{{ post.published_at | date_format }}

{# Custom format: "15 Jan 2026" #}
{{ post.published_at | date_format(format="%d %b %Y") }}

{# Always guard against null published_at before formatting #}
{% if post.published_at %}
  <time datetime="{{ post.published_at }}">
    {{ post.published_at | date_format }}
  </time>
{% endif %}
```

The `datetime` attribute on `<time>` should always receive the raw ISO 8601 string (`post.published_at`), not the formatted version.

### excerpt

Strips all HTML tags from a string and truncates to a given number of words, appending `"..."`. The default word count is 55.

```html
{# Default: 55 words #}
{{ post.content | excerpt }}

{# Custom length: 30 words #}
{{ post.content | excerpt(words=30) }}
```

Note: `post.excerpt` is already a plain-text excerpt generated by the CMS (either the manual excerpt set in the admin, or an automatic 55-word truncation of the content). For most templates, use `post.excerpt` directly rather than applying the `excerpt` filter to `post.content` manually. Use the filter when you need a different word count than the stored excerpt.

### strip_html

Removes all HTML tags from a string, leaving only text content. Useful when you need a plain-text version of content for an attribute value or a context where HTML is not appropriate.

```html
{# Plain-text version of post content for a meta description #}
<meta name="description" content="{{ post.content | strip_html | truncate_words(count=25) }}">
```

### reading_time

Estimates reading time in minutes based on a words-per-minute rate. Returns an integer, minimum 1.

```html
{# Default: 200 wpm #}
{{ post.content | reading_time }} min read

{# Custom rate #}
{{ post.content | reading_time(wpm=250) }} min read
```

Note: `post.reading_time` is pre-computed by the CMS and available directly on the post object. Use the filter only if you need to recompute with a different wpm rate.

### truncate_words

Truncates a plain-text string to a given number of words. Unlike `excerpt`, this does not strip HTML first; use `strip_html` before `truncate_words` if the input may contain HTML tags.

```html
{{ post.excerpt | truncate_words(count=20) }}
```

### slugify

Converts a string to a URL-safe slug (lowercase, spaces replaced with hyphens, special characters removed).

```html
{# Produces: "hello-world-from-rust" #}
{{ "Hello World from Rust!" | slugify }}
```

### absolute_url

Prepends `site.url` to a relative path, producing a fully-qualified URL. Useful when constructing canonical URLs or Open Graph meta tags.

```html
{# Produces: "https://example.com/blog/my-post" #}
{{ "/blog/my-post" | absolute_url }}

{# Canonical URL for a post — equivalent to post.url #}
<link rel="canonical" href="{{ post.url }}">

{# Canonical URL for a static asset #}
<meta property="og:image" content="{{ "/theme/static/images/default-og.png" | absolute_url }}">
```

---

## 9. Tera Functions

Functions are called with the `{{ function_name(arg=value) }}` syntax. Unlike filters, they are not chained onto a value; they stand alone.

### url_for

Generates a canonical URL for a named resource. Use this instead of hard-coding URL patterns.

```html
{# URL for a category archive #}
<a href="{{ url_for(type="category", slug="rust") }}">Rust</a>

{# URL for a tag archive #}
<a href="{{ url_for(type="tag", slug="async") }}">async</a>

{# URL for a specific post #}
<a href="{{ url_for(type="post", slug="hello-world") }}">Hello World</a>

{# URL for a page #}
<a href="{{ url_for(type="page", slug="about") }}">About</a>

{# URL for an author archive #}
<a href="{{ url_for(type="author", slug="alice") }}">Alice</a>
```

Supported `type` values: `"post"`, `"page"`, `"category"`, `"tag"`, `"author"`.

### get_posts

Queries posts from the CMS with optional filtering. Returns a list of Post objects. This is useful for sidebar widgets, featured post sections, or any template that needs to display a list of posts that is different from the one passed in context.

```html
{# 3 most recent posts in the "news" category #}
{% set news_posts = get_posts(category="news", limit=3) %}
{% for post in news_posts %}
  <li><a href="{{ post.url }}">{{ post.title }}</a></li>
{% endfor %}

{# 5 most recent posts by a specific author #}
{% set author_posts = get_posts(author="alice", limit=5) %}

{# Oldest posts first #}
{% set old_posts = get_posts(limit=10, order="asc") %}

{# Pages only #}
{% set pages = get_posts(post_type="page", limit=20) %}
```

**All arguments are optional:**

| Argument | Default | Description |
|----------|---------|-------------|
| `post_type` | `"post"` | `"post"` or `"page"` |
| `status` | `"published"` | Post status filter |
| `category` | (none) | Filter by category slug |
| `tag` | (none) | Filter by tag slug |
| `author` | (none) | Filter by author username |
| `limit` | `10` | Maximum results to return |
| `offset` | `0` | Skip this many results (for manual pagination) |
| `order_by` | `"published_at"` | Field to sort by |
| `order` | `"desc"` | Sort direction: `"asc"` or `"desc"` |

### get_terms

Fetches all terms for a given taxonomy. Returns a list of Term objects sorted alphabetically. Useful for rendering a category list or tag cloud.

```html
{# All categories #}
{% set categories = get_terms(taxonomy="category") %}
<ul class="category-list">
  {% for term in categories %}
    <li>
      <a href="{{ term.url }}">{{ term.name }}</a>
      <span class="count">({{ term.post_count }})</span>
    </li>
  {% endfor %}
</ul>

{# All tags #}
{% set tags = get_terms(taxonomy="tag") %}
<div class="tag-cloud">
  {% for tag in tags %}
    <a href="{{ tag.url }}" class="tag">{{ tag.name }}</a>
  {% endfor %}
</div>
```

### hook

Described in full in Section 7. Invokes all plugin partials registered for the given hook name and returns their concatenated HTML.

```html
{{ hook(name="head_end") | safe }}
```

Always apply `| safe` to the return value.

---

## 10. Pagination

The `pagination` context variable is populated automatically on templates that display lists of posts: `index.html` and `archive.html`. It is `null` on other template types.

Pagination is driven by the CMS; you do not need to call `get_posts` with `offset` to implement it. The core handles slicing the post list and setting `pagination.prev_url` and `pagination.next_url` to the correct page URLs.

### The Pagination object

| Field | Type | Description |
|-------|------|-------------|
| `pagination.current_page` | integer | Current page number, 1-indexed |
| `pagination.total_pages` | integer | Total number of pages |
| `pagination.per_page` | integer | Items per page (set in site config) |
| `pagination.total_items` | integer | Total number of posts across all pages |
| `pagination.prev_url` | string or null | URL of the previous page; null on page 1 |
| `pagination.next_url` | string or null | URL of the next page; null on the last page |

### Rendering a pagination control

The recommended pattern guards the entire pagination block with `{% if pagination %}` so the same template code works cleanly on pages that do not receive pagination data:

```html
{% if pagination %}
  <nav class="pagination" aria-label="Page navigation">
    {% if pagination.prev_url %}
      <a href="{{ pagination.prev_url }}" class="pagination-prev" rel="prev">← Newer</a>
    {% endif %}

    <span class="pagination-info">
      Page {{ pagination.current_page }} of {{ pagination.total_pages }}
    </span>

    {% if pagination.next_url %}
      <a href="{{ pagination.next_url }}" class="pagination-next" rel="next">Older →</a>
    {% endif %}
  </nav>
{% endif %}
```

The `rel="prev"` and `rel="next"` link attributes are useful for SEO — include them in the `<a>` tags rather than in the `<head>`.

For a numbered pagination strip (1 2 3 ... 10), note that the v1 API provides only `prev_url` and `next_url`; it does not provide a list of page numbers. A simple previous/next control is the supported pattern for v1 themes.

---

## 11. Navigation Menus

The CMS provides two navigation menus from the global context: `nav.primary` for the main site header navigation and `nav.footer` for the footer. Both are `NavMenu` objects with the same structure.

### NavMenu structure

```
nav.primary
  .items          — ordered list of NavItem
    [0]
      .label      — display text
      .url        — target URL
      .target     — "_self" or "_blank"
      .is_current — true if this item matches the current request URL
      .children   — list of NavItem (for sub-menus)
```

### Rendering a flat navigation menu

```html
<nav class="site-nav" aria-label="Primary navigation">
  <ul>
    {% for item in nav.primary.items %}
      <li>
        <a href="{{ item.url }}"
           {% if item.target == "_blank" %}target="_blank" rel="noopener noreferrer"{% endif %}
           {% if item.is_current %}aria-current="page"{% endif %}>
          {{ item.label }}
        </a>
      </li>
    {% endfor %}
  </ul>
</nav>
```

The `is_current` flag is set by the CMS to `true` when the item's URL matches `request.path`. Use it to apply an active state style via CSS or the `aria-current="page"` accessibility attribute.

### Rendering a two-level dropdown menu

When menu items have children, you can render a nested list:

```html
<nav class="site-nav">
  <ul>
    {% for item in nav.primary.items %}
      <li {% if item.children | length > 0 %}class="has-children"{% endif %}>
        <a href="{{ item.url }}"
           {% if item.is_current %}aria-current="page"{% endif %}>
          {{ item.label }}
        </a>

        {% if item.children | length > 0 %}
          <ul class="sub-menu">
            {% for child in item.children %}
              <li>
                <a href="{{ child.url }}"
                   {% if child.target == "_blank" %}target="_blank" rel="noopener noreferrer"{% endif %}
                   {% if child.is_current %}aria-current="page"{% endif %}>
                  {{ child.label }}
                </a>
              </li>
            {% endfor %}
          </ul>
        {% endif %}
      </li>
    {% endfor %}
  </ul>
</nav>
```

The API only provides one level of children. There is no recursive nesting beyond two levels in v1.

### Rendering the footer menu

The footer menu uses the same loop pattern:

```html
<footer class="site-footer">
  <nav aria-label="Footer navigation">
    <ul>
      {% for item in nav.footer.items %}
        <li><a href="{{ item.url }}">{{ item.label }}</a></li>
      {% endfor %}
    </ul>
  </nav>
  {{ hook(name="footer") | safe }}
</footer>
```

If no footer menu has been configured in the admin, `nav.footer.items` will be an empty list and the loop produces no output.

---

## 12. Static Assets

Files placed in the theme's `static/` directory are served verbatim by the CMS under the URL prefix `/theme/static/`. The directory structure is preserved.

### URL mapping

| File path on disk | Served URL |
|-------------------|-----------|
| `themes/my-theme/static/css/style.css` | `/theme/static/css/style.css` |
| `themes/my-theme/static/js/main.js` | `/theme/static/js/main.js` |
| `themes/my-theme/static/images/logo.svg` | `/theme/static/images/logo.svg` |

Note the path segment is always `/theme/static/` regardless of your theme's name. This is intentional: it means you can switch the active theme without updating all asset URLs.

### Referencing assets in templates

```html
{# Stylesheet #}
<link rel="stylesheet" href="/theme/static/css/style.css">

{# JavaScript — defer to avoid render-blocking #}
<script src="/theme/static/js/main.js" defer></script>

{# Image #}
<img src="/theme/static/images/logo.svg" alt="{{ site.name }}">

{# Absolute URL for an asset (e.g. for og:image) #}
<meta property="og:image" content="{{ "/theme/static/images/og-default.png" | absolute_url }}">
```

### Recommended directory layout

```
static/
  css/
    style.css          ← main stylesheet
    print.css          ← optional print styles
  js/
    main.js            ← optional progressive-enhancement JS
  images/
    logo.svg
    favicon.ico
  fonts/
    (self-hosted web fonts, if any)
```

### What belongs in static/ versus inline in templates

- CSS and JS files should live in `static/` so they can be cached and served efficiently.
- Small inline `<style>` or `<script>` blocks for critical above-the-fold CSS are fine in `base.html`.
- Do not check large binary files (video, audio) into the theme's `static/` directory; large media belongs in the CMS media library.

### No build pipeline required

Synaptic Signals does not include a CSS preprocessor, bundler, or minifier. If you want to use Sass, PostCSS, or a similar tool, run the build step in your local development environment and commit only the compiled output to `static/`. The CMS serves the files as-is.

---

## 13. Testing Your Theme

### Installing a theme

There are two ways to install a theme:

**Zip upload (recommended)** — Go to **Appearance** (`/admin/appearance`), scroll to the Upload Theme section, and upload a `.zip` file containing your theme. The zip may place theme files at the root or inside a single top-level folder — both layouts are accepted. The CMS will validate the structure and reject the upload with an error message if anything is missing. On success the theme appears in the theme list immediately. Uploading a zip whose `theme.toml` names an already-installed theme replaces it in place.

**Manual installation** — Copy your theme directory into `themes/global/` (for a theme available to all sites) or `themes/sites/<site_id>/` (for a site-specific theme). The theme will appear in the Appearance list on next server restart.

### Verify the theme loads

Activate your theme in the admin at **Appearance** (`/admin/appearance`). Select your theme and click **Activate**. The switch is immediate — templates and static assets update for all visitors without a server restart. If the theme is missing required files or has a malformed `theme.toml`, the admin will display an error and the active theme will not change.

### Test all seven template types

Each required template has a distinct URL pattern. Visit each one to confirm it renders without errors:

| Template | URL to visit |
|----------|-------------|
| `base.html` + `index.html` | `/` |
| `single.html` | Any published post URL |
| `page.html` | Any published page URL |
| `archive.html` | Any category, tag, or author archive URL |
| `search.html` | `/search` (no query) and `/search?q=test` |
| `404.html` | Any URL that does not exist |

### Test pagination

Create enough posts to trigger pagination (more than the configured per-page limit, typically 10). Visit the home page and archive pages and confirm:

- The pagination control renders correctly.
- The "previous" link is absent on page 1.
- The "next" link is absent on the last page.
- The `aria-label` on the pagination `<nav>` is present.

### Test with plugins installed

If possible, install at least one plugin that uses hooks (the default SEO plugin is ideal) and verify:

- `hook("head_end")` output appears correctly in the rendered `<head>`.
- `hook("body_end")` output appears before `</body>`.
- The `| safe` filter is applied so the plugin HTML is not escaped.

The clearest way to verify this is to view the HTML source of a rendered page and search for the expected plugin output.

### Test edge cases

- A post with no featured image: the `{% if post.featured_image %}` guard should prevent broken `<img>` tags.
- A post with no categories or tags: the conditional renders should collapse cleanly.
- A `single.html` where `prev_post` and `next_post` are both null (a site with only one post).
- An `archive.html` for a category with zero posts: the empty-state message should render.
- An empty `nav.primary.items` list: the navigation `<ul>` should render with no items.

### Validate your HTML

Run your rendered output through the W3C HTML validator or a local equivalent. Common issues:

- Missing `alt` attributes on images (check `post.featured_image.alt_text`).
- Duplicate `id` attributes when repeating post cards in a loop.
- Block-level elements inside inline elements.

### Check accessibility

- Every image has a non-empty `alt` attribute.
- Navigation landmarks use `<nav>` with `aria-label` to distinguish multiple nav regions.
- Pagination uses `aria-label="Page navigation"` on its `<nav>` element.
- The current navigation item uses `aria-current="page"`.
- The `<html>` element has a `lang` attribute populated from `{{ site.language }}`.

---

## 14. Theme Authoring Checklist

Use this checklist before publishing or deploying a theme.

### Manifest

- [ ] `theme.toml` is present at the theme root.
- [ ] `name` in `theme.toml` matches the directory name exactly.
- [ ] `api_version = "1"` is set.
- [ ] `version`, `description`, and `author` fields are filled in.

### Screenshot

- [ ] `screenshot.png` is present at the theme root (optional but recommended).
- [ ] Screenshot is 1200×900 px and shows the theme rendering a real page.

### Required files

- [ ] `templates/base.html` exists.
- [ ] `templates/index.html` exists.
- [ ] `templates/single.html` exists.
- [ ] `templates/page.html` exists.
- [ ] `templates/archive.html` exists.
- [ ] `templates/search.html` exists.
- [ ] `templates/404.html` exists.

### Template inheritance

- [ ] Every template except `base.html` starts with `{% extends "base.html" %}`.
- [ ] Every child template defines `{% block content %}`.
- [ ] No child template defines blocks that are not declared in `base.html`.

### Hook points

- [ ] `{{ hook(name="head_start") | safe }}` is placed at the start of `<head>`.
- [ ] `{{ hook(name="head_end") | safe }}` is placed at the end of `<head>`.
- [ ] `{{ hook(name="body_start") | safe }}` is placed at the start of `<body>`.
- [ ] `{{ hook(name="body_end") | safe }}` is placed at the end of `<body>`.
- [ ] `{{ hook(name="before_content") | safe }}` is placed before `{% block content %}`.
- [ ] `{{ hook(name="after_content") | safe }}` is placed after `{% endblock content %}`.
- [ ] `{{ hook(name="footer") | safe }}` is placed inside the `<footer>` element.
- [ ] Every `hook()` call has `| safe` applied to it.

### Safety — | safe usage

- [ ] `{{ post.content | safe }}` is used for rendered post/page HTML content.
- [ ] `{{ page.content | safe }}` is used for rendered page HTML content.
- [ ] `| safe` is NOT applied to any user-supplied fields: `post.title`, `post.excerpt`, `post.author.display_name`, `term.name`, `query`, `request.path`, or any `post.meta.*` value.
- [ ] `| safe` is ONLY applied to `hook()` return values and to `post.content` / `page.content` fields, which contain HTML sanitized by the CMS.

### Global context usage

- [ ] `{{ site.language }}` is used as the `lang` attribute on the `<html>` element.
- [ ] The `{% block title %}` in `base.html` has a default value of `{{ site.name }}`.
- [ ] Navigation is rendered from `nav.primary.items`, not hard-coded.
- [ ] `item.is_current` is used to mark the active navigation item.
- [ ] `item.target == "_blank"` triggers `rel="noopener noreferrer"` on the link.

### Pagination

- [ ] `{% if pagination %}` guards the pagination block in `index.html` and `archive.html`.
- [ ] `{% if pagination.prev_url %}` guards the previous page link.
- [ ] `{% if pagination.next_url %}` guards the next page link.
- [ ] The pagination `<nav>` has an `aria-label` attribute.

### Post context safety

- [ ] Featured images are guarded with `{% if post.featured_image %}` before accessing `.url` or `.alt_text`.
- [ ] `prev_post` and `next_post` are guarded with `{% if prev_post %}` and `{% if next_post %}`.
- [ ] Category and tag loops are guarded with `{% if post.categories | length > 0 %}`.
- [ ] `post.published_at` is guarded with `{% if post.published_at %}` before passing to `date_format`.

### Static assets

- [ ] CSS is linked as `/theme/static/css/style.css` (not a relative path).
- [ ] No asset URLs are hard-coded with a theme name (use `/theme/static/` prefix, not `/themes/my-theme/static/`).
- [ ] All `<img>` tags have `alt` attributes.

### HTML quality

- [ ] HTML passes W3C validation with no errors.
- [ ] `<html lang="{{ site.language }}">` is set.
- [ ] Multiple `<nav>` elements have distinct `aria-label` attributes.
- [ ] `<time>` elements carry a `datetime="{{ post.published_at }}"` attribute alongside the formatted display text.

---

*Synaptic Signals Theme Authoring Guide — API v1 — last updated 2026-02-22*
