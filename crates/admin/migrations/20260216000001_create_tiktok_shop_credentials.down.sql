SET search_path TO admin, public;

DROP TRIGGER IF EXISTS tiktok_shop_credentials_updated_at ON admin.tiktok_shop_credentials;
DROP TABLE IF EXISTS admin.tiktok_shop_credentials;
