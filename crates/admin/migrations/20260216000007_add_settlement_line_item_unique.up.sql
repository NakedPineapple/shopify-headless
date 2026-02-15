-- Add unique constraint for settlement line item upserts.

SET search_path TO admin, public;

-- Settlement line items always reference a specific order.
ALTER TABLE admin.tiktok_settlement_line_items
    ALTER COLUMN tiktok_order_id SET NOT NULL;

CREATE UNIQUE INDEX idx_tiktok_settlement_line_items_unique
    ON admin.tiktok_settlement_line_items(settlement_id, tiktok_order_id);
