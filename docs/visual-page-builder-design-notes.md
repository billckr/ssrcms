# Visual Page Builder — Design Notes & Future Direction

> **Status:** Exploratory / deferred. No implementation has started. This document captures
> architectural thinking for a potential Phase 3/4 feature.

---

## Current Theme System (as of Phase 1–2)

Themes in Synaptic Signals are intentionally simple and powerful:

1. Build a standard HTML + CSS website.
2. Insert Tera template tags where you want dynamic content (post body, navigation, etc.).
3. Zip the directory and upload via the admin UI — it works immediately.

A **custom theme builder** also exists in the admin: it copies the default theme into a new
editable theme, lets the user make adjustments (template files, CSS), and promotes it to a live
theme. This is useful for moderate customization but is not a visual drag-and-drop experience —
it is a code-level editor on top of the zip-based system.

Both approaches are intentional: low barrier, no build pipeline, mirrors the WordPress
"drop files in a folder" mental model that Synaptic Signals targets.

---

## The Preferred Direction: Tera-Native Block Composer

Rather than building a fully open component registry (like Puck/React), the better fit for
Synaptic Signals is a **curated drag-and-drop composer built entirely on top of Tera** — using
the app's own built-in blocks as the palette.

The key insight: the blocks already exist. The app already ships Tera partials for all core
features. The user just needs a way to arrange them visually instead of hand-coding includes.

### Built-in blocks (the palette)

These are the app-defined blocks a user can place — features already built into Synaptic Signals:

- Posts list
- Single post
- Pages
- Navigation (header, footer, custom menus)
- Subscribe / newsletter form
- Comments
- Search
- Archive
- Sidebar widgets
- ... and any future built-in features

The palette is **curated and app-defined** — not an open registry. This is simpler, safer,
and more appropriate for the agency/freelancer market than an infinitely extensible system.

### How zones work

The active theme declares **zones** — named areas where blocks can be dropped. A minimal theme
might have just `main`. A richer theme might declare:

```toml
# theme.toml
[zones]
header  = { label = "Header",  max_blocks = 1 }
hero    = { label = "Hero",    max_blocks = 1 }
main    = { label = "Main content" }
sidebar = { label = "Sidebar" }
footer  = { label = "Footer",  max_blocks = 1 }
```

The theme author controls the outer structure and CSS. The site owner controls what goes
*inside* each zone. Clean separation of concerns.

### Block manifests

Each built-in block has a small manifest describing its configurable props:

```toml
# blocks/posts.toml
name     = "Posts List"
id       = "posts"
template = "blocks/posts.html"

[[props]]
key     = "count"
label   = "Number of posts"
type    = "number"
default = 10

[[props]]
key     = "category"
label   = "Filter by category"
type    = "text"
```

### Page layout stored as JSON

When a user saves their layout in the composer, it is stored as structured JSON in the database:

```json
[
  { "block": "navigation", "zone": "header",  "props": { "menu": "primary" } },
  { "block": "posts",      "zone": "main",    "props": { "count": 5 } },
  { "block": "subscribe",  "zone": "sidebar", "props": {} },
  { "block": "footer",     "zone": "footer",  "props": {} }
]
```

### Tera renders it by walking the JSON

The theme's zone placeholders become simple Tera loops:

```jinja
{% for block in layout.main %}
  {% include "blocks/" ~ block.id ~ ".html" %}
{% endfor %}
```

No new rendering engine. No new template language. The public render path is still pure Tera.

### The Leptos admin UI

The composer in the admin shows:

- **Left panel:** palette of available blocks (the curated app list)
- **Centre canvas:** the theme's zones, each a drop target
- **Right panel:** prop editor for the selected block (count, menu choice, category filter, etc.)
- **Preview:** an iframe hitting the live SSR Tera render endpoint — what the user sees is
  exactly what the public sees

---

## Database Change Required

One column needs to be added to the pages table before the content model is finalised:

```sql
ALTER TABLE pages ADD COLUMN content_blocks JSONB;
```

- If `content_blocks` is populated → the block composer renderer is used.
- If `content_blocks` is NULL → the existing raw `content TEXT` field renders as today.

This makes the composer fully **opt-in and backwards compatible**. Existing themes that hand-code
their Tera includes continue to work unchanged.

Lock this in before the content model migration is written to avoid a breaking schema change later.

---

## Why This Is Simpler Than a Puck-Style Open Registry

| Puck / open registry | Synaptic Signals block composer |
|---|---|
| Any developer can author a component | Only app-defined blocks in the palette |
| Components ship as JS/WASM modules | Blocks are Tera partials already in the app |
| Requires WASM component model for Rust | Not needed — blocks are pre-compiled into the binary |
| Open-ended, complex to govern | Curated, predictable, easy to document |

The tradeoff is intentional. The target users (agencies, freelancers, non-developers) benefit
more from a well-designed fixed palette than from infinite extensibility.

---

## What Is Still Genuinely Hard

| Challenge | Detail |
|---|---|
| **Drag-and-drop in Leptos** | No mature Rust/WASM DnD library. Options: `wasm-bindgen` interop with a small JS DnD helper, or pointer-event-based DnD implemented directly in Leptos. |
| **Inline prop editors** | Number inputs, text fields, menu selectors, image pickers — all buildable in Leptos but written from scratch. |
| **Live preview fidelity** | An iframe hitting the SSR endpoint is simplest. Theme CSS must be loaded in the iframe context, not the admin context. |
| **Zone awareness** | The composer needs to read the active theme's `theme.toml` to know which zones to display. Theme switching may require layout migration. |

---

## Relationship to the Existing Theme System

The block composer and the zip-based theme system are **complementary, not competing**:

- The theme controls *layout and styling* — outer HTML structure, header/footer markup, CSS.
- The composer controls *zone content* — which blocks appear in each zone and in what order.
- A theme that does not declare zones works exactly as today; the composer is simply not available.
- A theme that declares zones unlocks the composer for that theme's users.

This is the natural next step beyond the custom theme builder already in the app — no code
required from the site owner, and no disruption to theme authors who prefer full template control.

---

## Reference: Puck and Astro (original inspiration)

**[Puck](https://github.com/puckeditor/puck)** is an open-source visual drag-and-drop page
editor for React. It gives users a canvas, a component palette, live preview, and outputs
structured JSON. Think Webflow or Elementor, but open-source and embeddable.

**[Astro](https://astro.build)** is a *web framework* (island architecture, SSR/SSG, content
collections) — a build/runtime framework rather than an editor. Less directly comparable.

The Synaptic Signals block composer takes the Puck concept but replaces the open React component
registry with the app's own curated Tera block library — simpler, no JS dependency, and fully
consistent with the Tera-first architecture.
