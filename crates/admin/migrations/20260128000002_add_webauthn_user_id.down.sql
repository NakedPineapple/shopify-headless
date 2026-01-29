SET search_path TO admin, public;

DROP INDEX IF EXISTS admin.idx_admin_user_webauthn_user_id;
ALTER TABLE admin.admin_user DROP COLUMN IF EXISTS webauthn_user_id;
