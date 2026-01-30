-- Create lot_allocation table for linking order line items to inventory lots

SET search_path TO admin, public;

CREATE TABLE admin.lot_allocation (
    id SERIAL PRIMARY KEY,
    lot_id INTEGER NOT NULL REFERENCES admin.inventory_lot(id) ON DELETE RESTRICT,
    shopify_order_id TEXT NOT NULL,
    shopify_line_item_id TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    allocated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    allocated_by INTEGER REFERENCES admin.admin_user(id)
);

CREATE INDEX idx_lot_allocation_lot_id ON admin.lot_allocation(lot_id);
CREATE INDEX idx_lot_allocation_order ON admin.lot_allocation(shopify_order_id);
CREATE UNIQUE INDEX idx_lot_allocation_line_item_lot
    ON admin.lot_allocation(shopify_line_item_id, lot_id);
