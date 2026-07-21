# Synaptic Signals VPS Deployment Guide

This guide covers deploying Synaptic Signals to a test VPS from your local dev
machine, currently `178.156.176.60` with domain `bckr.dev`.

## Quick Start

```bash
cd /home/ssrust26/synaptic-signals
VPS_PASSWORD='...' ./scripts/deploy-vps.sh
```

That's it. The script:

1. Checks the VPS's glibc version is >= your local machine's (aborts with a
   clear message rather than shipping a binary that fails to run).
2. Runs `cargo build --release` **fresh, every time** — the binary always
   embeds exactly your current `migrations/` directory (migrations are
   compiled in via `sqlx::migrate!`, not read from disk at runtime, so a
   stale binary is the classic way to get a "missing migration" error even
   though the `.sql` file is sitting right there on the VPS).
3. Ships the binary, `synap-cli`, `themes/`, `plugins/`, and `admin/static/`
   to the VPS.
4. Runs `synap-cli install --non-interactive`, which handles DB sync,
   running migrations, creating the admin user, and generating the
   Caddyfile + systemd unit.
5. Installs the Caddy config and systemd service, restarts, and verifies
   the app responds with a 200.

Re-run it any time you want to push local changes — it's idempotent and
reuses the existing `.env`/DB credentials unless you pass `--clean`.

### First-time / full-reset install

```bash
VPS_PASSWORD='...' ./scripts/deploy-vps.sh --clean
```

`--clean` additionally: stops any crash-looping leftover service, drops and
recreates the `synaptic_signals` database from scratch, and removes any
orphaned install directories from previous attempts. Use this if the VPS is
in an unknown/broken state (which is exactly what happened before this
script existed — see "Why this exists" below).

### Configuration

All defaults match the current test VPS; override with env vars for a
different target:

| Variable | Default | Purpose |
|---|---|---|
| `VPS_HOST` | `178.156.176.60` | VPS IP/hostname |
| `VPS_USER` | `root` | SSH user |
| `VPS_PORT` | `22` | SSH port |
| `VPS_PASSWORD` | *(unset)* | If set, uses `sshpass` instead of key auth |
| `VPS_DOMAIN` | `bckr.dev` | Site domain (also used for Caddy + site record) |
| `INSTALL_DIR` | `/var/www/bckr.dev` | Install path on the VPS |
| `SYNAPTIC_USER` | `www-data` | System user the service runs as |
| `APP_PORT` | `3000` | Port Axum listens on |
| `ADMIN_EMAIL` | `bill.coker@gmail.com` | Admin login email (seeded on first install) |
| `ADMIN_USERNAME` | `admin` | Admin username |
| `APP_NAME` | `Synaptic Signals` | Admin panel brand name |

## Why this exists

An earlier pair of ad-hoc scripts (`deploy-vps-setup.sh` +
`deploy-service-setup.sh`, now removed) copied a binary that had been built
*before* certain migrations were added, then separately copied the
migrations folder — which does nothing, since `sqlx::migrate!("../migrations")`
bakes migration files into the binary **at compile time on whichever
machine ran `cargo build`**. The fix is procedural: always rebuild
immediately before shipping, which `deploy-vps.sh` now does automatically
every run.

## Two deploy modes

```bash
VPS_PASSWORD='...' ./scripts/deploy-vps.sh              # auto-configures a site + admin (fast dev path)
VPS_PASSWORD='...' ./scripts/deploy-vps.sh --no-install  # infra only: build, ship, migrate, start — no site/admin
```

Use `--no-install` to mirror the intended post-release flow: get the binary
running as a service first, then configure the app yourself via
`synap-cli install` (interactive prompts) once it's up. This is what the
real `scripts/install.sh` (the GitHub-release-based production installer)
is designed around — `deploy-vps.sh --no-install` gives you the same
"infra up, configure by hand" shape for local test builds.

After a `--no-install` deploy, on the VPS:

```bash
sudo -u www-data bash -c 'cd /var/www/bckr.dev && ./synap-cli install'
```

Must run as the service user (`synap-cli` checks it owns `$INSTALL_DIR`),
and must use `./synap-cli` (the full/relative path) rather than the bare
command — `sudo`'s `secure_path` on RHEL/AlmaLinux strips `/usr/local/bin`,
so the global symlink isn't found under `sudo -u`. This regenerates the
Caddyfile/systemd unit for whatever domain/admin you choose; reload/restart
afterwards:

```bash
cp /var/www/bckr.dev/Caddyfile /etc/caddy/Caddyfile && caddy reload --config /etc/caddy/Caddyfile
cp /var/www/bckr.dev/synaptic-signals.service /etc/systemd/system/ && systemctl daemon-reload && systemctl restart synaptic-signals
```

## Managing the app on the VPS

`deploy-vps.sh` symlinks `synap-cli` into `/usr/local/bin`, so it's runnable
from anywhere **when logged in directly** (not through `sudo -u`, see note
above). It reads `DATABASE_URL` from the `.env` in the **current
directory**, so `cd` into the install dir first:

```bash
cd /var/www/bckr.dev
synap-cli site list
synap-cli user list
synap-cli plugin list
synap-cli theme list
synap-cli --help          # full command list
```

## Verification

```bash
ssh root@178.156.176.60 systemctl status synaptic-signals
ssh root@178.156.176.60 journalctl -u synaptic-signals -n 50
curl -I https://bckr.dev
```

## Troubleshooting

### Service won't start

```bash
journalctl -u synaptic-signals -n 100
```

Common causes: `.env` missing/wrong `DATABASE_URL`, Postgres not running
(`systemctl status postgresql`), port already in use.

### Database connection failed

```bash
psql "postgres://synaptic:PASSWORD@localhost:5432/synaptic_signals"
systemctl status postgresql
```

### Caddy not proxying traffic

```bash
systemctl status caddy
caddy reload --config /etc/caddy/Caddyfile
tail -f /var/log/caddy/bckr.dev.log
```

### Full reset

```bash
VPS_PASSWORD='...' ./scripts/deploy-vps.sh --clean
```

## Security notes

⚠️ **Change the VPS root password** after testing — it was shared in plain
text during setup.

⚠️ **The database password** is auto-generated by the script and stored only
in `${INSTALL_DIR}/.env` on the VPS (`chmod 600`).

⚠️ This VPS hosts other sites (`ioncode.com`, `servermesh.dev`) — `deploy-vps.sh`
only ever touches the `synaptic_signals` DB/role and `bckr.dev`'s own
Caddy/systemd entries, never the shared Caddy install or other sites' data.

## Directory structure on VPS

```
/var/www/bckr.dev/
├── synaptic                  # Binary (deployed, rebuilt every deploy)
├── synap-cli                 # CLI (deployed)
├── .env                      # Environment config (generated once, preserved after)
├── themes/                   # Theme files (deployed)
├── admin/static/             # Admin UI static assets (deployed)
├── uploads/                  # User uploads (created on first run)
├── sites/                    # Multi-site data (created on first run)
├── search-index/             # Search index (created on first run)
└── plugins/                  # Plugins directory (deployed)
```
