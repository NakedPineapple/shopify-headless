# Naked Pineapple Integration Tests

Integration tests for the Naked Pineapple e-commerce platform.

## Running Tests

```bash
# Start the database
task db:start

# Run integration tests
task test:integration

# Run with coverage
task coverage:integration
```

## Test Structure

- **Storefront tests** - Public API and user flows
- **Admin tests** - Admin panel and management APIs
- **Database tests** - Direct database integration tests

## Environment

Tests use the `integration_test` database user with access to both `np_storefront` and `np_admin` databases.
