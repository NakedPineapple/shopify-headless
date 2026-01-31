SET search_path TO admin, public;

DROP TRIGGER IF EXISTS shiphero_credentials_updated_at ON admin.shiphero_credentials;
DROP TABLE IF EXISTS admin.shiphero_credentials;
