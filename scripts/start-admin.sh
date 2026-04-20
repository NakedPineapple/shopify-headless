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

# NAT keep-alive: Fly's stateful NAT drops unsolicited inbound WireGuard UDP,
# so after a restart peers cannot re-handshake with us until we initiate
# outbound traffic. Ping every tailnet peer in a loop to keep sessions warm
# in both directions. Without this, admin becomes unreachable on every restart
# (including the weekly TLS cert renewal) until someone manually pokes it.
(
  set +e
  sleep 5
  while :; do
    /app/tailscale --socket=/var/run/tailscale/tailscaled.sock status --peers 2>/dev/null \
      | awk '/^100\./ { print $1 }' \
      | while read -r peer_ip; do
          /app/tailscale --socket=/var/run/tailscale/tailscaled.sock ping -c 1 --timeout=3s "$peer_ip" >/dev/null 2>&1
        done
    sleep 60
  done
) &

echo "Tailscale connected. Starting application..."
exec su -s /bin/sh appuser -c "/app/server"
