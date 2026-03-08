#!/bin/sh
set -e

TAILSCALE_HOSTNAME="${FLY_APP_NAME:-nakedpineapple-admin}"

# Start Tailscale daemon in background
/app/tailscaled \
  --state=/var/lib/tailscale/tailscaled.state \
  --socket=/var/run/tailscale/tailscaled.sock \
  --verbose=1 &

# Wait for daemon socket to be ready (up to 30 seconds)
echo "Waiting for tailscaled socket..."
for i in $(seq 1 30); do
  [ -S /var/run/tailscale/tailscaled.sock ] && break
  if [ "$i" -eq 30 ]; then
    echo "FATAL: tailscaled socket not ready after 30 seconds"
    exit 1
  fi
  sleep 1
done

# Authenticate and connect to tailnet
echo "Connecting to tailnet as ${TAILSCALE_HOSTNAME}..."
if ! /app/tailscale --socket=/var/run/tailscale/tailscaled.sock up \
  --auth-key="${TAILSCALE_AUTHKEY}?ephemeral=false&preauthorized=true" \
  --hostname="${TAILSCALE_HOSTNAME}" \
  --advertise-tags="${TAILSCALE_TAGS}" \
  --accept-routes=false \
  --reset; then
  echo "FATAL: tailscale up failed"
  exit 1
fi

# Verify Tailscale is connected
if ! /app/tailscale --socket=/var/run/tailscale/tailscaled.sock status; then
  echo "FATAL: Tailscale not connected after authentication"
  exit 1
fi

# Disable any previous tailscale serve config (app serves HTTPS directly now)
/app/tailscale --socket=/var/run/tailscale/tailscaled.sock serve --https=443 off 2>/dev/null || true

echo "Tailscale connected. Starting application..."
exec su -s /bin/sh appuser -c "/app/server"
