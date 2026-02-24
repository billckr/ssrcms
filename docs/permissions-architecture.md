# Permissions Architecture — Current State & Future Direction

## The Problem With the Current Approach

The admin UI is built around a single shell function, `admin_page()`, that renders the nav sidebar, header, flash message, and page content in one call. Every page render function accepts `is_global_admin: bool` and later `can_manage_users: bool` as parameters.

This means:

- Every render function signature carries auth flags it does not own.
- Adding a new permission requires updating `admin_page()`, every render function signature, and every handler call site — often 20–30 files.
- The presentation layer is making decisions that belong at the boundary.
- The compiler enforces threading but not correctness — nothing stops a handler passing the wrong value.

The `can_manage_users` change was a concrete example: one boolean flag required changes across 21 files and was mechanical enough to need sed and perl to be viable.

---

## What the Architecture Should Be

### Core idea: derive capabilities once at the boundary

When a user's request is authenticated, compute a `Capabilities` struct from their role and site assignment in the middleware extractor. Everything downstream receives that struct — it does not need to re-derive anything from raw role strings.

```rust
/// Derived once in AdminUser extractor. Never recomputed downstream.
pub struct AdminCaps {
    pub can_manage_users:    bool,
    pub can_manage_sites:    bool,
    pub can_manage_plugins:  bool,
    pub can_manage_settings: bool,
    pub can_create_content:  bool,
    pub can_publish_content: bool,
}

impl AdminCaps {
    pub fn from_user(user: &User, site_role: &str, is_global_admin: bool) -> Self {
        let is_admin = is_global_admin || site_role == "admin";
        Self {
            can_manage_users:    is_admin,
            can_manage_sites:    is_admin,
            can_manage_plugins:  is_admin,
            can_manage_settings: is_admin,
            can_create_content:  true,
            can_publish_content: is_admin || site_role == "editor",
        }
    }
}
```

`AdminUser` grows one field: `pub caps: AdminCaps`. The extractor fills it once. No handler needs to re-evaluate role strings.

### Shell knows about capabilities directly

```rust
// admin_page takes one caps struct — not a growing list of booleans
pub fn admin_page(title: &str, content: &str, ctx: &PageContext) -> String
```

```rust
pub struct PageContext<'a> {
    pub current_path:          &'a str,
    pub current_site:          &'a str,
    pub user_email:            &'a str,
    pub flash:                 Option<&'a str>,
    pub caps:                  &'a AdminCaps,
    pub visiting_foreign_site: bool,
}
```

Adding a new permission is: one field on `AdminCaps`, one line in the nav table. Zero other file changes.

### Nav is data, not code

```rust
struct NavItem {
    href:         &'static str,
    label:        &'static str,
    required_cap: fn(&AdminCaps) -> bool,
}

static NAV: &[NavItem] = &[
    NavItem { href: "/admin",            label: "Dashboard",  required_cap: |_| true },
    NavItem { href: "/admin/posts",      label: "Posts",      required_cap: |_| true },
    NavItem { href: "/admin/pages",      label: "Pages",      required_cap: |_| true },
    NavItem { href: "/admin/media",      label: "Media",      required_cap: |_| true },
    NavItem { href: "/admin/categories", label: "Categories", required_cap: |_| true },
    NavItem { href: "/admin/tags",       label: "Tags",       required_cap: |_| true },
    NavItem { href: "/admin/users",      label: "Users",      required_cap: |c| c.can_manage_users },
    NavItem { href: "/admin/plugins",    label: "Plugins",    required_cap: |c| c.can_manage_plugins },
    NavItem { href: "/admin/appearance", label: "Appearance", required_cap: |_| true },
    NavItem { href: "/admin/settings",   label: "Settings",   required_cap: |c| c.can_manage_settings },
    NavItem { href: "/admin/sites",      label: "Sites",      required_cap: |_| true },
];
```

The render loop filters automatically. Adding a new restricted nav item is one line.

### Content renderers become pure

```rust
// Before: auth flags mixed into presentation
pub fn render_list(
    users: &[UserRow],
    flash: Option<&str>,
    current_site: &str,
    current_user_id: &str,
    can_manage_access: bool,
    is_global_admin: bool,
    visiting_foreign_site: bool,
    user_email: &str,
    can_manage_users: bool,
) -> String

// After: renderer only knows about its data
pub fn render_list(
    users: &[UserRow],
    current_user_id: &str,
    can_manage_access: bool,
    ctx: &PageContext,
) -> String
```

The shell parameters collapse to a single `ctx` reference. Render functions own their data parameters and nothing else.

---

## Philosophy

**Boundary is where identity becomes capability.** The moment a request is authenticated, translate "who is this person" into "what can this person do." Downstream code only asks capability questions — it never inspects role strings.

**Presentation does not make access decisions.** `is_global_admin` leaking into a render function means the render function is doing access control. That is the wrong layer. Handlers enforce access (return 403 or not). Render functions only express what the UI looks like given a known set of capabilities.

**Stable interfaces absorb change.** A `PageContext` struct with a `caps` field absorbs any number of new permissions without changing function signatures. A boolean parameter list forces signature changes on every addition.

**Principle of least knowledge.** A function that renders a posts list does not need to know whether the current user is a global admin. It needs to know whether to show a "Publish" button. Those are different questions, and the further upstream you answer them the cleaner everything downstream stays.

---

## When to Refactor

This is not urgent while the role model is still settling. The right time is:

1. The role/capability model is considered stable.
2. Before adding a third or fourth boolean-gated nav item.
3. As a single focused PR — not incrementally, because half-migrated state is worse than either extreme.

The refactor is mechanical and safe: purely additive changes to structs, replacement of argument lists, no logic changes. It is a good candidate for a day's work once the time is right.
