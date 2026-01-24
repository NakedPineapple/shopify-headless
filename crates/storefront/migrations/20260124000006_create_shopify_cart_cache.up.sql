-- Create shopify_cart_cache table to persist Shopify cart IDs across sessions
-- Maps browser session IDs to Shopify cart IDs

SET search_path TO storefront, public;

CREATE TABLE storefront.shopify_cart_cache (
    session_id TEXT PRIMARY KEY,
    shopify_cart_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER shopify_cart_cache_updated_at
    BEFORE UPDATE ON storefront.shopify_cart_cache
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();
