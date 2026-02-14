SET search_path TO admin, public;

CREATE TABLE admin.meta_product_mapping (
    id SERIAL PRIMARY KEY,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    facebook_product_id TEXT NOT NULL,
    retailer_id TEXT,
    match_type TEXT NOT NULL DEFAULT 'manual',
    status TEXT NOT NULL DEFAULT 'active',
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_meta_product_mapping_fb_product
    ON admin.meta_product_mapping (facebook_product_id);

CREATE INDEX idx_meta_product_mapping_shopify_product
    ON admin.meta_product_mapping (shopify_product_id);

CREATE INDEX idx_meta_product_mapping_retailer
    ON admin.meta_product_mapping (retailer_id) WHERE retailer_id IS NOT NULL;

CREATE TRIGGER meta_product_mapping_updated_at
    BEFORE UPDATE ON admin.meta_product_mapping
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
