# Synaptic Signals — User Databases (Planned Feature)

> **Status:** Design proposal — not yet implemented.
> **Last updated:** 2026-03-01

---

## Purpose

Allow site admins and agency clients to create and manage their own isolated SQLite
databases through the CMS admin UI. These databases are separate from the CMS's own
Postgres data store. Pages and plugins can read from (and optionally write to) these
databases via Tera template functions and form submissions.

**Primary use cases:**
- Product catalogs and inventory listings
- Store items, pricing, availability
- Staff directories, menus, schedules
- Any structured data a client wants to display and manage without building a custom app
- Plugin persistent storage (secondary use case — plugins that need to store state
  beyond post meta fields)

---

## Architecture

### Two database layers

```
┌─────────────────────────────────────────────────────────┐
│  PostgreSQL (CMS core)                                  │
│  posts, pages, users, media, plugins, settings, sites   │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  SQLite (user databases — one file per database)        │
│  data/databases/<site-uuid>/<name>.db                   │
│  e.g. data/databases/53f.../inventory.db                │
│       data/databases/53f.../contacts.db                 │
└─────────────────────────────────────────────────────────┘
```

The CMS manages the SQLite files (creates them, tracks them in Postgres metadata tables,
controls access). It does not manage their schema via migrations — schema changes are
done through the admin schema builder UI.

### Metadata in Postgres

```sql
-- Tracks which databases exist and where their files are
CREATE TABLE user_databases (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id      UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,           -- "inventory", "contacts"
    display_name TEXT NOT NULL,           -- "Product Inventory"
    file_path    TEXT NOT NULL,           -- data/databases/<site-uuid>/<name>.db
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (site_id, name)
);

-- Named credentials scoped to one database, with read or read-write access
CREATE TABLE user_db_access (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    db_id         UUID NOT NULL REFERENCES user_databases(id) ON DELETE CASCADE,
    label         TEXT NOT NULL,          -- "public-read", "form-writer"
    password_hash TEXT NOT NULL,
    can_write     BOOLEAN NOT NULL DEFAULT false,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## Admin UI (to build)

### Databases page — `/admin/databases`

Lists all databases for the current site. Actions:
- **Create database** — enter a name (slugified), creates the `.db` file and a metadata row
- **Manage tables** — schema builder (see below)
- **Browse data** — simple table view with pagination, search, inline edit, delete row
- **Add access credential** — create a read or read-write credential for use in templates/forms
- **Delete database** — drops the `.db` file and all metadata

### Schema builder — `/admin/databases/{name}/schema`

Define tables and columns without writing SQL. The UI generates and executes
`CREATE TABLE` / `ALTER TABLE` statements against the SQLite file.

Supported column types (maps cleanly to SQLite and Tera context):
- `text` — VARCHAR, free text
- `integer` — whole numbers
- `decimal` — prices, quantities (stored as TEXT to avoid float precision issues)
- `boolean` — true/false toggle
- `date` — ISO 8601 date string
- `image` — stores a path/URL to an upload (not the binary itself)

Example: creating a "products" table:
```
Table: products
  id          integer  (auto-increment, primary key)
  name        text     (required)
  description text
  price       decimal
  in_stock    boolean  (default: true)
  image_url   image
```

### Data browser — `/admin/databases/{name}/browse/{table}`

Spreadsheet-style view. Rows editable inline. Supports:
- Pagination
- Filter by column value
- Sort by column
- Add row, edit row, delete row (with confirmation)
- CSV export

---

## Template API (Tera functions to register)

All functions are read-only from templates. Write operations go through form submissions.

### `user_db_query`

Fetch rows from a table with optional filtering, ordering, and limiting.

```jinja
{% set products = user_db_query(
    database = "inventory",
    table    = "products",
    filter   = {in_stock: true},
    order_by = "name",
    order    = "asc",
    limit    = 50
) %}

{% for product in products %}
<div class="product">
  <h3>{{ product.name }}</h3>
  <p>{{ product.description }}</p>
  <span class="price">${{ product.price }}</span>
