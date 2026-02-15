SET search_path TO admin, public;

DROP TRIGGER IF EXISTS tiktok_sync_state_updated_at ON admin.tiktok_sync_state;
DROP TABLE IF EXISTS admin.tiktok_sync_state;

DROP TABLE IF EXISTS admin.tiktok_order_items;

DROP TRIGGER IF EXISTS tiktok_orders_updated_at ON admin.tiktok_orders;
DROP TABLE IF EXISTS admin.tiktok_orders;
