# Naked Pineapple CLI

Command-line tools for Naked Pineapple database migrations and management.

## Overview

The CLI provides tools for:

- Database migrations (storefront and admin databases)
- Future: User management, cache clearing, data seeding

## Installation

```bash
cargo install --path crates/cli
```

Or run directly:

```bash
cargo run -p naked-pineapple-cli -- <command>
```

## Commands

### Migrations

Run database migrations:

```bash
# Run storefront migrations
np-cli migrate storefront

# Run admin migrations
np-cli migrate admin

# Run all migrations
np-cli migrate all
```

## Configuration

The CLI reads database URLs from environment variables:

- `STOREFRONT_DATABASE_URL` - PostgreSQL connection for storefront
- `ADMIN_DATABASE_URL` - PostgreSQL connection for admin

You can set these in a `.env` file or export them directly.

## Migration Files

Migrations are stored in each crate's `migrations/` directory:

- `crates/storefront/migrations/` - Storefront database migrations
- `crates/admin/migrations/` - Admin database migrations

Migrations follow the naming convention: `YYYYMMDDHHMMSS_description.sql`
