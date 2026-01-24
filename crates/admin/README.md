# Naked Pineapple Admin

Internal administration panel for Naked Pineapple.

## Overview

The admin binary serves the internal admin interface on port 3001. It provides:

- Dashboard with key metrics
- Product, order, and customer management (via Shopify Admin API)
- Inventory management
- AI-powered chat assistant (Claude integration)
- Admin user management

## Security

**CRITICAL: This binary must ONLY run on Tailscale-protected infrastructure.**

- Accessible only via Tailscale VPN
- Requires MDM-managed devices
- Contains HIGH PRIVILEGE Shopify Admin API token
- Has access to admin-only PostgreSQL database (np_admin)

### Security Isolation

The admin binary is completely isolated from the storefront:

- Separate PostgreSQL database (np_admin vs np_storefront)
- Separate user tables (admin_users ≠ storefront users)
- Admin API token only available in this binary
- Deployed on separate VM accessible only via Tailscale

## Architecture

- **Web Framework**: Axum
- **Templating**: Askama for server-side rendering
- **Database**: PostgreSQL (np_admin) for admin users and chat history
- **E-commerce**: Shopify Admin API (HIGH PRIVILEGE)
- **AI**: Claude API for chat assistant

## Running

```bash
# Development (ensure Tailscale is connected)
cargo run -p naked-pineapple-admin

# Or via Taskfile
task dev:admin
```

The server listens on `http://localhost:3001`.

## Configuration

See `.env.example` for required environment variables:

- `ADMIN_DATABASE_URL` - PostgreSQL connection string
- `SHOPIFY_ADMIN_ACCESS_TOKEN` - Admin API token (HIGH PRIVILEGE)
- `CLAUDE_API_KEY` - Claude API key for AI chat
