# Naked Pineapple Storefront

Public-facing e-commerce storefront for Naked Pineapple.

## Overview

The storefront binary serves the customer-facing website on port 3000. It provides:

- Product browsing and search
- Shopping cart with HTMX-powered interactions
- User authentication (password + WebAuthn passkeys)
- Account management and order history
- Blog and static content pages

## Architecture

- **Web Framework**: Axum with HTMX for interactivity
- **Templating**: Askama for server-side rendering
- **Database**: PostgreSQL (np_storefront) for local user data
- **E-commerce**: Shopify Storefront API (source of truth for products)
- **Auth**: Shopify Customer Account API for OAuth

## Security

This binary only has access to:

- Shopify Storefront API (public access)
- Shopify Customer Account API (OAuth)
- Local PostgreSQL database (np_storefront)

It does **NOT** have access to:

- Shopify Admin API (that's in the admin binary)
- Admin PostgreSQL database (np_admin)

## Running

```bash
# Development
cargo run -p naked-pineapple-storefront

# Or via Taskfile
task dev:storefront
```

The server listens on `http://localhost:3000`.

## Configuration

See `.env.example` for required environment variables:

- `STOREFRONT_DATABASE_URL` - PostgreSQL connection string
- `SHOPIFY_STORE` - Shopify store domain
- `SHOPIFY_STOREFRONT_*` - Storefront API tokens
- `SHOPIFY_CUSTOMER_*` - Customer Account API credentials
