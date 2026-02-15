SET search_path TO admin, public;

DROP TRIGGER IF EXISTS tiktok_returns_updated_at ON admin.tiktok_returns;
DROP TABLE IF EXISTS admin.tiktok_returns;
