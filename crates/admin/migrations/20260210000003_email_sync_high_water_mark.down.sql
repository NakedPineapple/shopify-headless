SET search_path TO admin, public;

ALTER TABLE admin.inbound_email
    DROP COLUMN IF EXISTS folder,
    DROP COLUMN IF EXISTS is_read;

DROP TABLE IF EXISTS admin.email_sync_state;
