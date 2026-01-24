-- Revert shopify_cart_cache table creation

DROP TRIGGER IF EXISTS shopify_cart_cache_updated_at ON storefront.shopify_cart_cache;
DROP TABLE IF EXISTS storefront.shopify_cart_cache;
