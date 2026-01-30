-- Create inventory_lot table for tracking units received from manufacturing batches

SET search_path TO admin, public;

CREATE TABLE admin.inventory_lot (
    id SERIAL PRIMARY KEY,
    batch_id INTEGER NOT NULL REFERENCES admin.manufacturing_batch(id) ON DELETE RESTRICT,
    lot_number TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    received_date DATE NOT NULL,
    shopify_location_id TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_inventory_lot_batch_id ON admin.inventory_lot(batch_id);
CREATE INDEX idx_inventory_lot_location ON admin.inventory_lot(shopify_location_id);
CREATE INDEX idx_inventory_lot_received ON admin.inventory_lot(received_date DESC);

CREATE TRIGGER inventory_lot_updated_at
    BEFORE UPDATE ON admin.inventory_lot
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
