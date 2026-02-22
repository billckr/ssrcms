# Synaptic Signals — Deployment Guide

This guide covers installing Synaptic Signals on a Linux server using the `synaptic-cli` installer, Caddy as a reverse proxy, and systemd for process management.

---

## Prerequisites

- A Linux server (Debian/Ubuntu/RHEL/Fedora)
- PostgreSQL 14+ running and accessible
- Caddy 2.x installed
- A domain name pointing at the server's IP

---

## 1. Build the Binary

On your build machine (or on the server itself):

```bash
git clone <repo>
cd synaptic-signals
cargo build --release
```

This produces two binaries in `target/release/`:
- `synaptic` — the CMS server
- `synaptic-cli` — the installer/manager

---

## 2. Prepare the Install Directory

```bash
# Create a directory to hold the CMS
sudo mkdir -p /opt/synaptic-signals/uploads
sudo chown -R www-data:www-data /opt/synaptic-signals

# Copy binaries
sudo cp target/release/synaptic /opt/synaptic-signals/
sudo cp target/release/synaptic-cli /opt/synaptic-signals/

# Copy required runtime files
sudo cp -r themes /opt/synaptic-signals/
sudo cp -r plugins /opt/synaptic-signals/   # if any
```

---

## 3. Create the Environment File

```bash
sudo nano /opt/synaptic-signals/.env
```

Minimum required variables:

```env
DATABASE_URL=postgres://synaptic:yourpassword@localhost:5432/synaptic_signals
SECRET_KEY=<64-character random string>
SITE_NAME=My Site
BASE_URL=https://example.com
LOG_LEVEL=info
```

Generate a SECRET_KEY:
```bash
openssl rand -hex 32
```

---

## 4. Run the Installer

The installer wizard handles database migration, admin user creation, and generating the Caddy + systemd config files.

```bash
cd /opt/synaptic-signals
./synaptic-cli install
```

You will be prompted for:
- **Domain name** — e.g. `example.com` (Caddy handles HTTPS automatically)
- **Port** — the port Axum listens on (default: `3000`)
- **Install directory** — full path (default: current directory)
- **Database URL** — pre-filled from `.env` if present
- **Admin user** — username, email, display name, password

The installer will:
1. Connect to the database and apply all migrations
2. Create the admin user
3. Write a `Caddyfile` in the current directory
4. Write a `synaptic-signals.service` file in the current directory

---

## 5. Configure Caddy

Copy the generated Caddyfile into your Caddy configuration:

```bash
# Option A: Replace the main Caddyfile
sudo cp Caddyfile /etc/caddy/Caddyfile

# Option B: Include it from the main Caddyfile
# Add to /etc/caddy/Caddyfile:
#   import /opt/synaptic-signals/Caddyfile

sudo caddy reload --config /etc/caddy/Caddyfile
```

Caddy will automatically obtain and renew a TLS certificate from Let's Encrypt.

**What the Caddyfile does:**
- Serves `/uploads/*` directly from the filesystem (bypasses Axum)
- Serves `/theme/*` directly from the filesystem (bypasses Axum)
- Proxies everything else to Axum on `localhost:{PORT}`
- Adds security headers (HSTS, X-Frame-Options, etc.)
- Enables zstd + gzip compression
- Writes JSON access logs to `/var/log/caddy/{domain}.log`

---

## 6. Enable the Systemd Service

```bash
sudo cp synaptic-signals.service /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable synaptic-signals
sudo systemctl start synaptic-signals

# Check it started cleanly
sudo systemctl status synaptic-signals
sudo journalctl -u synaptic-signals -f
```

The service runs as `www-data`, restarts automatically on failure, and reads environment variables from `/opt/synaptic-signals/.env`.

---

## 7. Verify

```bash
# Check the process is listening
curl -I http://localhost:3000

# Check the public site (after Caddy is configured)
curl -I https://example.com

# Check the admin panel
curl -I https://example.com/admin
```

---

## CLI Reference

