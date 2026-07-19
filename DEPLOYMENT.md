# Synaptic Signals VPS Deployment Guide

This guide covers deploying Synaptic Signals to your VPS at `178.156.176.60` with domain `bckr.dev`.

## Quick Start

### Phase 1: VPS Setup (One-time, ~10 minutes)

```bash
# SSH into VPS
ssh root@178.156.176.60

# Run the setup script (installs PostgreSQL, Caddy, creates directories)
bash /tmp/deploy-vps-setup.sh

# Save the database credentials shown at the end!
```

### Phase 2: Build Locally & Deploy (From your dev machine)

```bash
# Build release binary
cd /home/ssrust26/synaptic-signals
cargo build --release

# Copy binary to VPS
scp target/release/synaptic root@178.156.176.60:/var/www/bckr.dev/

# Copy themes
scp -r themes/* root@178.156.176.60:/var/www/bckr.dev/themes/

# (Optional) Copy migrations if they've changed
scp -r migrations root@178.156.176.60:/var/www/bckr.dev/
```

### Phase 3: Configure & Start (On VPS)

```bash
# SSH into VPS
ssh root@178.156.176.60

# Create .env from template
cp /var/www/bckr.dev/.env.template /var/www/bckr.dev/.env
nano /var/www/bckr.dev/.env

# Edit these values:
# - DATABASE_URL: Use credentials from Phase 1 output
# - SECRET_KEY: Generate a random 64-byte string
# - ADMIN_EMAIL: Your email
# - LOG_LEVEL: info (or debug for troubleshooting)

# Run service setup
bash /var/www/bckr.dev/deploy-service-setup.sh

# Check status
systemctl status synaptic-signals
```

Visit `https://bckr.dev` - it should be live! 🚀

---

## Detailed Breakdown

### Step 1: Initial VPS Setup

Run this **once** on your VPS to prepare everything:

```bash
ssh root@178.156.176.60
curl -O https://raw.githubusercontent.com/yourusername/synaptic-signals/main/scripts/deploy-vps-setup.sh
bash deploy-vps-setup.sh
```

This script:
- ✅ Updates system packages
- ✅ Installs PostgreSQL (if needed)
- ✅ Creates database and user
- ✅ Installs Caddy
- ✅ Creates directory structure
- ✅ Generates random DB password

**Save the output!** You'll need the `DATABASE_URL` for your `.env` file.

### Step 2: Build Locally

On your **development machine** (where you have Rust installed):

```bash
cd /home/ssrust26/synaptic-signals

# Build in release mode (optimized, smaller binary)
cargo build --release

# Binary location: target/release/synaptic (~50-100MB)
```

### Step 3: Deploy Binary & Assets

Copy the compiled binary and theme files to VPS:

```bash
# Copy binary
scp target/release/synaptic root@178.156.176.60:/var/www/bckr.dev/

# Copy themes directory
scp -r themes/* root@178.156.176.60:/var/www/bckr.dev/themes/

# (Optional) Copy migrations if they changed
scp -r migrations root@178.156.176.60:/var/www/bckr.dev/
```

### Step 4: Configure Environment

SSH into VPS and create `.env`:

```bash
ssh root@178.156.176.60
cd /var/www/bckr.dev

# Copy template
cp .env.template .env

# Edit with your values
nano .env
```

**Required environment variables:**

```env
# From Phase 1 setup script output
DATABASE_URL=postgres://synaptic:PASSWORD@localhost:5432/synaptic_signals

# Generate random string: openssl rand -base64 48
SECRET_KEY=YOUR_64_BYTE_RANDOM_STRING_HERE

# Logging level
LOG_LEVEL=info

# Installation directory
INSTALL_DIR=/var/www/bckr.dev

# Your email for admin alerts
ADMIN_EMAIL=bill.coker@gmail.com

# Max upload size in MB
MAX_UPLOAD_MB=25
```

### Step 5: Set Up Service & Start

```bash
# Run service setup script
bash /var/www/bckr.dev/deploy-service-setup.sh
```

