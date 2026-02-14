SET search_path TO admin, public;

DROP TRIGGER IF EXISTS amazon_sp_credentials_updated_at ON admin.amazon_sp_credentials;
DROP TABLE IF EXISTS admin.amazon_sp_credentials;
