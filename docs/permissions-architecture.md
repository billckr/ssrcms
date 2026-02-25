# Permissions Architecture

**Status: `AdminCaps` refactor complete** (all handlers migrated, `cargo check --workspace` passes).

---

## Current Architecture

### Boundary: `AdminCaps` derived once in the extractor

`core/src/middleware/admin_auth.rs` defines `AdminCaps`, computed from `global_role` and `site_role` at the moment a request is authenticated. Nothing downstream re-evaluates role strings.

```rust
#[derive(Debug, Clone)]
pub struct AdminCaps {
    pub is_global_admin:       bool,
    pub visiting_foreign_site: bool,
    pub can_manage_users:      bool,
    pub can_manage_sites:      bool,
    pub can_manage_plugins:    bool,
    pub can_manage_settings:   bool,
    pub can_manage_content:    bool,
    pub can_manage_appearance: bool,
}

impl AdminCaps {
    pub fn from_roles(global_role: &str, site_role: &str, visiting_foreign: bool) -> Self {
        let is_global_admin = global_role == "super_admin";
        let is_admin = is_global_admin || site_role == "admin";
        Self {
            is_global_admin,
            visiting_foreign_site: visiting_foreign,
            can_manage_users:      is_admin,
            can_manage_sites:      is_admin,
            can_manage_plugins:    is_admin,
            can_manage_settings:   is_admin,
            can_manage_content:    true,
            can_manage_appearance: is_admin,
        }
    }
}
```

`AdminUser.caps` is the only downstream source of capability truth.

### Shell: `PageContext` carries flattened caps to the presentation layer

`admin/src/lib.rs` defines `PageContext` as a flat struct of primitives (to avoid a circular crate dependency — `admin` cannot import from `core`).

```rust
pub struct PageContext {
    pub current_site:          String,
    pub user_email:            String,
    pub is_global_admin:       bool,
    pub visiting_foreign_site: bool,
    pub can_manage_users:      bool,
    pub can_manage_sites:      bool,
    pub can_manage_plugins:    bool,
    pub can_manage_settings:   bool,
    pub can_manage_content:    bool,
    pub can_manage_appearance: bool,
}
```

`admin_page(title, current_path, flash, content, ctx: &PageContext)` is the single shell entry point. All render functions accept `ctx: &PageContext` instead of individual boolean parameters.

### Handler pattern

Every handler follows this pattern:

```rust
let cs = state.site_hostname(admin.site_id);
let ctx = super::page_ctx(&admin, &cs);       // fills PageContext from AdminCaps
// ... build page data ...
Html(admin::pages::foo::render_x(&data, flash, &ctx))
```

`page_ctx()` in `core/src/handlers/admin/mod.rs` is the bridge that translates `AdminUser` → `PageContext`.

### Capability checks in handlers

Handlers gate access using `admin.caps.*`:

```rust
if !admin.caps.can_manage_users {
    return (StatusCode::FORBIDDEN, "Forbidden").into_response();
}
```

Not `admin.site_role.as_str() == "admin" || admin.is_global_admin`. That derivation lives exclusively in `AdminCaps::from_roles`.

---

## What Is Not Yet Done

- **Nav is still code, not data.** The nav sidebar in `admin_page()` is still rendered with inline `if ctx.can_manage_*` conditionals rather than a static `NAV` table. This is the next incremental improvement when a new nav item is needed.
- **`can_manage_content` is unused as a gate.** Content creation is open to all authenticated admin users; no handler yet checks `can_manage_content`. This is intentional for now (authors/editors are all in the admin).
- **WASM plugin capability layer.** When the WASM plugin tier is built, plugins will receive a capability token derived from the same `AdminCaps` model rather than making a separate auth decision.

---

## Adding a New Permission

1. Add a field to `AdminCaps` in `admin_auth.rs` with its derivation logic in `from_roles`.
2. Add the same field to `PageContext` in `admin/src/lib.rs`.
3. Forward the field in `page_ctx()` in `handlers/admin/mod.rs`.
4. Use `admin.caps.your_new_cap` in handlers that need to gate on it.
5. Use `ctx.your_new_cap` in render functions that need to adjust the UI.

That is the complete list — no other files need touching.

---

## Philosophy

**Boundary is where identity becomes capability.** The moment a request is authenticated, translate "who is this person" into "what can this person do." Downstream code asks only capability questions — it never inspects role strings.

**Presentation does not make access decisions.** Handlers enforce access (return 403 or not). Render functions only express what the UI looks like given a known set of capabilities.

**Stable interfaces absorb change.** A `PageContext` struct absorbs any number of new permissions without changing function signatures. A growing boolean parameter list forces signature changes across every call site on every addition.




