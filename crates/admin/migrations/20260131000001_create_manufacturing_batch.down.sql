SET search_path TO admin, public;

DROP TRIGGER IF EXISTS manufacturing_batch_updated_at ON admin.manufacturing_batch;
DROP TABLE IF EXISTS admin.manufacturing_batch;
