SET search_path TO admin, public;

DROP INDEX IF EXISTS admin.idx_tiktok_settlement_line_items_unique;

ALTER TABLE admin.tiktok_settlement_line_items
    ALTER COLUMN tiktok_order_id DROP NOT NULL;
