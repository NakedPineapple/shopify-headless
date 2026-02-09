SET search_path TO admin, public;

ALTER TABLE admin.admin_user
DROP COLUMN IF EXISTS password_hash;
