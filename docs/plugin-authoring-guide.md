# Synaptic Signals — Plugin Authoring Guide

> **API version:** 1
> **Last updated:** 2026-02-21

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Prerequisites](#2-prerequisites)
3. [Create a "Related Posts" Plugin from Scratch](#3-create-a-related-posts-plugin-from-scratch)
   - 3.1 [Directory Structure](#31-directory-structure)
   - 3.2 [The `plugin.toml` Manifest](#32-the-plugintoml-manifest)
   - 3.3 [Registering a Hook](#33-registering-a-hook)
   - 3.4 [Writing the Hook Partial](#34-writing-the-hook-partial)
   - 3.5 [Testing with the Dev Server](#35-testing-with-the-dev-server)
4. [Adding a Custom Meta Field](#4-adding-a-custom-meta-field)
5. [Registering a Plugin-Provided Route](#5-registering-a-plugin-provided-route)
6. [Adding a Custom Tera Filter](#6-adding-a-custom-tera-filter)
7. [Available Context Reference](#7-available-context-reference)
8. [Security Rules](#8-security-rules)
9. [Troubleshooting](#9-troubleshooting)

---

## 1. Introduction

A Synaptic Signals plugin is a directory containing:

- A `plugin.toml` manifest that declares the plugin's identity, the hooks it registers, the custom meta fields it defines, and any custom routes it provides.
- One or more Tera template partials that implement the plugin's output at each registered hook point.
- An optional `static/` subdirectory for CSS, JavaScript, and image assets.

**There is no compilation step.** Plugins are not compiled Rust code, not `.so` shared libraries, and not WebAssembly modules. They are Tera templates — the same Jinja2-style syntax used in Django, Twig, and Liquid — interpreted at runtime by the Synaptic Signals core. Drop a directory into `plugins/`, restart the dev server (or wait for hot reload), and the plugin is active.

This design has deliberate constraints. A plugin author can control presentation and inject markup at defined hook points. A plugin cannot execute operating system commands, query the database directly, make network requests, or access the filesystem. These are not missing features — they are the security model. See [Section 8](#8-security-rules) for the full boundary description.

The first plugin shipped with the project is the SEO plugin (`plugins/seo/`). Its `plugin.toml` and `seo/meta.html` partial are worth reading as a reference implementation. The examples in this guide follow the same conventions.

---

## 2. Prerequisites

You need:

- Familiarity with **Jinja2, Twig, Django Templates, or Liquid** syntax. Tera is a Rust implementation of the same model. If you can write `{% for item in list %}`, `{% if condition %}`, `{{ variable }}`, and `{% set x = expr %}`, you already know the essentials.
- A working Synaptic Signals development environment: Rust toolchain installed, `cargo leptos watch` running, a local PostgreSQL or SQLite database initialised. Refer to the project README for environment setup.
- A text editor. No special tooling is required for plugin development.

You do **not** need to know Rust, WASM, or anything about the Synaptic Signals internals. The plugin API is entirely at the template level.

### Tera quick reference

```html
{# Comment — not rendered #}

{{ variable }}                          {# Output a variable (auto-escaped) #}
{{ object.field }}                      {# Dot-access a field #}
{{ value | filter_name(arg=val) }}      {# Apply a filter #}
{{ function_name(arg=val) }}            {# Call a function #}

{% if condition %}...{% endif %}
{% for item in list %}...{% endfor %}
{% set name = expression %}
{% if variable is defined %}...{% endif %}
```

Tera auto-escapes HTML by default in `.html` templates. Use `{{ value | safe }}` only when you know the value is already safe HTML produced by the core (such as `post.content`, which is sanitised rendered HTML).

---

## 3. Create a "Related Posts" Plugin from Scratch

This section walks through building a plugin called `related-posts` that appends a "You might also like" widget after every single post's content. The widget queries up to four posts that share at least one category with the current post and renders them as a list of linked titles.

### 3.1 Directory Structure

Create the following layout inside the `plugins/` directory at the root of your Synaptic Signals installation:

```
plugins/
  related-posts/
    plugin.toml
    related/
      widget.html
```

The subdirectory name (`related/`) is a namespace convention to avoid template name collisions between plugins. Use your plugin name or a logical prefix. Template paths in `plugin.toml` are relative to the plugin's root directory.

### 3.2 The `plugin.toml` Manifest

Create `plugins/related-posts/plugin.toml`:

```toml
[plugin]
name        = "related-posts"
version     = "1.0.0"
api_version = "1"
description = "Displays a 'You might also like' widget after single post content."
author      = "Your Name <you@example.com>"

[hooks]
after_content = "related/widget.html"
```

Every field in the `[plugin]` table is required:

| Field | Description |
|-------|-------------|
| `name` | Machine-readable identifier. Use lowercase letters, digits, and hyphens. Must be unique across installed plugins. |
| `version` | Semantic version string for the plugin itself. |
| `api_version` | The Synaptic Signals Plugin API version this plugin targets. Use `"1"` for the current stable API. |
| `description` | One-sentence human-readable description, shown in the admin panel. |
| `author` | Your name and contact email. |

The `[hooks]` table maps hook names to template paths. The key is a hook name from the [hook system](#available-hook-points); the value is the relative path to the partial template file inside the plugin directory.

### 3.3 Registering a Hook

The manifest above registers `after_content` as the hook point. This means the core will call `related/widget.html` immediately after the main content area on every rendered page where the theme calls `{{ hook(name="after_content") }}`.

Available hook points are:

| Hook name | Location |
|-----------|----------|
| `head_start` | Inside `<head>`, before anything |
| `head_end` | Inside `<head>`, before `</head>` |
| `body_start` | Inside `<body>`, before any content |
| `body_end` | Inside `<body>`, before `</body>` |
| `before_content` | Before the main content area |
| `after_content` | After the main content area |
| `footer` | Inside the footer element |

When multiple plugins register the same hook, their partials fire in alphabetical order by plugin name and their outputs are concatenated. A plugin cannot control its position relative to other plugins at the same hook point; if ordering matters, coordinate naming with the other plugin author or open a core feature request.

If the active theme does not call `{{ hook(name="after_content") }}` in its templates, the partial never renders. The theme is responsible for placing hook call sites. The default theme includes calls for all standard hook points.

### 3.4 Writing the Hook Partial

Create `plugins/related-posts/related/widget.html`:

```html
{#
  related-posts — widget.html
  Hooks into: after_content
  Renders a "You might also like" section on single post pages.

  Context available (from the calling template):
    post          — the current Post object (defined on single.html)
    site          — global site config
    request, session, nav — always available

  SECURITY: All user-supplied values (post.title, term.name) are output
  as context variables. They are never rendered as template source strings.
#}

{# Only show this widget on single post pages where post is in context #}
{% if post is defined and post.post_type == "post" %}

  {#
    Build a list of related posts. We query each category the current post
    belongs to and collect results, then deduplicate by checking the slug.

    get_posts() returns up to `limit` published posts. We exclude the current
    post by checking its slug against each result.
  #}
  {% set related = [] %}

  {% for cat in post.categories %}
    {% set candidates = get_posts(category=cat.slug, limit=5) %}
    {% for candidate in candidates %}
      {% if candidate.slug != post.slug %}
        {% set related = related | concat(with=candidate) %}
      {% endif %}
    {% endfor %}
  {% endfor %}

  {# Render only if there is at least one related post #}
  {% if related | length > 0 %}
  <aside class="related-posts" aria-label="Related posts">
    <h2 class="related-posts__heading">You might also like</h2>
    <ul class="related-posts__list">
      {# Show at most four items #}
      {% for item in related | slice(end=4) %}
      <li class="related-posts__item">
        <a href="{{ item.url }}" class="related-posts__link">
          {{ item.title }}
        </a>
        {% if item.published_at %}
        <time class="related-posts__date" datetime="{{ item.published_at }}">
          {{ item.published_at | date_format(format="%B %-d, %Y") }}
        </time>
        {% endif %}
      </li>
      {% endfor %}
    </ul>
  </aside>
  {% endif %}

{% endif %}
```

Key points about this template:

**`{% if post is defined %}`** — The `after_content` hook fires on every page type that calls it, including archive pages and the home page where `post` is not in context. Always guard against undefined variables before using them.

**`get_posts(category=cat.slug, limit=5)`** — This is a Tera function provided by the core. It executes a read-only database query and returns a list of Post objects. The full signature is documented in [plugin-api-v1.md](./plugin-api-v1.md#6-tera-functions). The `category` argument accepts a category slug string.

**`url_for()`** is not needed here because `item.url` is already the canonical absolute URL on every Post object. Use `url_for(type="category", slug="...")` when you need to construct a URL for a resource you do not have a Post object for, such as a category archive page.

**`{{ item.title }}`** — Post titles are auto-escaped by Tera. A title containing `<script>` will be rendered as `&lt;script&gt;`. You do not need to apply any manual escaping to context variables in `.html` templates.

**`post.content | safe`** — `post.content` is the only field where `| safe` is appropriate, because the core stores and returns sanitised HTML. Never apply `| safe` to fields that contain raw user input such as `post.title`, `post.excerpt`, `post.meta.*`, `author.display_name`, or `term.name`.

### 3.5 Testing with the Dev Server

Start the development server if it is not already running:

```
./app.sh start
```

To view live logs while testing:

```
./app.sh logs
```

Plugin template file changes (`.html`, `.xml`) require a server restart to take effect because templates are loaded at startup:

```
./app.sh restart
```

You do **not** need to recompile the binary for template changes — only `./app.sh restart`. You do need to restart for `plugin.toml` manifest changes that add or remove hooks, routes, or meta fields, because those are registered at startup.

Navigate to any single post URL in your browser. The related posts widget should appear below the post content. If it does not appear, check:

1. The theme's `single.html` (or `base.html`) calls `{{ hook(name="after_content") }}` at the correct location.
2. The post belongs to at least one category, and other published posts share that category.
3. The `plugin.toml` is valid TOML — a syntax error in the manifest silently prevents the plugin from loading. Check the server log output for parse errors.

To verify the hook is firing even when there are no related posts to display, temporarily remove the `{% if related | length > 0 %}` guard and add a static string such as `<p>hook fired</p>`.

---

## 4. Adding a Custom Meta Field

Custom meta fields allow plugin authors to attach editor-editable data to posts and pages. The data is stored in the `post_meta` database table and surfaced in the admin UI as extra fields on the post edit screen. In templates, the values are accessed as `post.meta.<key>`.

Suppose you want to let editors specify a custom call-to-action text for the related posts widget — for example, "Explore more Rust content" — that overrides the default "You might also like" heading.

**Step 1: Declare the field in `plugin.toml`.**

```toml
[plugin]
name        = "related-posts"
version     = "1.1.0"
api_version = "1"
description = "Displays a 'You might also like' widget after single post content."
author      = "Your Name <you@example.com>"

[hooks]
after_content = "related/widget.html"

[meta_fields]
custom_cta_text = { label = "Related Posts Heading", type = "text", description = "Override the 'You might also like' heading for this post. Leave blank to use the default." }
```

The key `custom_cta_text` becomes the template accessor `post.meta.custom_cta_text`. The `label` and `description` values are displayed in the admin UI. The `type` controls the input widget.

Supported field types:

| Type | Template value | Admin input |
|------|---------------|-------------|
| `text` | string | Single-line text input |
| `textarea` | string | Multi-line textarea |
| `boolean` | bool | Checkbox |
| `integer` | integer | Number input |
| `url` | string | URL input (validated on save) |
| `image` | Media or null | Media library picker |

**Step 2: Use the field in the template.**

Update the heading line in `related/widget.html`:

```html
{# Use per-post override heading if set, otherwise fall back to the default #}
{% set heading = post.meta.custom_cta_text | default(value="You might also like") %}

<aside class="related-posts" aria-label="Related posts">
  <h2 class="related-posts__heading">{{ heading }}</h2>
  ...
</aside>
```

The `| default(value="...")` filter returns the fallback string when the field is null or empty. This pattern is used throughout the SEO plugin (`seo/meta.html`) and is the idiomatic way to handle optional meta fields.

**Important:** `post.meta.custom_cta_text` is a user-supplied string. It is output as a context variable and auto-escaped. It must never be passed to `tera.render_str()` or any function that would treat it as template source. See [Section 8](#8-security-rules).

After modifying `plugin.toml` to add the `[meta_fields]` block, restart the dev server so the core can register the new field and add it to the admin post-edit form.

---

## 5. Registering a Plugin-Provided Route

Plugins can declare custom HTTP routes in their `plugin.toml`. The route is handled by the core's Rust dispatcher; the plugin supplies a Tera template that renders the response body. This is how the SEO plugin provides an XML sitemap at `/sitemap.xml`.

The following example adds an RSS 2.0 feed at `/feed.xml`.

**Step 1: Declare the route in `plugin.toml`.**

```toml
[routes]
"/feed.xml" = { template = "related/feed.xml", content_type = "application/rss+xml" }
```

The key is the URL path. `template` is the path to the Tera template file relative to the plugin root. `content_type` sets the `Content-Type` response header.

**Step 2: Create the template.**

Create `plugins/related-posts/related/feed.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
{#
  related-posts — feed.xml
  Route: /feed.xml
  Content-Type: application/rss+xml

  This template receives the standard global context. Plugin route templates
  do not receive a page-specific context (no `post` or `page` variable).
  Use get_posts() to fetch content.

  SECURITY: All user-supplied values are output as context variables.
  They are never rendered as template source strings.
#}
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{{ site.name }}</title>
    <link>{{ site.url }}</link>
    <description>{{ site.description }}</description>
    <language>{{ site.language }}</language>
    <atom:link href="{{ site.url }}/feed.xml" rel="self" type="application/rss+xml"/>

    {% set feed_posts = get_posts(limit=20, order_by="published_at", order="desc") %}
    {% for item in feed_posts %}
    <item>
      <title>{{ item.title }}</title>
      <link>{{ item.url }}</link>
      <guid isPermaLink="true">{{ item.url }}</guid>
      <description>{{ item.excerpt }}</description>
      <pubDate>{{ item.published_at | date_format(format="%a, %d %b %Y %H:%M:%S +0000") }}</pubDate>
      {% for cat in item.categories %}
      <category>{{ cat.name }}</category>
      {% endfor %}
    </item>
    {% endfor %}

  </channel>
</rss>
```

Note that this template file has a `.xml` extension, not `.html`. Tera's auto-escaping behaviour differs by extension: `.html` files auto-escape `<`, `>`, `&`, and `"`. In a `.xml` template you must escape manually where needed, or configure the template engine extension mapping. For RSS content where you want `<title>` to contain literal text characters that need XML escaping, output variables normally — Tera still escapes `<` and `>` in variable output blocks for `.xml` files. Verify your output is well-formed XML by validating against a feed validator.

**Constraints on plugin routes:**

- Routes are presentation-only. The template renders a response body from data provided by the core context. Plugins cannot implement request handlers with custom logic, authentication, or side effects.
- Route templates receive the standard global context (`site`, `request`, `session`, `nav`) plus any data the core injects via `route_context` for that route type. For plugin-declared routes the core provides the global context only; use `get_posts()` and `get_terms()` to fetch content data.
- Plugin routes cannot override core routes (e.g. `/`, `/blog/`, admin paths). If two plugins declare the same path, the core logs an error at startup and loads neither conflicting route. Use unique, specific paths for your plugin routes.

---

## 6. Adding a Custom Tera Filter

Tera filters are applied with the pipe syntax: `{{ value | my_filter(arg=val) }}`. The core ships a set of built-in filters documented in [plugin-api-v1.md](./plugin-api-v1.md#5-tera-filters):

| Filter | Purpose |
|--------|---------|
| `date_format` | Format an ISO 8601 datetime string |
| `excerpt` | Strip HTML and truncate to N words |
| `strip_html` | Remove all HTML tags |
| `reading_time` | Estimate reading time in minutes |
| `slugify` | Convert a string to a URL-safe slug |
| `truncate_words` | Truncate to N words |
| `absolute_url` | Prepend `site.url` to a relative path |

**Custom filters require a core code change.** Tera filters are Rust closures or functions registered with the Tera engine at startup. There is no mechanism for a plugin template to define a new filter; the plugin boundary is between Rust and Tera, and filters live on the Rust side.

If you need a filter that does not exist in the standard set, the correct path is:

1. **Open a pull request** against the Synaptic Signals core repository adding the filter as a Rust function registered at engine initialisation time.
2. Follow the contribution guide: the filter must be deterministic, must not perform I/O, must be documented in [plugin-api-v1.md](./plugin-api-v1.md), and must include unit tests.
3. Once merged, the filter becomes available to all plugins targeting `api_version = "1"`. New filters are additive and do not require a version bump.

**Working around a missing filter in the meantime:**

If you are waiting on a core PR, many transformations can be achieved with Tera's built-in capabilities:

```html
{# Tera has no built-in `upper` filter, but you can compose existing ones #}
{# For simple string manipulation, use the built-in `replace` or conditionals #}

{# Cap a number to a maximum without a custom filter #}
{% set display_count = item_count %}
{% if display_count > 99 %}{% set display_count = 99 %}{% endif %}
{{ display_count }}

{# Build a conditional class string without a filter #}
<article class="post{% if post.featured_image %} post--has-image{% endif %}">
```

Do not implement filter-like behaviour by rendering user content as a template string. See [Section 8](#8-security-rules) for why this is forbidden.

---

## 7. Available Context Reference

The complete, authoritative reference for every variable, type, filter, and function available in plugin templates is:

**[plugin-api-v1.md](./plugin-api-v1.md)**

That document covers:

- Global context variables available in every template (`site.*`, `request.*`, `session.*`, `nav.*`, `pagination`)
- Per-template-type context (`post` on `single.html`, `posts` on `index.html` and `archive.html`, `page` on `page.html`, `query` and `results` on `search.html`)
- Full type definitions for Post, Author, Term, Media, NavMenu, NavItem, Pagination
- All built-in Tera filters with argument signatures and examples
- All built-in Tera functions (`hook()`, `get_posts()`, `get_terms()`, `url_for()`) with argument signatures and examples
- The hook system, hook point locations, and hook partial context rules
- Plugin route registration and the `route_context` variable
- Custom field declaration, supported types, and template access patterns
- Security boundaries
- Versioning policy

When you see a variable or function in this guide, the canonical specification is in `plugin-api-v1.md`. If there is any discrepancy between this guide and that document, the API reference takes precedence.

---

## 8. Security Rules

### The cardinal rule

> **User-supplied content must always enter templates as context variables. It must never be rendered as Tera template source.**

This is the single most important rule in the plugin system. Violating it creates a template injection vulnerability — an attacker who can control a post title, comment body, custom field value, or any other user-editable string can inject arbitrary Tera syntax if that string is passed to the template engine as source code rather than data.

**Safe — pass as a context variable (the only correct pattern):**

```html
{# post.title is a context variable. Tera auto-escapes it on output. #}
<h1>{{ post.title }}</h1>

{# post.meta.custom_cta_text is user-supplied. Use default() for null safety. #}
{% set heading = post.meta.custom_cta_text | default(value="You might also like") %}
<h2>{{ heading }}</h2>

{# Author name is user-supplied. Output directly, never as template source. #}
<span class="author">{{ post.author.display_name }}</span>
```

**Unsafe — this is what must never happen (shown for illustration only):**

The core must never pass a user-supplied string to `tera.render_str()` or equivalent. As a plugin author working entirely within Tera templates, you cannot call `render_str()` — it is a Rust API, not a Tera function. This rule is enforced at the core level. What you need to understand is: if you ever find yourself constructing template logic from user data, stop and find a different approach.

### What plugins structurally cannot do

The Tera template engine has no mechanisms for the following operations. These are not disabled or sandboxed — they simply do not exist in the language:

- Execute operating system commands or shell code
- Connect to databases, external APIs, or any network resource
- Read files from the server filesystem (other than template includes managed by the core)
- Write files to the server filesystem
- Access Rust process environment variables or application secrets
- Install persistent code, hooks, or backdoors that survive a template reload
- Escalate privileges or access the admin session of another user
- Bypass Tera's auto-escaping for HTML output (without explicitly using `| safe`)

**On `| safe`:** The `| safe` filter marks a value as pre-escaped HTML and bypasses auto-escaping. It is appropriate for exactly one field: `post.content`, which the core stores as sanitised HTML after running the content through an HTML sanitiser on ingest. Do not apply `| safe` to any field that contains raw user input. When in doubt, omit `| safe` — Tera will escape the value and the output will be visible as text rather than rendered as HTML, which is preferable to an XSS vulnerability.

### Context discipline

The core is responsible for what it puts into the template context. Plugin templates can only access variables that the core explicitly provides. However, any variable the core exposes is accessible to any plugin and theme template — there is no per-plugin scoping of context. The core must not include password hashes, session tokens, secret keys, API credentials, or any field not listed in `plugin-api-v1.md`.

As a plugin author, you do not control context population. If you believe a value should be in the context but is missing, open a core PR to add it with appropriate documentation and review.

---

## 9. Troubleshooting

### Plugin is installed but its hook output does not appear

**Check 1: Does the theme call the hook?**

Open the active theme's templates and search for `{{ hook(name="after_content") }}` (or whichever hook your plugin registers). If the theme template does not call the hook, the partial never fires. Either modify the theme to add the hook call site, or file a request with the theme maintainer.

**Check 2: Is the `plugin.toml` valid TOML?**

A TOML syntax error — a missing quote, an extra comma, an invalid key — will prevent the plugin from loading. The core logs the parse error at startup. Run the server with `RUST_LOG=info cargo leptos watch` and inspect the log for lines mentioning your plugin name.

You can validate TOML syntax locally using any online TOML validator or the `taplo` CLI:

```
taplo check plugins/related-posts/plugin.toml
```

**Check 3: Did you restart after changing `plugin.toml`?**

Template file changes are picked up by hot reload. Manifest changes (`plugin.toml`) require a server restart because hooks, routes, and meta fields are registered at startup.

**Check 4: Is the template partial path correct?**

The path in `[hooks]` is relative to the plugin's root directory. For `after_content = "related/widget.html"`, the file must exist at `plugins/related-posts/related/widget.html`. Case matters on Linux filesystems.

---

### Template renders but shows no output (empty hook output)

**Check: Are your conditions too restrictive?**

The most common cause is a guard condition that evaluates to false on the page you are testing. Add a temporary fallback to verify the partial is rendering at all:

```html
{% if post is defined and post.post_type == "post" %}
  {# ... normal output ... #}
{% else %}
  <!-- related-posts: post not defined or not a post page -->
{% endif %}
```

Remove the debug comment before shipping.

---

### `get_posts()` returns an empty list

**Check 1: Are there published posts matching your filter?**

`get_posts()` only returns posts with `status = "published"`. Draft and scheduled posts are excluded. Verify you have published content in the category or with the tag you are filtering on.

**Check 2: Is the slug correct?**

The `category` and `tag` arguments to `get_posts()` accept slugs, not display names. A category named "Web Development" has a slug of `"web-development"`. Inspect the Term object in context:

```html
{% for cat in post.categories %}
  <!-- category: name={{ cat.name }} slug={{ cat.slug }} -->
{% endfor %}
```

---

### A variable outputs as empty when I expect a value

**Check: Are you using `| default()` correctly?**

The Tera `default` filter requires the named argument syntax:

```html
{# Correct #}
{% set val = post.meta.custom_cta_text | default(value="fallback") %}

{# Wrong — positional arguments are not supported for default() #}
{% set val = post.meta.custom_cta_text | default("fallback") %}
```

---

### `post.meta.<key>` is undefined for a field I declared

**Check 1: Did you restart the server after adding the `[meta_fields]` entry?**

Meta field declarations are read at startup. Adding a field to `plugin.toml` and hot-reloading templates does not register the field. Restart the dev server.

**Check 2: Has any content been saved with this field?**

A meta field declared in `plugin.toml` is registered in the admin UI form. Until an editor opens a post, sets the field value, and saves, the field is null in the database. `post.meta.custom_cta_text` will be null (not missing) for posts that have never had the field saved. Use `| default(value="")` to handle the null case gracefully.

---

### XML route output is malformed

Tera auto-escapes `<`, `>`, `&`, and `"` in variable output blocks for all template files, including `.xml` files. This is correct behaviour for element text content. However, it means `post.content` (which is HTML) will be double-escaped in an XML context if you output it directly. For RSS `<description>` elements, use `post.excerpt` instead of `post.content`, or wrap the content in a CDATA section:

```xml
<description><![CDATA[{{ post.content | safe }}]]></description>
```

CDATA sections are not processed by the XML parser and do not require entity escaping. The `| safe` here is appropriate because `post.content` is sanitised by the core before storage.

---

### A filter or function I want does not exist

Refer to [Section 6](#6-adding-a-custom-tera-filter) for guidance on contributing a new filter to the core. For functions, the same process applies — open a PR, document the function in `plugin-api-v1.md`, and include tests.

In the interim, check whether the transformation you need can be approximated with Tera's built-in control flow, existing filters, or a restructured data query. Many presentation transformations that seem to require a filter can be handled with `{% if %}` branches or by requesting slightly different data from `get_posts()`.

---

*Synaptic Signals Plugin Authoring Guide — last updated 2026-02-21*
