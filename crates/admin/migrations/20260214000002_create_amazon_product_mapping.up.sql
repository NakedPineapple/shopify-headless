SET search_path TO admin, public;

CREATE TABLE admin.amazon_product_mapping (
    id SERIAL PRIMARY KEY,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    asin TEXT NOT NULL,
    amazon_sku TEXT NOT NULL,
    marketplace_id TEXT NOT NULL DEFAULT 'ATVPDKIKX0DER',
    match_type TEXT NOT NULL DEFAULT 'manual',
    status TEXT NOT NULL DEFAULT 'active',
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_amazon_product_mapping_sku_marketplace
    ON admin.amazon_product_mapping (amazon_sku, marketplace_id);

CREATE INDEX idx_amazon_product_mapping_shopify_product
    ON admin.amazon_product_mapping (shopify_product_id);

CREATE INDEX idx_amazon_product_mapping_asin
    ON admin.amazon_product_mapping (asin);

CREATE TRIGGER amazon_product_mapping_updated_at
    BEFORE UPDATE ON admin.amazon_product_mapping
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
