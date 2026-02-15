SET search_path TO admin, public;

CREATE TABLE admin.faire_product_mapping (
    id SERIAL PRIMARY KEY,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    faire_product_token TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'manual',
    status TEXT NOT NULL DEFAULT 'active',
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_faire_product_mapping_token
    ON admin.faire_product_mapping (faire_product_token);

CREATE INDEX idx_faire_product_mapping_shopify
    ON admin.faire_product_mapping (shopify_product_id);

CREATE TRIGGER faire_product_mapping_updated_at
    BEFORE UPDATE ON admin.faire_product_mapping
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
