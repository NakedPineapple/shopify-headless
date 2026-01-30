# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Naked Pineapple is a Rust e-commerce platform integrated with Shopify. It consists of a public storefront and an internal admin panel, both built with Axum and server-side rendered with Askama templates.

## Development Setup

Required environment variables are documented in `.env.example`.

## Common Commands

**Always use Task commands instead of cargo directly.** The Taskfile wraps cargo with proper configuration. Do not run `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`, or `cargo check` directly. Always prefer comprehensive tasks to targeted tasks: `task check` over `task lint` or `task rust:check`, `task build` over `task rust:build`.

All commands use the [Task](https://taskfile.dev/) runner. Run from `NakedPineapple/`:

```bash
# Development
task dev                    # Run storefront with Tailwind watch (port 3000)
task dev:admin              # Run admin with Tailwind watch (port 3001)

# Testing
task test                   # Run all tests
task test:integration       # Run integration tests (requires db:start)

# Code Quality
task fmt                    # Format code
task check                  # Run all checks (fmt, lint, audit, deny)

# Database (local development)
task db:start               # Start PostgreSQL in Docker. (Likely does not need to be used. The database will already be running.)
task db:migrate             # Run migrations for both databases
task db:reset               # Drop, recreate, and migrate
task sqlx:prepare           # Generate SQLX offline cache for all crates (commit .sqlx/ after running)

# Build
task build                  # Build all (includes CSS. Note that if the user is running `task dev` or `task dev:admin` in parallel to the Claude completing its work then there is a `cargo watch` watcher automatically rebuilding both rust and CSS. `task build` rarely, if ever, needs to be ran by Claude.)

# Running migrations against Fly.io databases
task fly:db:proxy:storefront  # Terminal 1: proxy to localhost:15432
task fly:db:migrate:storefront:staging  # Terminal 2: run migrations
```

## Architecture

### Workspace Crates

- **core** (`crates/core`) - Shared types library with no I/O. Contains `EntityId`, `Price`, `Email`, and status enums.
- **storefront** (`crates/storefront`) - Public e-commerce site (port 3000). Uses Shopify Storefront API.
- **admin** (`crates/admin`) - Internal admin panel (port 3001, Tailscale-only). Uses Shopify Admin API and Claude API for AI chat.
- **cli** (`crates/cli`) - Database migrations (`np-cli migrate storefront|admin|all`).
- **integration-tests** (`crates/integration-tests`) - End-to-end tests.

### Database Isolation

Two separate PostgreSQL databases enforce security boundaries:

- **np_storefront** - User accounts, sessions, search index
- **np_admin** - Admin users, OAuth tokens, chat history

The admin binary has no access to storefront user data and vice versa. Admin is accessible only via Tailscale VPN.

### Key Technologies

- **Web**: Axum 0.8 + HTMX for interactivity
- **Templates**: Askama (server-side rendering)
- **Database**: PostgreSQL with SQLx (compile-time verification via `.sqlx/` cache)
- **CSS**: Tailwind CSS 4.0 (compiled separately for each app)
- **Shopify**: GraphQL clients via `graphql_client` for Storefront, Customer Account, and Admin APIs
- **Search**: Tantivy full-text search
- **AI**: Claude API integration in admin for chat assistant

### Database Workflow

Migrations are SQL files in `crates/{app}/migrations/` and are embedded at compile time. After adding migrations:
1. Run `task db:migrate` (rebuilds CLI to pick up new files)
2. Run `task sqlx:prepare` to update offline cache
3. Commit `.sqlx/` directory

## Code Patterns

### SQLx Query Macros

All database interactions must use SQLx's compile-time verified query macros:
- `query!` - For queries that don't return rows or when you handle results manually
- `query_as!` - For queries that map results to a struct
- `query_scalar!` - For queries that return a single scalar value

Do not use the runtime `query()` or `query_as()` functions. The macros ensure SQL is validated against the database schema at compile time.

### SQLx Offline Mode

Queries are verified at compile time using cached metadata in `.sqlx/`. After changing SQL queries:

```bash
task sqlx:prepare          # Regenerate cache
```

Commit the updated `.sqlx/` directory.

### Error Handling

Use `thiserror` for custom error types. Each crate has an `error.rs` defining its error enum.

### Code Style

- Rust 2024 edition with `#![forbid(unsafe_code)]` in production
- Prices use `rust_decimal::Decimal` wrapped in core `Price` type
- IDs are type-safe via core `EntityId<T>` wrapper

### Function Length Limit

A 100-line function limit is strictly enforced. `#[allow(clippy::too_many_lines)]` should not be used unless splitting the function is genuinely not possible.

### Lint Allows

Avoid `#[allow(...)]` attributes. The following require particular scrutiny:

- `#[allow(clippy::too_many_lines)]` - Refactor the function instead
- `#[allow(dead_code)]` - Remove the unused code instead
- `#[allow(deprecated)]` - Migrate to the non-deprecated API instead

If an allow is truly unavoidable, it **must** include a comment explaining why the allow is necessary and why there is no alternative. This follows the broken window theory: each shortcut makes future shortcuts more acceptable, leading to gradual code quality erosion.

## Brand Identity

The storefront and admin share the same tropical island—just at different times of day.

### Storefront: "Tropical Luxe"

The customer-facing site evokes golden hour on the beach:

- **Colors**: Warm coral (#d63a2f), golden honey, soft cream, terracotta, leaf green
- **Typography**: Playfair Display (elegant headlines) + DM Sans (body)
- **Aesthetic**: Sun-drenched editorial, like a fashion magazine at a tropical resort
- **Motion**: Staggered entrance animations, scroll-triggered reveals, Ken Burns effects
- **Feel**: Vibrant, celebratory, warm and inviting

See `crates/storefront/BRAND_IDENTITY.md` for complete design system including component patterns, color tokens, and layout guidelines.

### Admin: "Midnight Lagoon"

The internal admin is the same island after dark—moonlit, calm, focused:

- **Colors**: Deep lagoon blues with coral accents glowing like embers/torches
- **Typography**: DM Sans for all UI (Playfair Display only for sidebar logo)
- **Aesthetic**: Professional but never corporate—"still barefoot, still island time"
- **Motion**: Minimal—only for loading states and transitions
- **Feel**: Quiet productivity, easy on the eyes for late-night sessions

The admin supports dark mode (primary) and "Morning Shade" light mode (cool whites, shaded cabana feel).

See `crates/admin/BRAND_IDENTITY.md` for complete design system including the Lagoon color scale, semantic tokens, and component specifications.
