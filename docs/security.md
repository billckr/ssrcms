# Synaptic Signals — Security Rules

> This document is mandatory reading for anyone contributing to the core or writing plugins and themes.
> The security model is only as strong as the discipline applied to these rules.

---

## The Cardinal Rule

> **User-supplied content MUST always enter templates as context variables.**
> **It must NEVER be rendered as Tera template source.**

User-supplied content means: post titles, post body content, comments, custom field values,
author names, taxonomy names, and any other data that originates outside the application's
own code.

### What this means in practice

**Correct — content as a context variable (safe):**
```rust
// In Rust: set the value in the context
ctx.insert("post", &post_context);
```
```html
<!-- In template: reference the variable — Tera auto-escapes it -->
<h1>{{ post.title }}</h1>
```

**Forbidden — content as template source (template injection vulnerability):**
```rust
// NEVER do this
tera.render_str(&post.title, &ctx)?;       // post.title could contain {{ malicious_code }}
tera.render_str(&post.content, &ctx)?;     // same danger
tera.render_str(&comment.body, &ctx)?;     // same danger
```

If user content is rendered as a template string, an attacker can inject Tera template
syntax (e.g. `{{ get_posts(limit=1000) }}`) and force the engine to execute it.

---

## Content Sanitization Contract

Post content is stored in the database as HTML. Theme templates render it with `| safe`
to prevent double-escaping:

```html
{{ post.content | safe }}
```

The `| safe` filter bypasses Tera's auto-escaping. This is safe **only because** the
content was sanitized with `ammonia` before storage.

### The contract

1. The `post::create()` and `post::update()` model functions call `sanitize_content(html)`
   before writing to the database. `sanitize_content` uses `ammonia::clean()` with a
   conservative allowlist.
2. Any code path that writes post content to the database MUST go through `sanitize_content`.
3. The Phase 3 admin editor must not bypass this contract.
4. Raw API endpoints (Phase 4+) must also sanitize before storage.

**If you skip step 1, `{{ post.content | safe }}` becomes an XSS vector.**

---

## Context Discipline

The template context is the boundary between the Rust core and the plugin/theme layer.
What goes into the context is what plugins can read and output.

### What must NEVER be in the context

- Password hashes (`user.password_hash`)
- Session tokens or secret keys
- Database credentials or internal configuration
- Private API keys
- Any field not explicitly documented in `docs/plugin-api-v1.md`

### How to enforce this

The `UserContext` struct (not `User`) is what goes into the template context. It contains
only the fields that are safe to expose: id, username, display_name, role, bio, url.
The `User` struct (which includes `password_hash`) is never inserted into a context.

The same pattern applies to all models: the `*Context` structs are the safe public view;
the raw model structs are internal.

---

## What the Tera Structural Sandbox Prevents

A Tera plugin, regardless of its author's intent, **cannot**:

- Access the database directly
- Execute operating system commands
- Make outbound HTTP or network requests
- Read or write the server filesystem
- Read environment variables or secrets
- Install persistent code
- Exfiltrate credentials

These are structurally impossible — Tera has no I/O primitives. The language has nothing
to exploit for system access. This is the fundamental security advantage over WordPress,
where a PHP plugin can do anything PHP can do.

---

## What the Sandbox Does NOT Prevent

### Bad context design
If a developer accidentally puts sensitive data into the context (e.g. `ctx.insert("user", &full_user)`
where `full_user` includes `password_hash`), a plugin template could read and output it.
The sandbox does not protect against bad context design. Follow the `*Context` struct pattern.

### Harmful output within permitted scope
A plugin can render spam, misleading content, or deceptive UI elements while staying
fully within the sandbox. The sandbox prevents system-level harm, not all harm.
The plugin registry (Phase 4) and plugin review process are the mitigations here.

### Bugs in the Tera engine
A severe bug in the Tera library itself could theoretically break the structural guarantees.
This is an extremely unlikely scenario for a well-maintained library, but it is not
impossible in the way that breaking a WASM sandbox is essentially impossible.

---

## Auto-Escaping Rules

Tera auto-escapes all variables in `.html` and `.xml` templates by default. This means
`{{ post.title }}` is safe even if the title contains `<script>` — it will render as
`&lt;script&gt;`.

### When to use `| safe`

Only use `| safe` when you have a specific reason to trust the value:

| Value | Use `| safe`? | Reason |
|-------|--------------|--------|
| `post.content` | Yes | Sanitized by `ammonia` before storage |
| `post.title` | No | Auto-escaping is correct |
| `hook(name=...)` | Yes | Hook output is trusted HTML generated by the core |
| `post.meta.*` | No | User-supplied; auto-escaping is correct |
| JSON-LD values via `json_encode` | Yes | `json_encode` produces valid JSON strings; HTML auto-escaping on top breaks them |
| URLs in `href`/`src` | No | Auto-escaping is correct for URL characters |

### JSON-LD pattern

When rendering JSON-LD in a `<script>` block, `json_encode` already escapes strings
for JSON. HTML auto-escaping on top double-escapes them (turning `"` into `&quot;`).
Use `| json_encode | safe` for string values in JSON-LD:

```html
<script type="application/ld+json">
{
  "name": {{ post.title | json_encode | safe }},
  "url": "{{ post.url | safe }}"
}
</script>
```

---

## Auth Boundaries

- All admin routes must be protected by a session guard (Phase 3).
- The session guard must check both authentication (is the user logged in?) and
  authorization (does their role permit this action?).
- Session tokens are stored server-side (in the `tower_sessions` table) and referenced
  by an opaque cookie. The cookie is signed with `SECRET_KEY`.
- In production, `SECRET_KEY` must be a cryptographically random 64+ byte string.
  The default development key must never be used in production.

---

## Security Checklist (run before each release)

- [ ] No `tera.render_str()` calls on user-supplied content anywhere in the codebase
- [ ] All post content writes go through `sanitize_content()`
- [ ] No sensitive fields (password_hash, secrets) in any `*Context` struct
- [ ] All admin routes have session guard middleware
- [ ] `SECRET_KEY` is documented as required in production; development default is clearly labelled
- [ ] Plugin templates reviewed for `| safe` misuse on user-supplied values
- [ ] `docs/plugin-api-v1.md` accurately reflects what is exposed in the context
- [ ] No SQL constructed by string interpolation from user input (all queries use `.bind()`)
- [ ] No raw internal error strings (sqlx messages, table names, stack traces) in HTTP responses — all server-side errors return generic messages to the client

---

*Synaptic Signals security rules — last updated 2026-02-21*
