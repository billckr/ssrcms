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

## Role Hierarchy and Admin Account Protection

There are three tiers of access privilege:

| `users.role` | `site_users.role` | Scope |
|---|---|---|
| `"super_admin"` | (not required) | **Super Admin** — unrestricted access to all sites; bypasses `site_users`; can manage sites, themes, plugins, all users. Install-time account is `is_protected = TRUE`. |
| any | `"admin"` | **Site Admin** — full control of one site only: content, settings, themes (activate), plugins (configure), users for their site. Cannot see or manage super admin accounts. |
| any | `"editor"` | Edit all posts on their site |
| any | `"author"` | Create and edit own posts only |

**Important:** `users.role` no longer has an `"admin"` value. The agency super-admin
is identified by `role = 'super_admin'`. Site admin privilege is stored in
`site_users.role = 'admin'`, not in the `users` table.

### Deletion guards (enforced in `handlers/admin/users.rs`)

The `delete_user` handler enforces four guards in order:

1. **No self-deletion** — a user cannot delete their own account.
2. **Protected accounts** — accounts with `is_protected = TRUE` cannot be deleted by anyone. The install-time super admin account has this flag set automatically.
3. **Super admin privilege** — only a super admin (`users.role = 'super_admin'`) can delete another super admin account. A site-scoped admin cannot.
4. **Last super admin** — the final super admin account cannot be deleted, preventing a full lockout.

The delete button is hidden in the admin UI for accounts that match guard 1 (self) or guard 2 (protected). Guards 3 and 4 are enforced server-side regardless of UI state.

### Marking an account as protected

The `synaptic-cli install` command automatically sets `is_protected = TRUE` on the
account it creates. For manual upgrades, migration 0013 retroactively sets
`is_protected = TRUE` on all pre-existing accounts that had `role = 'admin'`.

---

## Security Checklist (run before each release)

- [ ] No `tera.render_str()` calls on user-supplied content anywhere in the codebase
  - A `render_str()` helper exists in `core/src/templates/loader.rs` but is `#[allow(dead_code)]` — confirm it remains unused before each release
- [ ] All post content writes go through `sanitize_content()`
  - Covers both `post::create()` and `post::update()` — both sanitize before binding
- [ ] No sensitive fields (password_hash, secrets) in any `*Context` struct
  - `UserContext` is the only user-facing struct; it has no `password_hash`
  - `user.rs` unit test explicitly asserts `password_hash` is absent from serialized `UserContext`
- [ ] All admin routes have session guard middleware
  - All admin handlers take `_admin: AdminUser` extractor; missing it causes a compile error
- [ ] Install-time super admin account has `is_protected = TRUE`
  - Prevents deletion even by other super admins
  - Verify with: `SELECT username, is_protected FROM users WHERE role = 'super_admin';`
- [ ] Site-management routes (`/admin/sites/*`) are gated behind `is_global_admin`
  - Site-scoped admins must not see the Sites nav item or access site CRUD handlers
- [ ] `SECRET_KEY` is documented as required in production; development default is clearly labelled
- [ ] Plugin templates use `| json_encode | safe` (not bare `| safe`) for all values inside JSON-LD `<script>` blocks
  - Bare `| safe` on a string inside a JSON string literal produces invalid JSON if the value contains `"` or `\`
  - Correct pattern: `{{ value | json_encode | safe }}` — `json_encode` adds quotes and escapes special chars
- [ ] `docs/plugin-api-v1.md` accurately reflects what is exposed in the context
- [ ] No SQL constructed by string interpolation from user input (all queries use `.bind()`)
- [ ] No raw internal error strings (sqlx messages, table names, stack traces) in HTTP responses — all server-side errors return generic messages to the client

---

## Audit Log

| Date | Auditor | Finding | Resolution |
|---|---|---|---|
| 2026-02-22 | Claude Code | `post::update()` did not call `sanitize_content()` on new content | Fixed — `update()` now sanitizes via `match &data.content` block |
| 2026-02-22 | Claude Code | SEO plugin JSON-LD used bare `\| safe` on URL and date values | Fixed — all JSON-LD string values now use `\| json_encode \| safe` |
| 2026-02-23 | Claude Code | Site-scoped admin could delete global admin accounts | Fixed — four-guard system in `delete_user`: no self-delete, protected flag, global-admin-only can delete global admins, last-admin lockout prevention |

---

*Synaptic Signals security rules — last updated 2026-02-23*
