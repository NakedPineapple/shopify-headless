# Contributing to Pineapple Skin Co.

Thank you for your interest in contributing. This guide covers everything you need to get started.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Task](https://taskfile.dev/) (task runner — all commands go through `task`, never raw `cargo`)
- [Docker](https://www.docker.com/) (for local PostgreSQL)
- [Node.js](https://nodejs.org/) 22+ (for Tailwind CSS and image optimization)

## Getting Started

1. Clone the repository and copy the environment template:

   ```bash
   cp .env.example .env
   ```

2. Fill in the required values in `.env`. At minimum you need database URLs and Shopify API credentials. See `.env.example` for documentation on each variable.

3. Start the local database and run migrations:

   ```bash
   task db:start
   task db:migrate
   ```

4. Start a development server:

   ```bash
   task dev          # Storefront on port 3000
   task dev:admin    # Admin on port 3001
   ```

## Development Workflow

### Commands

Always use `task` instead of running `cargo` directly:

```bash
task check    # Format, lint, audit, deny (run before pushing)
task test     # Run all tests
task build    # Build everything including CSS
task fmt      # Format code
```

### Code Quality

Before submitting a pull request, run `task check` and `task test`. The CI pipeline runs these same checks.

Key rules enforced by CI:

- **No `unsafe` code** — production crates use `#![forbid(unsafe_code)]`
- **100-line function limit** — refactor rather than adding `#[allow(clippy::too_many_lines)]`
- **No `#[allow(...)]` attributes** without a comment explaining why there is no alternative
- **SQLx compile-time verification** — use `query!`, `query_as!`, `query_scalar!` macros, not runtime query functions

### Database Changes

After adding or modifying SQL migrations:

```bash
task db:migrate
task sqlx:prepare
```

Commit the updated `.sqlx/` directory with your changes.

## Commit Messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/) enforced by [cocogitto](https://docs.cocogitto.io/). A lefthook git hook validates commit messages automatically.

### Format

```
type(scope): description
```

### Types

`feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `ci`, `build`, `chore`, `style`, `revert`

### Scopes

Crate scopes: `storefront`, `admin`, `core`, `cli`

Domain scopes: `cart`, `blog`, `images`, `search`, `warehouse`

Cross-cutting: `analytics`, `logging`, `monitoring`, `security`

Infrastructure: `deploy`, `ci`, `infra`, `deps`, `docs`

### Examples

```
feat(storefront): add size guide modal to product page
fix(admin): prevent duplicate webhook processing
refactor(core): extract Price formatting into Display impl
ci: add CodeQL workflow for security scanning
```

## Pull Requests

- Branch from `main` and open your PR against `main`.
- Keep PRs focused — one logical change per PR.
- CI must pass before merging (lint, test, conventional commit check).
- Include a short description of what changed and why in the PR body.

## Security

If you discover a security vulnerability, **do not open a public issue**. See [SECURITY.md](SECURITY.md) for reporting instructions.
