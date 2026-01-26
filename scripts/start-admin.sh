#!/bin/sh

# Use FLY_APP_NAME as Tailscale hostname (e.g., nakedpineapple-admin or nakedpineapple-admin-staging)
TAILSCALE_HOSTNAME="${FLY_APP_NAME:-nakedpineapple-admin}"

# Start Tailscale daemon in background with reduced verbosity
/app/tailscaled \
  --state=/var/lib/tailscale/tailscaled.state \
  --socket=/var/run/tailscale/tailscaled.sock \
  --verbose=0 \
  2>&1 | grep -v "^\[RATELIMIT\]\|^logpolicy\|^monitor:\|^2026/\|^#" &

# Wait for daemon socket to be ready
for i in 1 2 3 4 5; do
  [ -S /var/run/tailscale/tailscaled.sock ] && break
  sleep 1
done

# Authenticate and connect to tailnet
/app/tailscale --socket=/var/run/tailscale/tailscaled.sock up \
  --auth-key="${TAILSCALE_AUTHKEY}" \
  --hostname="${TAILSCALE_HOSTNAME}" \
  --accept-routes=false \
  2>&1 | grep -v "^#"

# Disable any previous tailscale serve config (app serves HTTPS directly now)
/app/tailscale --socket=/var/run/tailscale/tailscaled.sock serve --https=443 off 2>/dev/null || true

# Start the application (serves HTTPS directly with TLS certificates)
exec /app/server
