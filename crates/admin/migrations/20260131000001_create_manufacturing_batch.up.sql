-- Create manufacturing_batch table for tracking production run costs

SET search_path TO admin, public;

CREATE TABLE admin.manufacturing_batch (
    id SERIAL PRIMARY KEY,
    batch_number TEXT NOT NULL,
    shopify_product_id TEXT NOT NULL,
    shopify_variant_id TEXT,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    manufacture_date DATE NOT NULL,
    raw_cost_per_item DECIMAL(10,4) NOT NULL,
    label_cost_per_item DECIMAL(10,4) NOT NULL DEFAULT 0,
    outer_carton_cost_per_item DECIMAL(10,4) NOT NULL DEFAULT 0,
    cost_per_unit DECIMAL(10,4) GENERATED ALWAYS AS (
        raw_cost_per_item + label_cost_per_item + outer_carton_cost_per_item
    ) STORED,
    total_batch_cost DECIMAL(12,4) GENERATED ALWAYS AS (
        (raw_cost_per_item + label_cost_per_item + outer_carton_cost_per_item) * quantity
    ) STORED,
    currency_code TEXT NOT NULL DEFAULT 'USD',
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE UNIQUE INDEX idx_manufacturing_batch_number_product
    ON admin.manufacturing_batch(batch_number, shopify_product_id);
CREATE INDEX idx_manufacturing_batch_product ON admin.manufacturing_batch(shopify_product_id);
CREATE INDEX idx_manufacturing_batch_date ON admin.manufacturing_batch(manufacture_date DESC);

CREATE TRIGGER manufacturing_batch_updated_at
    BEFORE UPDATE ON admin.manufacturing_batch
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
