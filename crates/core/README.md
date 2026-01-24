# Naked Pineapple Core

Shared types library for the Naked Pineapple e-commerce platform.

## Overview

This crate provides common types used across all Naked Pineapple components:

- **storefront** - Public-facing e-commerce site
- **admin** - Internal administration panel (Tailscale-only)
- **cli** - Command-line tools for migrations and management

## Design Principles

The core crate contains only types and traits - no I/O, no database access, no HTTP clients. This keeps it lightweight and allows it to be used anywhere without pulling in heavy dependencies.

## Modules

- `types::id` - Type-safe ID wrappers for entities
- `types::price` - Price representation with currency support
- `types::email` - Validated email addresses
- `types::status` - Status enums for orders, fulfillment, etc.

## Usage

```rust
use naked_pineapple_core::{EntityId, Price, CurrencyCode, Email};

let user_id = EntityId::new(42);
let price = Price::new(Decimal::new(1999, 2), CurrencyCode::USD);
let email = Email::new_unchecked("user@example.com");
```
