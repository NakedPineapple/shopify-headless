SET search_path TO admin, public;

DROP TRIGGER IF EXISTS meta_product_mapping_updated_at ON admin.meta_product_mapping;
DROP TABLE IF EXISTS admin.meta_product_mapping;
