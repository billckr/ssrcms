# Synaptic Signals — Admin Handler Patterns

Reference for writing admin request handlers in `core/src/handlers/admin/`.
Follow these patterns for every new admin page or operation.

---

## The Admin Rendering Model

The `admin` crate generates HTML as plain strings. Every page render function accepts
a `flash: Option<&str>` parameter which is displayed as a banner above the page content:

```rust
// In admin/src/pages/your_page.rs
pub fn render_list(items: &[Item], flash: Option<&str>) -> String {
    let content = /* build page HTML */;
    admin_page("Page Title", "/admin/your-page", flash, &content)
}
```

`admin_page()` (in `admin/src/lib.rs`) HTML-escapes the flash string and classifies
it as an error or success banner based on keyword matching — no separate type needed.

**Error keywords** (triggers red banner): `Error`, `error`, `failed`, `Failed`, `invalid`,
`does not`, `incorrect`, `must`, `cannot`

**Everything else** is treated as a success (green banner).

---

## Three Handler Types

### 1. List / Read handlers

These display data. Errors should degrade gracefully — show an empty list and log,
rather than crashing or panicking.

```rust
pub async fn list(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Html<String> {
    let items = crate::models::thing::list(&state.db).await.unwrap_or_else(|e| {
        tracing::warn!("failed to list things: {:?}", e);
        vec![]
    });
    // map to view structs...
    Html(admin::pages::thing::render_list(&rows, None))
}
```

**Rules:**
- Use `unwrap_or_else(|e| { tracing::warn!(...); default })`, never bare `unwrap_or_default()`
- Returning an empty list is better than a 500 page — the admin can still navigate
- `tracing::warn!` because a DB being temporarily unavailable is a recoverable situation

---

### 2. Form save handlers (create / update)

These process form submissions. On failure, **re-render the form** with the submitted
data pre-populated and a flash message. Never return bare HTML like
`Html(format!("<p>Error: {}</p>", e))`.

```rust
pub async fn save_new(
    State(state): State<AppState>,
    _admin: AdminUser,
    Form(form): Form<ThingForm>,
) -> impl IntoResponse {
    let create = CreateThing {
        name: form.name.clone(),   // clone fields you'll need on error re-render
        // ...
    };

    match crate::models::thing::create(&state.db, &create).await {
        Ok(_) => Redirect::to("/admin/things").into_response(),
        Err(e) => {
            tracing::error!("create thing error: {:?}", e);
            let edit = ThingEdit {
                id: None,
                name: form.name,   // pre-populate from submitted form
                // ...
            };
            let msg = friendly_save_error(&e);
            Html(admin::pages::thing::render_editor(&edit, Some(&msg))).into_response()
        }
    }
}
```

**Rules:**
- Clone the form fields you need for re-rendering *before* moving them into the model struct
- `tracing::error!` because a failed write is unexpected and should be investigated
- Never put `e.to_string()` directly in the response — use a friendly error helper (see below)
- On success: `Redirect::to(list_url).into_response()` (PRG pattern)
- On failure: re-render the form page with the flash message

#### Friendly error helper

Every handler file that does saves should have a small helper that converts the error
to a user-visible message without leaking internal details:

```rust
fn friendly_save_error(e: &crate::errors::AppError) -> String {
    let s = e.to_string();
    if s.contains("duplicate key") || s.contains("unique") {
        "An item with that name or slug already exists.".to_string()
    } else {
        "Failed to save. Please try again.".to_string()
    }
}
```

The two cases cover ~90% of user-triggered errors. Everything else returns the generic message.
The internal error is already in the log via `tracing::error!`.

---

### 3. Delete handlers

Delete handlers always redirect, but must check guards before deleting. When a guard
prevents deletion, re-render the list with a flash message rather than silently redirecting.

**Simple delete (no guards needed):**
```rust
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = crate::models::thing::delete(&state.db, id).await {
        tracing::error!("failed to delete thing {}: {:?}", id, e);
    }
    Redirect::to("/admin/things").into_response()
}
```

**Delete with guards (e.g. user deletion):**

When a delete must be conditionally blocked, re-render the list with the flash error
rather than doing a redirect (which loses the error message):

```rust
pub async fn delete(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let cs = state.site_hostname(admin.site_id);

    macro_rules! deny {
        ($msg:expr) => {{
            tracing::warn!("delete denied for {} by {}: {}", id, admin.user.id, $msg);
            let rows = /* fetch and map to view structs */;
            return Html(admin::pages::thing::render_list(&rows, Some($msg), &cs, ...)).into_response();
        }};
    }

    // Guard: example — no self-deletion
    if id == admin.user.id {
        deny!("You cannot delete your own account.");
    }

    if let Err(e) = crate::models::thing::delete(&state.db, id).await {
        tracing::error!("failed to delete thing {}: {:?}", id, e);
    }
    Redirect::to("/admin/things").into_response()
}
```

