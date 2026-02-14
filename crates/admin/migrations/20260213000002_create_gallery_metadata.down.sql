SET search_path TO admin, public;

DROP TRIGGER IF EXISTS gallery_metadata_updated_at ON admin.gallery_metadata;
DROP TABLE IF EXISTS admin.gallery_metadata;
