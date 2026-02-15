SET search_path TO admin, public;

DROP TABLE IF EXISTS admin.tiktok_settlement_line_items;

DROP TRIGGER IF EXISTS tiktok_settlements_updated_at ON admin.tiktok_settlements;
DROP TABLE IF EXISTS admin.tiktok_settlements;
