# Naked Pineapple

A headless e-commerce platform for [Naked Pineapple](https://nakedpineapple.co) skincare, built with Rust and integrated with Shopify's GraphQL APIs.

## Architecture

The platform is a Rust workspace with two server-side rendered web applications and shared libraries:

```
crates/
  storefront/   Public-facing store (port 3000)
  admin/        Internal admin panel (port 3001, Tailscale VPN only)
  core/         Shared types — EntityId, Price, Email, status enums
  cli/          Database migration tool
```

**Storefront** serves the customer experience: product pages, cart, checkout handoff to Shopify, user accounts with passkey (WebAuthn) authentication, full-text search via Tantivy, and a blog. It supports multiple domains through per-request site context resolution.

**Admin** is an internal tool for order management, customer lookup, AI-assisted chat (Claude API), and Shopify Admin API operations. It is only accessible over Tailscale.

Both applications use the same stack:

- [Axum](https://github.com/tokio-rs/axum) for HTTP routing
- [Askama](https://github.com/djc/askama) for server-side HTML templates
- [HTMX](https://htmx.org) for interactivity without client-side JavaScript frameworks
- [PostgreSQL](https://www.postgresql.org) with [SQLx](https://github.com/launchbadge/sqlx) compile-time verified queries
- [Tailwind CSS](https://tailwindcss.com) v4

Two separate PostgreSQL databases (`np_storefront` and `np_admin`) enforce a security boundary — the admin binary has no access to storefront user data and vice versa.

## Getting Started

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites, setup instructions, and development workflow.

## Deployment

Both applications are containerized and deployed to [Fly.io](https://fly.io). CI/CD is handled by GitHub Actions:

- Push to `main` triggers lint, test, version bump, Docker build, and staging deploy
- Production promotion is a manual workflow dispatch

Docker configurations are in `docker/` and Fly configs in `fly/`.

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](https://opensource.org/licenses/Apache-2.0).
