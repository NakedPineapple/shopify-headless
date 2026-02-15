SET search_path TO admin, public;

DROP TRIGGER IF EXISTS tiktok_shop_performance_updated_at ON admin.tiktok_shop_performance;
DROP TABLE IF EXISTS admin.tiktok_shop_performance;
