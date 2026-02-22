# Synaptic Signals — Development Guide

Day-to-day reference for running, building, checking, and restarting the CMS during development.


  So the full stack is:

  ┌────────────────────────┬───────────────────────────────────┐
  │         Layer          │            Technology             │
  ├────────────────────────┼───────────────────────────────────┤
  │ HTTP server            │ Rust (Axum)                       │
  ├────────────────────────┼───────────────────────────────────┤
  │ Database               │ PostgreSQL via Rust (SQLx)        │
  ├────────────────────────┼───────────────────────────────────┤
  │ Admin UI               │ Rust (HTML generated server-side) │
  ├────────────────────────┼───────────────────────────────────┤
  │ Public templates       │ Tera (HTML rendered by Rust)      │
  ├────────────────────────┼───────────────────────────────────┤
  │ Search                 │ Rust (Tantivy, embedded)          │
  ├────────────────────────┼───────────────────────────────────┤
  │ CLI                    │ Rust (Clap + Dialoguer)           │
  ├────────────────────────┼───────────────────────────────────┤
  │ Future admin hydration │ Rust → WASM (Leptos, not started) │
  └────────────────────────┴───────────────────────────────────┘

---

## Quick Reference

### app.sh (preferred — handles port cleanup, PID tracking, background process)

| Task | Command |
|---|---|
| Start server (background) | `./app.sh start` |
| Stop server | `./app.sh stop` |
| Restart server | `./app.sh restart` |
| Check if running | `./app.sh status` |
| Tail live logs | `./app.sh logs` |
| Debug build | `./app.sh build` |
| Release build | `./app.sh build-release` |
| Reinstall CLI after CLI changes | `./app.sh update-cli` |
| Apply migrations | `./app.sh migrate` |
| Clear search index | `./app.sh clean-index` |
| Force full recompile | `./app.sh clean-build` |
| Run unit tests | `./app.sh test` |
| Run unit tests (formatted table) | `./unittest.sh` |
| Run unit + integration tests | `DATABASE_URL=... ./app.sh test-all` |

### Direct cargo commands (useful during active development)

| Task | Command |
|---|---|
| Check for errors | `cargo check` |
| Check one crate | `cargo check -p synaptic-core` |
| Run unit tests (no DB) | `cargo test -p synaptic-core -p admin` |
| Run all tests incl. integration | `DATABASE_URL=... cargo test -p synaptic-core -- --include-ignored` |

All commands must be run from the workspace root (`/home/ssrust26/synaptic-signals/`).

---

## First-Time Setup

### 1. Rust toolchain

```bash
# Install rustup if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify:
```bash
rustc --version   # e.g. rustc 1.83.0
cargo --version
```

### 2. PostgreSQL

```bash
# Start (adjust data dir for your distro)
su postgres -s /bin/bash -c "pg_ctl -D /var/lib/pgsql/data start"

# Create DB and user (first time only)
su postgres -s /bin/bash -c "psql -c \"CREATE USER synaptic WITH PASSWORD 'password';\""
su postgres -s /bin/bash -c "psql -c \"CREATE DATABASE synaptic_signals OWNER synaptic;\""
```

### 3. Environment file

```bash
cp .env.example .env
# Edit DATABASE_URL, SECRET_KEY, etc.
```

Minimum `.env`:
```env
DATABASE_URL=postgres://synaptic:password@localhost:5432/synaptic_signals
SECRET_KEY=dev-secret-key-change-in-production-must-be-64-bytes-long-padding
LOG_LEVEL=info
```

### 4. Run migrations and seed admin user

```bash
synaptic-cli migrate
synaptic-cli user create   # choose role: admin
```

---

## Running the Server

Use `app.sh` — it handles everything automatically:

```bash
./app.sh start    # builds if needed, clears port, starts in background
./app.sh stop     # graceful stop with force-kill fallback
./app.sh restart  # stop + start in one command
./app.sh status   # is it running? which PID?
./app.sh logs     # tail logs/synaptic.log live (Ctrl+C to exit)
```

The server starts on `http://0.0.0.0:3000` by default.
- Public site: `http://localhost:3000`
- Admin panel: `http://localhost:3000/admin`

