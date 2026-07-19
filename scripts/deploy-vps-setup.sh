#!/bin/bash
# VPS Setup Script for Synaptic Signals
# Run this once on your VPS to prepare for deployment
# Usage: sudo bash deploy-vps-setup.sh

set -e

DOMAIN="bckr.dev"
INSTALL_DIR="/var/www/bckr.dev"
SERVICE_USER="www-data"
DB_NAME="synaptic_signals"
DB_USER="synaptic"
DB_PASSWORD="$(openssl rand -base64 24)"  # Generate random password
PORT=3000

echo "=== Synaptic Signals VPS Setup ==="
echo "Domain: $DOMAIN"
echo "Install Dir: $INSTALL_DIR"
echo "Service User: $SERVICE_USER"
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root"
   exit 1
fi

# 1. Update system
echo "Step 1: Updating system packages..."
apt-get update
apt-get upgrade -y

# 2. Install PostgreSQL if not installed
echo "Step 2: Checking PostgreSQL..."
if ! command -v psql &> /dev/null; then
    echo "Installing PostgreSQL..."
    apt-get install -y postgresql postgresql-contrib
    systemctl start postgresql
    systemctl enable postgresql
else
    echo "PostgreSQL already installed"
fi

# 3. Create database and user
echo "Step 3: Setting up PostgreSQL database..."
sudo -u postgres psql <<EOF
SELECT 1 FROM pg_database WHERE datname = '$DB_NAME' \gexec CREATE DATABASE $DB_NAME;
SELECT 1 FROM pg_user WHERE usename = '$DB_USER' \gexec CREATE USER $DB_USER WITH PASSWORD '$DB_PASSWORD';
ALTER ROLE $DB_USER SET client_encoding TO 'utf8';
ALTER ROLE $DB_USER SET default_transaction_isolation TO 'read committed';
ALTER ROLE $DB_USER SET default_transaction_deferrable TO on;
ALTER ROLE $DB_USER SET default_transaction_level TO 'read committed';
ALTER ROLE $DB_USER SET timezone TO 'UTC';
GRANT ALL PRIVILEGES ON DATABASE $DB_NAME TO $DB_USER;
EOF

echo ""
echo "PostgreSQL setup complete!"
echo "DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@localhost:5432/$DB_NAME"
echo ""
echo "IMPORTANT: Save these credentials - you'll need them for .env file"
echo ""

# 4. Install Caddy if not installed
echo "Step 4: Checking Caddy..."
if ! command -v caddy &> /dev/null; then
    echo "Installing Caddy..."
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
    curl https://dl.filippo.io/mkcert/latest?for=linux/amd64 | bash -s -- -installation-only
    apt-get install -y -qq --no-install-recommends caddy
    systemctl enable caddy
else
    echo "Caddy already installed"
fi

# 5. Create installation directory
echo "Step 5: Creating installation directory..."
if [ ! -d "$INSTALL_DIR" ]; then
    mkdir -p "$INSTALL_DIR"
    chown $SERVICE_USER:$SERVICE_USER "$INSTALL_DIR"
fi

# 6. Create subdirectories
echo "Step 6: Creating subdirectories..."
mkdir -p "$INSTALL_DIR/uploads"
mkdir -p "$INSTALL_DIR/sites"
mkdir -p "$INSTALL_DIR/search-index"
mkdir -p "$INSTALL_DIR/themes"
mkdir -p "$INSTALL_DIR/plugins"
chown -R $SERVICE_USER:$SERVICE_USER "$INSTALL_DIR"
chmod 755 "$INSTALL_DIR"

# 7. Create .env file template
echo "Step 7: Creating .env template..."
cat > "$INSTALL_DIR/.env.template" << 'ENVEOF'
# Update these values before deploying
DATABASE_URL=postgres://synaptic:PASSWORD@localhost:5432/synaptic_signals
SECRET_KEY=CHANGE_THIS_TO_A_64_BYTE_RANDOM_STRING
LOG_LEVEL=info
INSTALL_DIR=/var/www/bckr.dev
ADMIN_EMAIL=your-email@example.com
MAX_UPLOAD_MB=25
ENVEOF

chown $SERVICE_USER:$SERVICE_USER "$INSTALL_DIR/.env.template"
chmod 600 "$INSTALL_DIR/.env.template"

echo ""
echo "=== Setup Complete ==="
echo ""
echo "NEXT STEPS:"
echo "1. Create .env file with your actual configuration:"
echo "   cp $INSTALL_DIR/.env.template $INSTALL_DIR/.env"
echo "   nano $INSTALL_DIR/.env"
echo ""
echo "2. On your LOCAL machine:"
echo "   cargo build --release"
echo "   scp target/release/synaptic root@178.156.176.60:$INSTALL_DIR/"
echo "   scp -r themes/* root@178.156.176.60:$INSTALL_DIR/themes/"
echo ""
echo "3. Back on the VPS, set up the systemd service:"
echo "   sudo bash /var/www/bckr.dev/setup-service.sh"
echo ""
echo "Database credentials for .env:"
echo "DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@localhost:5432/$DB_NAME"