### `synaptic-cli install`

Interactive installer. Run from the install directory.

```
synaptic-cli install [OPTIONS]

Options:
  --output-dir <DIR>    Directory to write Caddyfile and .service (default: .)
  --non-interactive     Use defaults/env vars without prompting
```

### `synaptic-cli migrate`

Applies any pending database migrations. Safe to run multiple times.

```
synaptic-cli migrate [OPTIONS]

Options:
  --database-url <URL>  Overrides DATABASE_URL env var
```

```bash
# Example
DATABASE_URL=postgres://... ./synaptic-cli migrate
```

### `synaptic-cli user create`

Interactively creates a new user. Prompts for username, email, display name, password, and role.

```bash
./synaptic-cli user create
```

### `synaptic-cli user list`

Lists all users in a tabular format.

```bash
./synaptic-cli user list
```

### `synaptic-cli user reset-password`

Resets a user's password. Prompts for the user's email address, then a new password.

```bash
./synaptic-cli user reset-password
```

### `synaptic-cli plugin list`

Lists installed plugins by reading `plugin.toml` manifests from `./plugins/`.

```bash
./synaptic-cli plugin list
```

### `synaptic-cli theme list`

Lists installed themes by reading `theme.toml` manifests from `./themes/`.

```bash
./synaptic-cli theme list
```

### `synaptic-cli theme activate`

Activates a theme by updating `active_theme` in the database, then sends `SIGUSR1` to the running server so the change takes effect immediately — no restart required.

```bash
./synaptic-cli theme activate <name> [OPTIONS]

Options:
  --database-url <URL>    Database URL (overrides DATABASE_URL env var)
  --pid-file <PATH>       Path to the server PID file [default: synaptic.pid]
```

```bash
# Example
./synaptic-cli theme activate claude
```

The CLI reads the server's PID from `synaptic.pid` (written to the working directory on startup) and sends `SIGUSR1`. The server reacts by re-reading `active_theme` from the database and hot-reloading the templates. If the server is not running the change is still persisted in the database and will take effect on next start.

**`--pid-file`** is only needed if the server was started from a different directory or if `PID_FILE` was set to a custom path in the server config.

---

## Updating

```bash
# 1. Build new binary
cargo build --release

# 2. Copy binary (the service will be briefly down)
sudo systemctl stop synaptic-signals
sudo cp target/release/synaptic /opt/synaptic-signals/synaptic
sudo systemctl start synaptic-signals

# 3. Apply any new migrations
sudo /opt/synaptic-signals/synaptic-cli migrate
```

---

## Directory Layout at Runtime

```
/opt/synaptic-signals/
├── synaptic              # CMS binary
├── synaptic-cli          # CLI binary
├── .env                  # Environment variables (DATABASE_URL, SECRET_KEY, etc.)
├── themes/
│   └── default/          # Default theme templates + static assets
├── plugins/
│   └── seo/              # SEO plugin (and any others)
├── uploads/              # User-uploaded media files
├── search-index/         # Tantivy full-text search index (auto-created)
└── synaptic.pid          # Server PID (written on startup, removed on exit)
```

---

## Environment Variables Reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `SECRET_KEY` | Yes | — | Session signing key (≥32 random bytes, hex-encoded) |
| `HOST` | No | `0.0.0.0` | Address to bind |
| `PORT` | No | `3000` | Port to listen on |
| `SITE_NAME` | No | `Synaptic Signals` | Site display name |
| `BASE_URL` | No | `http://localhost:3000` | Canonical site URL (no trailing slash) |
| `ACTIVE_THEME` | No | `default` | Theme directory name under `themes/` |
| `UPLOADS_DIR` | No | `./uploads` | Path to store uploaded files |
| `SEARCH_INDEX_PATH` | No | `./search-index` | Path for Tantivy index files |
| `LOG_LEVEL` | No | `info` | Tracing log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `PID_FILE` | No | `./synaptic.pid` | Path to write the server PID file on startup |