Logs are written to `logs/synaptic.log` in the workspace root.

### What app.sh handles automatically on start
- Frees port 3000 if a previous process didn't exit cleanly
- Removes Tantivy lock files left by a crash
- Builds the binary if it doesn't exist yet
- Confirms the process is still alive 2 seconds after launch

---

## Checking for Errors

`cargo check` compiles without producing a binary — much faster than `cargo build`. Use it constantly while editing.

```bash
# Check the whole workspace
cargo check

# Check only the main server crate (faster if you haven't touched admin/cli)
cargo check -p synaptic-core

# Check the CLI
cargo check -p synaptic-cli
```

Expected output when clean:
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in Xs
```

The build should produce **zero warnings**. Phase 4 scaffolding that isn't yet called is annotated
with `#[allow(dead_code)]` at the call site so it doesn't pollute the output. If you see a new
warning after your changes, treat it as an error — either fix it or, if it's intentional
scaffolding, add a targeted `#[allow(dead_code)]` with a comment explaining why.

---

## Testing

### Unit tests (no database required)

Unit tests are inline `#[cfg(test)]` modules co-located with the source they test. They run entirely in-process with no network or database dependencies.

```bash
./app.sh test
# formatted summary table:
./unittest.sh
# or directly:
cargo test -p synaptic-core -p admin
```

Expected output: **66 tests pass, integration stubs ignored**.

Run a subset by name (substring match):

```bash
cargo test -p synaptic-core filters        # all filter tests
cargo test -p synaptic-core reading_time   # a specific test
cargo test -p synaptic-core -- --nocapture # show println! output
```

### Integration tests (require a live PostgreSQL instance)

Integration test stubs live in `core/tests/`. They are marked `#[ignore]` and skipped by default. To run them you need:
1. A running PostgreSQL instance
2. `DATABASE_URL` set to a test database

```bash
DATABASE_URL=postgres://user:pass@localhost/synaptic_test ./app.sh test-all
# or directly:
DATABASE_URL=postgres://user:pass@localhost/synaptic_test \
  cargo test -p synaptic-core -- --include-ignored
```

> **Note:** The integration stubs currently contain `todo!()` bodies — they are placeholders for when a `[lib]` target is added to `core/Cargo.toml`. The stubs document the intended test scenarios and the setup steps needed to implement them.

### Where tests live

| Location | What it tests |
|---|---|
| `core/src/templates/filters.rs` | All 7 Tera filters |
| `core/src/errors.rs` | `AppError` variants → HTTP status codes |
| `core/src/config.rs` | Config defaults and `bind_addr()` |
| `core/src/models/user.rs` | `UserRole`, password hashing, `UserContext` |
| `core/src/models/post.rs` | `PostStatus`/`PostType`, `sanitize_content`, `PostContext::build` |
| `admin/src/pages/posts.rs` | View link URL generation (post → `/blog/{slug}`, page → `/{slug}`) |
| `core/tests/model_crud.rs` | Post/user/taxonomy CRUD (integration, `#[ignore]`) |
| `core/tests/routes.rs` | HTTP route responses (integration, `#[ignore]`) |

---

## Building

```bash
# Debug build (faster compile, slower binary) — use during development
cargo build

# Release build (slower compile, optimised binary) — use for deployment
cargo build --release
```

Binaries are written to:
- `target/debug/synaptic` and `target/debug/synaptic-cli`
- `target/release/synaptic` and `target/release/synaptic-cli`

---

## The CLI (`synaptic-cli`)

### Available commands

```bash
synaptic-cli install              # Interactive setup wizard
synaptic-cli migrate              # Apply pending DB migrations
synaptic-cli user create          # Create a user interactively
synaptic-cli user list            # List all users
synaptic-cli user reset-password  # Reset a user's password by email
synaptic-cli plugin list          # List installed plugins
```

### When to reinstall the CLI

`synaptic-cli` is a compiled Rust binary installed globally at `~/.cargo/bin/`. It only updates when you explicitly reinstall it — editing source files alone has no effect on the running binary.

```bash
./app.sh update-cli
# or directly:
cargo install --path cli
```