</div>
{% endfor %}
```

### `user_db_get`

Fetch a single row by ID or unique column value.

```jinja
{% set item = user_db_get(database="inventory", table="products", id=product_id) %}
{% if item %}
  <h1>{{ item.name }}</h1>
{% endif %}
```

### `user_db_count`

```jinja
{% set total = user_db_count(database="inventory", table="products", filter={in_stock: true}) %}
<p>{{ total }} items in stock</p>
```

**No raw SQL from templates.** The filter/order/limit API covers the common cases and
is safe because queries are fully parameterized by the Rust layer. Arbitrary SQL from
Tera templates would be a template injection risk and is out of scope.

---

## Form Write API

The existing form handler (`POST /form/{name}`) is extended to optionally route
submissions into a user database instead of (or in addition to) `form_submissions`.

```html
<!-- Insert a new row into the "orders" table in the "store" database -->
<form method="POST" action="/form/new-order">
  <input type="hidden" name="_db"     value="store">
  <input type="hidden" name="_table"  value="orders">
  <input type="hidden" name="_action" value="insert">

  <input name="customer_name" required>
  <input name="email" type="email" required>
  <select name="product_id">...</select>
  <button type="submit">Place Order</button>
</form>
```

```html
<!-- Update an existing row -->
<form method="POST" action="/form/update-stock">
  <input type="hidden" name="_db"     value="inventory">
  <input type="hidden" name="_table"  value="products">
  <input type="hidden" name="_action" value="update">
  <input type="hidden" name="_id"     value="{{ product.id }}">
  <input name="in_stock" type="checkbox">
  <button>Save</button>
