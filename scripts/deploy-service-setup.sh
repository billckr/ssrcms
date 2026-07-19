#!/bin/bash
# Service Setup for Synaptic Signals on VPS
# Run this on the VPS after deploying the binary
# Usage: sudo bash /var/www/bckr.dev/setup-service.sh

set -e

DOMAIN="bckr.dev"
INSTALL_DIR="/var/www/bckr.dev"
SERVICE_USER="www-data"
PORT=3000
UPLOADS_DIR="$INSTALL_DIR/uploads"
THEME_DIR="$INSTALL_DIR/themes"

echo "=== Synaptic Signals Service Setup ==="

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root"
   exit 1
fi

# 1. Check if binary exists
if [ ! -f "$INSTALL_DIR/synaptic" ]; then
    echo "ERROR: Binary not found at $INSTALL_DIR/synaptic"
    echo "Deploy the binary first with: scp target/release/synaptic root@178.156.176.60:$INSTALL_DIR/"
    exit 1
fi

echo "Step 1: Setting binary permissions..."
chmod +x "$INSTALL_DIR/synaptic"
chown $SERVICE_USER:$SERVICE_USER "$INSTALL_DIR/synaptic"

# 2. Check if .env exists
if [ ! -f "$INSTALL_DIR/.env" ]; then
    echo "ERROR: .env file not found at $INSTALL_DIR/.env"
    echo "Create it from the template: cp $INSTALL_DIR/.env.template $INSTALL_DIR/.env"
    echo "Then edit it: nano $INSTALL_DIR/.env"
    exit 1
fi

echo "Step 2: Checking .env file..."
chown $SERVICE_USER:$SERVICE_USER "$INSTALL_DIR/.env"
chmod 600 "$INSTALL_DIR/.env"

# 3. Run database migrations
echo "Step 3: Running database migrations..."
cd "$INSTALL_DIR"
if [ -d "migrations" ]; then
    echo "Migrations found. Running..."
    # The binary will run migrations on startup via sqlx-migrate
else
    echo "WARNING: No migrations directory found"
fi

# 4. Set up systemd service
echo "Step 4: Setting up systemd service..."
cat > /etc/systemd/system/synaptic-signals.service << 'SERVICEEOF'
[Unit]
Description=Synaptic Signals CMS
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/var/www/bckr.dev
EnvironmentFile=/var/www/bckr.dev/.env
ExecStart=/var/www/bckr.dev/synaptic
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=synaptic-signals

# Hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/var/www/bckr.dev/uploads /var/www/bckr.dev/search-index /var/www/bckr.dev/sites /var/www/bckr.dev/plugins /var/www/bckr.dev /etc/caddy/Caddyfile /var/log/caddy

[Install]
WantedBy=multi-user.target
SERVICEEOF

chmod 644 /etc/systemd/system/synaptic-signals.service

# 5. Set up Caddy configuration
echo "Step 5: Setting up Caddy..."
cat > /etc/caddy/Caddyfile << CADDYEOF
$DOMAIN {
    # Serve uploads and theme static files directly — bypass Axum
    handle /uploads/* {
        root * $UPLOADS_DIR
        file_server
    }

    handle /theme/* {
        root * $THEME_DIR
        file_server
    }

    # Everything else goes to Axum
    reverse_proxy localhost:$PORT

    # Compression
    encode zstd gzip

    # Security headers
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "SAMEORIGIN"
        Referrer-Policy "strict-origin-when-cross-origin"
        -Server
    }

    log {
        output file /var/log/caddy/$DOMAIN.log
        format json
    }
}
CADDYEOF

# Create log directory
mkdir -p /var/log/caddy
chown caddy:caddy /var/log/caddy

# 6. Reload systemd and enable service
echo "Step 6: Enabling services..."
systemctl daemon-reload
systemctl enable synaptic-signals
systemctl enable caddy

# 7. Start services
echo "Step 7: Starting services..."
systemctl restart caddy
systemctl restart synaptic-signals

# Wait for service to start
sleep 2

# Check status
echo ""
echo "=== Service Status ==="
systemctl status synaptic-signals --no-pager
echo ""
echo "=== Caddy Status ==="
systemctl status caddy --no-pager

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Access your app at: https://$DOMAIN"
echo ""
echo "View logs:"
echo "  Synaptic: sudo journalctl -u synaptic-signals -f"
echo "  Caddy: sudo tail -f /var/log/caddy/$DOMAIN.log"
echo ""
echo "Restart service:"
echo "  sudo systemctl restart synaptic-signals"