**Rules:**
- Always redirect on success regardless of whether the delete failed — the admin list reflects actual state
- On a guard failure, re-render the list with the flash message (not a redirect, which loses the message)
- `tracing::error!` for failed DB writes; `tracing::warn!` for guard violations
- If there is a dependent resource to delete first (e.g. file on disk), log each step separately:

```rust
if let Err(e) = std::fs::remove_file(&path) {
    tracing::warn!("failed to delete file {:?}: {:?}", path, e);  // warn — file may not exist
}
if let Err(e) = crate::models::media::delete(&state.db, id).await {
    tracing::error!("failed to delete media record {}: {:?}", id, e);  // error — DB write fail
}
```

---

## Logging Levels

| Situation | Level | Reason |
|-----------|-------|--------|
| DB unavailable on a read | `warn` | Temporary / recoverable; not a code bug |
| Resource not found (404 redirect) | `warn` | Expected outcome; worth noting but not alarming |
| Failed to write to DB | `error` | Unexpected; should never happen in normal operation |
| Failed to load/switch a theme | `error` | Unexpected; configuration or filesystem problem |
| File deletion fails | `warn` | File may already be absent; not critical |
| Password hashing fails | `error` | Library/system failure; unexpected |

**Never use `tracing::error!` for conditions the user can cause by normal interaction**
(e.g. submitting a duplicate slug). Those should use `warn` at most, or just be handled
silently and reported to the user via the flash message.

---

## AppError Variants

Defined in `core/src/errors.rs`. Use the appropriate variant:

| Variant | HTTP | When to use |
|---------|------|-------------|
| `AppError::NotFound(msg)` | 404 | Resource doesn't exist |
| `AppError::Unauthorized` | 401 | Not logged in |
| `AppError::Forbidden` | 403 | Logged in but not permitted |
| `AppError::BadRequest(msg)` | 400 | Invalid user input |
| `AppError::Database(e)` | 500 | Auto-converted from `sqlx::Error` via `?` |
| `AppError::Template(e)` | 500 | Auto-converted from `tera::Error` via `?` |
| `AppError::Internal(msg)` | 500 | Unexpected internal failure |

All `Internal`/`Database`/`Template` errors log automatically via the `IntoResponse` impl
and return a generic message to the client — internal details are never exposed.

Public handlers (home, post, page, archive) return `Result<String, AppError>` and use `?`
to propagate. Admin handlers use explicit `match` because they need to re-render forms
rather than return a JSON error response.

---

## Anti-Patterns (never do these)

```rust
// WRONG — swallows errors silently; failures are invisible
let items = model::list(&db).await.unwrap_or_default();

// WRONG — leaks internal error details (sqlx messages, table names) to the browser
Html(format!("<p>Error: {}</p>", e)).into_response()

// WRONG — ignores write failures entirely
let _ = model::delete(&db, id).await;

// WRONG — discards error with no logging; failure unobservable
Err(_) => Redirect::to("/admin/things").into_response()

// WRONG — returns String as the error type instead of AppError
async fn activate(...) -> Result<Redirect, String>
```

---

## Adding a New Admin Page — Checklist

1. **Admin crate** (`admin/src/pages/your_page.rs`):
   - Define view structs (e.g. `YourItem`, `YourEdit`)
   - `render_list(items: &[YourItem], flash: Option<&str>) -> String`
   - `render_editor(item: &YourEdit, flash: Option<&str>) -> String`
   - Both delegate to `admin_page(title, path, flash, &content)`

2. **Handler** (`core/src/handlers/admin/your_page.rs`):
   - `list` handler — warn-logged fallback
   - `new` handler — renders empty editor
   - `edit` handler — warn-log + redirect on not found
   - `save_new` handler — re-render with flash on error
   - `save_edit` handler — re-render with flash on error
   - `delete` handler — guard checks first; re-render list with flash on denial; error-log and redirect on DB failure
   - `friendly_save_error()` helper

3. **Router** (`core/src/router.rs`):
   - Add routes behind the admin auth middleware layer

4. **Admin navigation** (`admin/src/lib.rs`):
   - Add nav link in the sidebar `nav_links` list

---

*Synaptic Signals admin handler patterns — last updated 2026-02-23*
