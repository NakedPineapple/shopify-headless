SET search_path TO admin, public;

CREATE TABLE admin.pinterest_product_mapping (
    id SERIAL PRIMARY KEY,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    pinterest_item_id TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'manual',
    status TEXT NOT NULL DEFAULT 'active',
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_pinterest_product_mapping_item
    ON admin.pinterest_product_mapping (pinterest_item_id);

CREATE INDEX idx_pinterest_product_mapping_shopify
    ON admin.pinterest_product_mapping (shopify_product_id);

CREATE TRIGGER pinterest_product_mapping_updated_at
    BEFORE UPDATE ON admin.pinterest_product_mapping
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