</form>
```

The form handler validates:
- `_db` and `_table` exist for the current site (cross-site access blocked)
- `_action` is one of `insert`, `update`, `delete`
- For `update`/`delete`: `_id` is present and is an integer
- Column values are type-checked against the schema before writing

---

## Security Model

### Isolation
- Each database file is scoped to a `site_id` — a Tera function call for site A cannot
  access site B's databases, enforced in Rust before the SQLite query runs.
- File paths are never constructed from user input. The `name` is looked up in the
  `user_databases` table; the `file_path` from that row is used.

### No raw SQL from templates
Template functions only accept table name, filter object, order, and limit. Query
construction happens entirely in Rust with parameterized statements.

### Write access via forms only
Templates are read-only. Writes go through `POST /form/{name}` which validates the
schema, checks site ownership, and uses parameterized INSERT/UPDATE/DELETE.

### File location
SQLite files live at `data/databases/<site-uuid>/<name>.db`, outside the web root,
never served as static files.

### Admin-only schema changes
`CREATE TABLE`, `ALTER TABLE`, `DROP TABLE` only via admin UI. Never from templates
or form submissions.

---

## Plugin Persistent Storage

As a secondary benefit, this system gives plugins a place to store state beyond post
meta fields. A plugin can declare a database in its `plugin.toml`:

```toml
[databases]
analytics = { display_name = "Plugin Analytics", tables = ["page_views", "events"] }
```

The plugin loader would create the database and tables on first activation. The plugin's
templates could then read from it, and a plugin-registered form action could write to it.

This is a future extension — the core user database feature does not require it.

---

## Implementation Roadmap

When this is built, the recommended order:

1. **Migrations** — `user_databases` and `user_db_access` tables in Postgres
2. **Model layer** — `core/src/models/user_database.rs` (CRUD functions, non-macro sqlx)
3. **SQLite pool manager** — `core/src/db_user.rs` — maintains a `SqlitePool` per open
   database file, cached in AppState. Handle open/close lifecycle.
4. **Admin handlers** — `core/src/handlers/admin/databases.rs` — list, create, delete,
   schema builder, data browser
5. **Admin UI** — `admin/src/pages/databases.rs`
6. **Tera functions** — `user_db_query`, `user_db_get`, `user_db_count` registered on
   `TemplateEngine` init
7. **Form handler extension** — detect `_db`/`_table`/`_action` fields, route to SQLite
8. **Router** — add `/admin/databases` routes
9. **Docs** — user guide for site admins

The SQLite pool manager (step 3) is the most architecturally interesting piece. Opening
a new `SqlitePool` per request is too slow; the pools need to be cached and evicted when
a database is deleted. A `HashMap<Uuid, SqlitePool>` behind an `Arc<RwLock<...>>` in
AppState, matching the pattern used by `TemplateEngine.engines`, is the right approach.

---

## Data Type and Query Limitations

Understanding these before building prevents designing around them after the fact.

### Layer 1 — What SQLite can store

SQLite has no strict type enforcement. Every value is stored as one of: `NULL`,
`INTEGER`, `REAL`, `TEXT`, or `BLOB`. Practically:

- ✅ Text, integers, booleans (0/1), dates (ISO 8601 strings or Unix timestamps)
- ✅ Large text fields — up to ~1 GB per cell technically
- ✅ JSON (SQLite has JSON functions, though not exposed in the simple filter API)
- ❌ No native UUID, array, or enum — those become TEXT
- ❌ No native decimal/currency — floats lose precision, so decimals must be stored as
  TEXT or integer cents, which means the database cannot perform math on them directly
- ❌ No binary files (BLOBs) exposed — files belong in the media library; columns store
  URLs or paths only

### Layer 2 — What the proposed schema builder exposes

The planned column types are: `text`, `integer`, `decimal`, `boolean`, `date`, `image`.

Not supported:
- ❌ **Rich text / HTML** — storable as text but no sanitization layer
- ❌ **Arrays or lists** — e.g. a product with multiple tags requires a separate join table
- ❌ **Relations between tables** — foreign keys exist in SQLite but the filter API has
  no JOIN support; complex relations belong in Postgres or a custom plugin
- ❌ **Geospatial** — no lat/lng type, though two `decimal` columns work for basic use
- ❌ **Full-text search columns** — SQLite has an FTS5 extension but it is not in scope

### Layer 3 — What the Tera query API can retrieve

The proposed `user_db_query(database, table, filter, order_by, limit)` is intentionally
simple. Limitations:

- ❌ **No range filters** — cannot express `price > 10` or `date between X and Y`;
  only equality filters (`{in_stock: true}`)
- ❌ **No OR conditions** — the filter object is AND-only
- ❌ **No pattern matching** — no LIKE, no `name contains "chair"`
- ❌ **No aggregates** — no SUM, AVG, COUNT with GROUP BY from templates
- ❌ **No JOINs** — single table per query call
- ❌ **No offset pagination** — LIMIT without OFFSET means page 2+ is not reachable
  from a template directly; workaround is passing a minimum-ID filter
- ✅ **Multiple queries per template** — calling `user_db_query` several times on
  different tables and combining results in the template is fine

### Layer 4 — Concurrency and scale

SQLite uses file-level locking: only one write at a time per database file.

- ✅ Read-heavy workloads — hundreds of concurrent reads are handled well
- ⚠️ Moderate write workloads — form submissions queue up rather than failing, acceptable
  for low-traffic sites
- ❌ High-concurrency writes — e.g. simultaneous checkout submissions — SQLite is the
  wrong tool; that belongs in Postgres or a dedicated service
- Practical ceiling: thousands to low millions of rows, moderate traffic, infrequent
  writes from forms

### What this system is and is not suited for

| Good fit | Poor fit |
|----------|----------|
| Product catalog — list and filter by category or status | Inventory with real-time stock decrement under concurrent load |
| Staff directory, menu items, event schedule | Complex reporting with aggregates or multi-table joins |
| Simple lead or contact list | Full-text search within stored content |
| Reference data — pricing tiers, FAQs, size charts | Data with deep relations requiring normalized schema |
| Low-write, high-read display data | High-concurrency write scenarios |

Anything requiring complex queries, relations, or high write concurrency should be
handled by a custom plugin backed by Postgres, not a user SQLite database.

---

## Open Questions

- **Schema migrations for user databases** — if a user adds a column, existing rows need
  a default value. SQLite's `ALTER TABLE ADD COLUMN` supports this; the UI should prompt
  for it.
- **Max database size** — should there be a configurable limit (e.g. 500 MB per database)?
  SQLite files can grow unbounded.
- **Backup/export** — the data browser has CSV export. Should `.db` file download be
  available to super admins for backup purposes?
- **Relations between tables** — the filter API supports single-table queries. Multi-table
  joins would require either expanding the API or accepting that complex queries aren't
  supported from templates (and should be handled by building a custom plugin instead).
- **Plugin database isolation** — should plugin databases be visible in the admin data
  browser, or kept hidden/internal to the plugin?
