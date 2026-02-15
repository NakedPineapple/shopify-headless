SET search_path TO admin, public;

CREATE TABLE admin.tiktok_product_mapping (
    id SERIAL PRIMARY KEY,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    tiktok_product_id TEXT NOT NULL,
    tiktok_sku_id TEXT,
    match_type TEXT NOT NULL DEFAULT 'manual',
    status TEXT NOT NULL DEFAULT 'active',
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_tiktok_product_mapping_product
    ON admin.tiktok_product_mapping (tiktok_product_id);

CREATE INDEX idx_tiktok_product_mapping_shopify_product
    ON admin.tiktok_product_mapping (shopify_product_id);

CREATE INDEX idx_tiktok_product_mapping_sku
    ON admin.tiktok_product_mapping (tiktok_sku_id) WHERE tiktok_sku_id IS NOT NULL;

CREATE TRIGGER tiktok_product_mapping_updated_at
    BEFORE UPDATE ON admin.tiktok_product_mapping
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
