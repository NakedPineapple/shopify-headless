SET search_path TO admin, public;

DROP TRIGGER IF EXISTS meta_commerce_credentials_updated_at ON admin.meta_commerce_credentials;
DROP TABLE IF EXISTS admin.meta_commerce_credentials;
