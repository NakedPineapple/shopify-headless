SET search_path TO admin, public;

ALTER TABLE admin.admin_user DROP CONSTRAINT IF EXISTS chk_slack_user_id_format;
ALTER TABLE admin.admin_user DROP COLUMN IF EXISTS slack_user_id;
