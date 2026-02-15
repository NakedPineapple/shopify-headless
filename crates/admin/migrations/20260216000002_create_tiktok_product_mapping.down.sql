SET search_path TO admin, public;

DROP TRIGGER IF EXISTS tiktok_product_mapping_updated_at ON admin.tiktok_product_mapping;
DROP TABLE IF EXISTS admin.tiktok_product_mapping;
