SET search_path TO admin, public;

DROP TRIGGER IF EXISTS meta_sync_state_updated_at ON admin.meta_sync_state;
DROP TABLE IF EXISTS admin.meta_sync_state;

DROP TABLE IF EXISTS admin.meta_order_items;

DROP TRIGGER IF EXISTS meta_orders_updated_at ON admin.meta_orders;
DROP TABLE IF EXISTS admin.meta_orders;
