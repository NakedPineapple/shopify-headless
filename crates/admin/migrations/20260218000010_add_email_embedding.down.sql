SET search_path TO admin, public;

DROP INDEX IF EXISTS admin.idx_inbound_email_embedding;
ALTER TABLE admin.inbound_email DROP COLUMN IF EXISTS embedding;