> **Important:** `cargo build` does NOT update the globally installed `synaptic-cli`. Only `cargo install --path cli` (or `./app.sh update-cli`) does.

Run `update-cli` whenever you change anything under `cli/src/`. Examples of changes that require it:

| Change | Example |
|---|---|
| Add a new command | Adding `synaptic-cli backup` |
| Change a prompt or message | Rewording a `dialoguer` prompt in `user create` |
| Fix a bug in the installer | Wrong path written to Caddyfile |
| Add a subcommand | Adding `plugin enable` / `plugin disable` |

### What does NOT need update-cli

| Changed | Action needed |
|---|---|
| `cli/src/**` | `./app.sh update-cli` |
| `core/src/**` or `admin/src/**` | `./app.sh restart` (recompile + restart) |
| `themes/**` template or CSS files | `./app.sh restart` (no recompile — just restart) |
| `plugins/**` template files | `./app.sh restart` (no recompile — just restart) |
| Active theme (via admin Appearance page) | Nothing — updates live immediately for all visitors |

---

## Database Migrations

Migrations live in `migrations/` at the workspace root. They are applied automatically on server startup AND can be run manually:

```bash
synaptic-cli migrate
```

### Creating a new migration

```bash
# Install sqlx-cli if you don't have it
cargo install sqlx-cli --no-default-features --features postgres

# Create a new migration file
sqlx migrate add <migration_name>
# e.g.: sqlx migrate add add_post_views_column
```

This creates `migrations/<timestamp>_<name>.sql`. Edit the file, then apply:

```bash
synaptic-cli migrate
# or
sqlx migrate run
```

---

## Restarting During Development

```bash
./app.sh restart
```

If you want automatic restarts on every file save, use `cargo-watch` (runs the server in the foreground — not via `app.sh`):

```bash
cargo install cargo-watch
cargo watch -x "run --bin synaptic"
```

---

## Common Issues

### `cargo: command not found`

Cargo is not in your PATH. Fix:
```bash
source "$HOME/.cargo/env"
```

To make this permanent (already done if you followed setup):
```bash
echo 'source "$HOME/.cargo/env"' >> ~/.bashrc
```

### Port 3000 already in use

`./app.sh start` clears this automatically. If you're not using `app.sh`:
```bash
fuser -k 3000/tcp
```

### `DATABASE_URL not set`

Either source your `.env` manually or ensure it exists:
```bash
export $(grep -v '^#' .env | xargs)
cargo run --bin synaptic
```

Or just make sure the `.env` file is in the workspace root — the server loads it automatically via `dotenvy`.

### Migrations fail on startup

Run them manually to see the error clearly:
```bash
synaptic-cli migrate
```

### `relation "_sqlx_migrations" already exists`

Harmless — SQLx checks for this table on every startup and skips if present.

### Search index schema mismatch

If you see `search index schema mismatch — recreating index` in logs, the schema changed and the index was wiped and rebuilt. This is automatic and expected after schema changes.

---

## Project Structure (quick reference)

```
synaptic-signals/
├── core/src/
│   ├── main.rs          — entrypoint, AppState assembly, router mount
│   ├── config.rs        — environment variable loading
│   ├── router.rs        — all route definitions
│   ├── models/          — DB models (post, page, user, media, taxonomy)
│   ├── handlers/        — Axum request handlers (public + admin)
│   │   └── admin/       — admin-specific handlers
│   ├── middleware/       — AdminUser session extractor
│   ├── templates/        — Tera engine, filters, functions, context builder
│   ├── plugins/          — HookRegistry, PluginLoader, manifest parser
│   └── search/           — Tantivy index, indexer, background rebuild
├── admin/src/
│   ├── lib.rs            — page shell builder, HTML escape, CSS embed
│   └── pages/            — login, dashboard, posts, media, taxonomy, users, settings
├── cli/src/
│   └── commands/         — install, migrate, user, plugin
├── migrations/           — SQLx migration files (0001–0007)
├── themes/default/       — default theme templates + CSS
├── plugins/seo/          — SEO plugin (meta tags, sitemap)
├── deployment/           — Caddyfile.template, synaptic-signals.service
└── docs/                 — this file and other guides
```
