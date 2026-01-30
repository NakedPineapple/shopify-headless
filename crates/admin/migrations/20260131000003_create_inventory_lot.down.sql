SET search_path TO admin, public;

DROP TRIGGER IF EXISTS inventory_lot_updated_at ON admin.inventory_lot;
DROP TABLE IF EXISTS admin.inventory_lot;