This script:
- ✅ Sets binary permissions
- ✅ Verifies .env exists
- ✅ Creates systemd service
- ✅ Configures Caddy
- ✅ Starts both services
- ✅ Shows status

If all looks good, you're done!

---

## Verification

### Test the deployment:

```bash
# Check service status
systemctl status synaptic-signals

# View recent logs
journalctl -u synaptic-signals -n 50

# Check if listening on port 3000
netstat -tlnp | grep 3000

# Check Caddy config
caddy validate --config /etc/caddy/Caddyfile
```

### Access the app:

- **User-facing site**: https://bckr.dev
- **Admin panel**: https://bckr.dev/admin
- **Check logs**: `sudo journalctl -u synaptic-signals -f`

---

## Future Deployments (After Initial Setup)

Once the VPS is configured, **subsequent updates** are much faster:

```bash
# On your local machine
cd /home/ssrust26/synaptic-signals
cargo build --release
scp target/release/synaptic root@178.156.176.60:/var/www/bckr.dev/

# On VPS, restart the service
ssh root@178.156.176.60 "systemctl restart synaptic-signals"
```

The systemd service will automatically pick up the new binary.

---

## Troubleshooting

### Service won't start?

```bash
# Check what's wrong
journalctl -u synaptic-signals -n 100

# Common issues:
# - .env file missing or incorrect
# - DATABASE_URL wrong
# - Database not running (systemctl start postgresql)
# - Port 3000 already in use
```

### Database connection failed?

```bash
# Test connection manually
psql "postgres://synaptic:PASSWORD@localhost:5432/synaptic_signals"

# Check PostgreSQL is running
systemctl status postgresql
systemctl restart postgresql
```

### Caddy not proxying traffic?

```bash
# Check Caddy is running
systemctl status caddy

# Reload Caddy config
caddy reload --config /etc/caddy/Caddyfile

# Check logs
tail -f /var/log/caddy/bckr.dev.log
```

### Reset to redeploy everything?

```bash
# Stop services
systemctl stop synaptic-signals caddy

# Clean app directory (but preserve uploads, sites)
rm -f /var/www/bckr.dev/synaptic
rm -f /var/www/bckr.dev/.env

# Redeploy from Phase 2 above
```

---

## Security Notes

⚠️ **After deployment, change the SSH password:**

```bash
ssh root@178.156.176.60
passwd  # Change from AnAwpWeqJWvc to something secure
```

⚠️ **SECRET_KEY must be random and long:**

```bash
openssl rand -base64 48  # Generate secure key
```

⚠️ **Database password is auto-generated** during Phase 1 - it's random and secure by default.

---

## Directory Structure on VPS

```
/var/www/bckr.dev/
├── synaptic                 # Binary (deployed)
├── .env                      # Environment config (created manually)
├── themes/                   # Theme files (deployed)
├── uploads/                  # User uploads (created on first run)
├── sites/                    # Multi-site data (created on first run)
├── search-index/             # Search index (created on first run)
├── plugins/                  # Plugins directory (created on first run)
└── migrations/               # Database migrations (optional, deployed)
```

---

## Monitoring & Maintenance

### View logs:

```bash
# Synaptic app logs
sudo journalctl -u synaptic-signals -f

# Caddy reverse proxy logs
sudo tail -f /var/log/caddy/bckr.dev.log

# System logs
sudo dmesg | tail
```

### Restart services:

```bash
sudo systemctl restart synaptic-signals
sudo systemctl restart caddy
```

### Check disk usage:

```bash
du -sh /var/www/bckr.dev/uploads
du -sh /var/www/bckr.dev/sites
```

---

## Support

If issues arise, check:
1. Logs: `journalctl -u synaptic-signals -n 100`
2. `.env` file is correct
3. PostgreSQL is running: `systemctl status postgresql`
4. Port 3000 is free: `lsof -i :3000`
5. Caddy config: `caddy validate --config /etc/caddy/Caddyfile`
